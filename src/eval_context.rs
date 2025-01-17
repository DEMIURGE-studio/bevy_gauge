use std::sync::Arc;
use bevy::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_utils::HashMap;
use crate::prelude::*;

// ---------------------------------------------------------------------
// 1) Definition of the StatContextRefs enum
// ---------------------------------------------------------------------
#[derive(Debug)]
pub enum StatContextRefs<'a> {
    Definitions(&'a StatDefinitions),
    SubContext(Box<HardMap<'a>>),
}

// ---------------------------------------------------------------------
// 2) The "hard" map with three possible slots
// ---------------------------------------------------------------------
#[derive(Debug)]
pub struct HardMap<'a> {
    refs: [Option<StatContextRefs<'a>>; 3],
}

impl<'a> HardMap<'a> {
    pub fn new() -> Self {
        Self {
            refs: [None, None, None],
        }
    }

    pub fn set(&mut self, key: StatContextType, val: StatContextRefs<'a>) {
        self.refs[key.idx()] = Some(val);
    }

    pub fn get(&self, key: StatContextType) -> &Option<StatContextRefs<'a>> {
        &self.refs[key.idx()]
    }

    /// A helper to pick the correct slot from a string.
    pub fn get_by_str(&self, key: &str) -> Option<&StatContextRefs<'a>> {
        match key {
            "self"   => self.get(StatContextType::This).as_ref(),
            "parent" => self.get(StatContextType::Parent).as_ref(),
            "target" => self.get(StatContextType::Target).as_ref(),
            _        => None,
        }
    }
}

// ---------------------------------------------------------------------
// 3) StatContext using Arc<str> for its keys
// ---------------------------------------------------------------------
#[derive(Component, Default)]
pub struct StatContext {
    pub sources: HashMap<Arc<str>, Entity>,
}

impl StatContext {
    /// Insert a mapping from a context key to an entity.
    /// The key is now stored as an Arc<str>.
    pub fn insert(&mut self, context: &str, entity: Entity) {
        // Convert the &str to Arc<str>
        self.sources.insert(Arc::from(context), entity);
    }

    pub fn trigger_change_detection(&mut self) {
        // Implementation omitted.
    }
}

// ---------------------------------------------------------------------
// 4) Implementation of StatContextRefs
// ---------------------------------------------------------------------
impl<'a> StatContextRefs<'a> {
    /// Build a StatContextRefs by scanning an entity’s definitions/context,
    /// storing them in a HardMap (with fixed keys) rather than a dynamic HashMap.
    pub fn build(
        entity: Entity,
        defs_query: &'a Query<'_, '_, &StatDefinitions>,
        ctx_query: &'a Query<'_, '_, &StatContext>,
    ) -> StatContextRefs<'a> {
        let mut hard_map = HardMap::new();

        // If the entity itself has definitions, store them under the "This" slot.
        if let Ok(defs) = defs_query.get(entity) {
            hard_map.set(StatContextType::This, StatContextRefs::Definitions(defs));
        }

        // If the entity has a StatContext, build subcontexts for each known key.
        if let Ok(stat_context) = ctx_query.get(entity) {
            for (key, child_entity) in &stat_context.sources {
                // Since the keys are now Arc<str>, we compare using key.as_ref().
                if *child_entity == entity {
                    continue; // Avoid self-reference.
                }
                let child_src = Self::build(*child_entity, defs_query, ctx_query);
                match key.as_ref() {
                    "self"   => hard_map.set(StatContextType::This, child_src),
                    "parent" => hard_map.set(StatContextType::Parent, child_src),
                    "target" => hard_map.set(StatContextType::Target, child_src),
                    _ => { /* ignore unknown keys */ }
                }
            }
        }

        StatContextRefs::SubContext(Box::new(hard_map))
    }

    /// Public getter that splits on '.' and calls get_parts recursively.
    pub fn get(&self, var: &str) -> Result<f32, StatError> {
        let parts: Vec<&str> = var.split('.').collect();
        self.get_parts(&parts)
    }

    fn get_parts(&self, parts: &[&str]) -> Result<f32, StatError> {
        if parts.is_empty() {
            return Err(StatError::NotFound("Empty stat identifier".to_string()));
        }

        match self {
            // ---------- 1) Leaf: Definitions
            StatContextRefs::Definitions(defs) => {
                if parts.len() == 1 {
                    defs.get_str(parts[0].into(), self)
                } else {
                    let joined = parts.join(".");
                    defs.get_str(joined.into(), self)
                }
            }

            // ---------- 2) Branch: SubContext (using HardMap)
            StatContextRefs::SubContext(hard_map) => {
                let head = parts[0];
                if Self::is_stat_name_segment(head) {
                    let joined = parts.join(".");
                    if let Some(StatContextRefs::Definitions(defs)) = hard_map.get(StatContextType::This) {
                        return defs.get_str(joined.into(), self);
                    } else {
                        return Err(StatError::NotFound(
                            format!("No 'self' definitions to handle stat {:?}", joined)
                        ));
                    }
                }

                if parts.len() == 1 {
                    return Err(StatError::NotFound(format!(
                        "Got a single-lowercase-part {:?}, but no stat name was provided",
                        head
                    )));
                }

                let tail = &parts[1..];
                match hard_map.get_by_str(head) {
                    Some(StatContextRefs::Definitions(defs)) => {
                        if tail.len() == 1 {
                            defs.get_str(tail[0].into(), self)
                        } else {
                            let joined = tail.join(".");
                            defs.get_str(joined.into(), self)
                        }
                    }
                    Some(StatContextRefs::SubContext(child_map)) => {
                        if tail.is_empty() {
                            return Err(StatError::NotFound("Empty tail".to_string()));
                        }
                        let head2 = tail[0];
                        let tail2 = &tail[1..];
                        if Self::is_stat_name_segment(head2) {
                            let joined = tail.join(".");
                            if let Some(StatContextRefs::Definitions(defs)) = child_map.get(StatContextType::This) {
                                return defs.get_str(joined.into(), self);
                            } else {
                                return Err(StatError::NotFound(format!(
                                    "No 'self' in subcontext to handle stat: {}",
                                    joined
                                )));
                            }
                        } else {
                            match child_map.get_by_str(head2) {
                                Some(child_src) => child_src.get_parts(tail2),
                                None => Err(StatError::NotFound(format!(
                                    "No subcontext for '{}'",
                                    head2
                                ))),
                            }
                        }
                    }
                    _ => Err(StatError::NotFound(format!(
                        "Key '{}' not found among subcontext",
                        head
                    ))),
                }
            }
        }
    }

    /// Helper: if the segment starts with an uppercase letter, treat it as a stat name.
    fn is_stat_name_segment(segment: &str) -> bool {
        segment.chars().next().map(char::is_uppercase).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------
// 5) Definition of StatContextType
// ---------------------------------------------------------------------
#[derive(Debug, Clone, Copy)]
pub enum StatContextType {
    This = 0,
    Parent = 1,
    Target = 2,
}

impl StatContextType {
    pub fn idx(&self) -> usize {
        *self as usize
    }
}

// ---------------------------------------------------------------------
// 6) A SystemParam wrapper to access queries
// ---------------------------------------------------------------------
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    definitions: Query<'w, 's, &'static StatDefinitions>,
    contexts: Query<'w, 's, &'static StatContext>,
}

impl StatAccessor<'_, '_> {
    pub fn build(&self, entity: Entity) -> StatContextRefs {
        StatContextRefs::build(entity, &self.definitions, &self.contexts)
    }
}
