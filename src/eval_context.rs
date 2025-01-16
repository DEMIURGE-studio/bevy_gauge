use bevy::prelude::*;
use bevy_utils::HashMap;
use crate::prelude::*;

#[derive(Debug)]
pub enum StatContextRefs<'a> {
    Definitions(&'a StatDefinitions),
    SubContext(Box<HardMap<'a>>),
    // This variant is used for an uninitialized or empty slot
    NoContext,
}

// ---------------------------------------------------------------------
// 2) The "hard" map with three possible slots
// ---------------------------------------------------------------------
#[derive(Debug)]
pub struct HardMap<'a> {
    refs: [StatContextRefs<'a>; 3],
}

impl<'a> HardMap<'a> {
    pub fn new() -> Self {
        Self {
            refs: [
                StatContextRefs::NoContext,
                StatContextRefs::NoContext,
                StatContextRefs::NoContext,
            ]
        }
    }

    pub fn set(&mut self, key: StatContextType, val: StatContextRefs<'a>) {
        self.refs[key.idx()] = val;
    }

    pub fn get(&self, key: StatContextType) -> &StatContextRefs<'a> {
        &self.refs[key.idx()]
    }

    /// A helper to pick the correct slot from a string:
    pub fn get_by_str(&self, key: &str) -> Option<&StatContextRefs<'a>> {
        match key {
            "self"   => Some(self.get(StatContextType::This)),
            "parent" => Some(self.get(StatContextType::Parent)),
            "target" => Some(self.get(StatContextType::Target)),
            _        => None,
        }
    }
}


// Placeholder for your real StatContext
#[derive(Component, Default)]
pub struct StatContext {
    pub sources: HashMap<String, Entity>,
}

impl StatContext {
    pub fn insert(&mut self, context: &str, entity: Entity) {
        self.sources.insert(context.to_string(), entity);
    }
    pub fn trigger_change_detection(&mut self) {}
}

// ---------------------------------------------------------------------
// 4) Implementation of `StatContextRefs`
// ---------------------------------------------------------------------
impl<'a> StatContextRefs<'a> {
    /// Build a StatContextRefs by scanning an entity's definitions/context
    /// and storing them in a HardMap instead of a HashMap.
    pub fn build(
        entity: Entity,
        defs_query: &'a Query<'_, '_, &StatDefinitions>,
        ctx_query: &'a Query<'_, '_, &StatContext>,
    ) -> StatContextRefs<'a> {
        // Create a HardMap with default NoContext in each slot
        let mut hard_map = HardMap::new();

        // If the entity itself has definitions, store them under the "This" slot
        if let Ok(defs) = defs_query.get(entity) {
            hard_map.set(StatContextType::This, StatContextRefs::Definitions(defs));
        }

        // If the entity has a StatContext, build subcontexts for each known key
        if let Ok(stat_context) = ctx_query.get(entity) {
            for (key, child_entity) in &stat_context.sources {
                // Avoid infinite recursion if an entity references itself
                if *child_entity == entity {
                    continue;
                }
                // Recursively build the child subcontext
                let child_src = Self::build(*child_entity, defs_query, ctx_query);

                // Match the child key to one of our 3 slots
                match key.as_str() {
                    "self"   => hard_map.set(StatContextType::This, child_src),
                    "parent" => hard_map.set(StatContextType::Parent, child_src),
                    "target" => hard_map.set(StatContextType::Target, child_src),
                    // If you have more “hard-coded” slots, handle them here
                    _ => {
                        // Or ignore unknown keys
                    }
                }
            }
        }

        // Return a SubContext if we stored anything
        StatContextRefs::SubContext(Box::new(hard_map))
    }

    /// Public getter that splits on '.' and calls `get_parts` recursively
    pub fn get(&self, var: &str) -> Result<f32, StatError> {
        let parts: Vec<&str> = var.split('.').collect();
        self.get_parts(&parts)
    }

    fn get_parts(&self, parts: &[&str]) -> Result<f32, StatError> {
        if parts.is_empty() {
            return Err(StatError::NotFound("Empty stat identifier".to_string()));
        }

        match self {
            // ================ 1) This is a "leaf" that has definitions ================
            StatContextRefs::Definitions(defs) => {
                if parts.len() == 1 {
                    // e.g. "Life"
                    defs.get_str(parts[0], self)
                } else {
                    // e.g. "Life.max" => let definitions parse the dot
                    let joined = parts.join(".");
                    defs.get_str(&joined, self)
                }
            }

            // ================ 2) This is a "branch" that has a HardMap ================
            StatContextRefs::SubContext(hard_map) => {
                let head = parts[0];
                // If the "head" starts uppercase, treat the entire string as a single stat in "self"
                if Self::is_stat_name_segment(head) {
                    let joined = parts.join(".");
                    if let StatContextRefs::Definitions(defs) = hard_map.get(StatContextType::This) {
                        return defs.get_str(&joined, self);
                    } else {
                        return Err(StatError::NotFound(
                            format!("No 'self' definitions to handle stat {:?}", joined)
                        ));
                    }
                }

                // If we only have 1 part and it's lowercase, e.g. "parent", that's incomplete
                if parts.len() == 1 {
                    return Err(StatError::NotFound(format!(
                        "Got a single-lowercase-part {:?}, but no stat name was provided",
                        head
                    )));
                }

                let tail = &parts[1..];

                // Look up the subcontext for `head` in the HardMap
                match hard_map.get_by_str(head) {
                    Some(StatContextRefs::Definitions(defs)) => {
                        // e.g. "parent.Strength" => tail has 1 item = "Strength"
                        if tail.len() == 1 {
                            defs.get_str(tail[0], self)
                        } else {
                            let joined = tail.join(".");
                            defs.get_str(&joined, self)
                        }
                    }
                    Some(StatContextRefs::SubContext(child_map)) => {
                        // e.g. "parent.parent.XYZ"
                        if tail.is_empty() {
                            return Err(StatError::NotFound("Empty tail".to_string()));
                        }
                        let head2 = tail[0];
                        let tail2 = &tail[1..];

                        if Self::is_stat_name_segment(head2) {
                            // e.g. "parent.parent.Life" => entire remainder is "Life"
                            let joined = tail.join(".");
                            if let StatContextRefs::Definitions(defs) = child_map.get(StatContextType::This) {
                                return defs.get_str(&joined, self);
                            } else {
                                return Err(StatError::NotFound(format!(
                                    "No 'self' in subcontext to handle stat: {}",
                                    joined
                                )));
                            }
                        } else {
                            // Recursively get from the child's subcontext
                            match child_map.get_by_str(head2) {
                                Some(child_src) => child_src.get_parts(tail2),
                                None => Err(StatError::NotFound(format!(
                                    "No subcontext for '{head2}'"
                                ))),
                            }
                        }
                    }
                    Some(StatContextRefs::NoContext) | None => {
                        Err(StatError::NotFound(format!(
                            "Key '{head}' not found among subcontext"
                        )))
                    }
                }
            }

            // ================ 3) This slot is empty (NoContext) ================
            StatContextRefs::NoContext => {
                Err(StatError::NotFound(
                    format!("NoContext: cannot resolve parts: {:?}", parts)
                ))
            }
        }
    }

    /// E.g. "Life", "Juice" start uppercase => treat them as a top-level stat name
    fn is_stat_name_segment(segment: &str) -> bool {
        segment
            .chars()
            .next()
            .map(char::is_uppercase)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StatContextType {
    This   = 0,
    Parent = 1,
    Target = 2,
}

impl StatContextType {
    pub fn idx(&self) -> usize {
        *self as usize
    }
}