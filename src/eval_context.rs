use bevy::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_utils::HashMap;
use crate::{prelude::*, stat_effect::InstantStatEffectInstance};

#[derive(Debug)]
pub enum StatContextRefs<'a> {
    Definitions(&'a Stats),
    SubContext(Box<HardMap<'a>>),
}

#[derive(Debug)]
pub struct HardMap<'a> {
    this: Option<StatContextRefs<'a>>,
    parent: Option<StatContextRefs<'a>>,
    target: Option<StatContextRefs<'a>>,
}

impl<'a> HardMap<'a> {
    pub fn new() -> Self {
        Self {
            this: None,
            parent: None,
            target: None,
        }
    }

    pub fn set(&mut self, key: &str, val: StatContextRefs<'a>) {
        match key {
            "self"   => self.this = Some(val),
            "parent" => self.parent = Some(val),
            "target" => self.target = Some(val),
            _        => (),
        }
    }

    pub fn get(&self, key: &str) -> &Option<StatContextRefs<'a>> {
        let result = match key {
            "self"   => &self.this,
            "parent" => &self.parent,
            "target" => &self.target,
            _        => &None,
        };

        return result;
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

impl<'a> StatContextRefs<'a> {
    /// Build a StatContextRefs by scanning an entity's definitions/context
    /// and storing them in a HardMap instead of a HashMap.
    pub fn build(
        entity: Entity,
        defs_query: &'a Query<'_, '_, &mut Stats>,
        ctx_query: &'a Query<'_, '_, &StatContext>,
    ) -> StatContextRefs<'a> {
        // Create a HardMap with default NoContext in each slot
        let mut hard_map = HardMap::new();

        // If the entity itself has definitions, store them under the "This" slot
        if let Ok(defs) = defs_query.get(entity) {
            hard_map.set("self", StatContextRefs::Definitions(defs));
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
                hard_map.set(key, child_src);
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
                    if let Some(StatContextRefs::Definitions(defs)) = hard_map.get("self") {
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
                match hard_map.get(head) {
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
                            if let Some(StatContextRefs::Definitions(defs)) = child_map.get("self") {
                                return defs.get_str(&joined, self);
                            } else {
                                return Err(StatError::NotFound(format!(
                                    "No 'self' in subcontext to handle stat: {}",
                                    joined
                                )));
                            }
                        } else {
                            // Recursively get from the child's subcontext
                            match child_map.get(head2) {
                                Some(child_src) => child_src.get_parts(tail2),
                                None => Err(StatError::NotFound(format!(
                                    "No subcontext for '{head2}'"
                                ))),
                            }
                        }
                    }
                    _ => {
                        Err(StatError::NotFound(format!(
                            "Key '{head}' not found among subcontext"
                        )))
                    }
                }
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

#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    definitions: Query<'w, 's, &'static mut Stats>,
    contexts: Query<'w, 's, &'static StatContext>,
}

impl StatAccessor<'_, '_> {
    pub fn build(&self, entity: Entity) -> StatContextRefs {
        StatContextRefs::build(entity, &self.definitions, &self.contexts)
    }

    pub fn apply_effect(&mut self, entity: Entity, effect: &InstantStatEffectInstance) {
        let Ok(mut stats) = self.definitions.get_mut(entity) else {
            return;
        };

        for (stat, value) in effect.effects.iter() {
            let _ = stats.add(stat, *value);
        }
    }
}