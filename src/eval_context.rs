use bevy::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_utils::hashbrown::HashMap;
use crate::{prelude::*, stat_effect::InstantStatEffectInstance};

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

#[derive(Debug)]
pub enum StatContextRefs<'a> {
    Definitions(&'a Stats),
    SubContext(Box<HashMap<&'a str, StatContextRefs<'a>>>),
}

impl<'a> StatContextRefs<'a> {
    pub fn build(
        entity: Entity,
        defs_query: &'a Query<'_, '_, &mut Stats>,
        ctx_query: &'a Query<'_, '_, &StatContext>,
    ) -> StatContextRefs<'a> {
        // Create a HardMap with default NoContext in each slot
        let mut context_map = HashMap::new();

        // If the entity itself has definitions, store them under the "This" slot
        if let Ok(defs) = defs_query.get(entity) {
            context_map.insert("self", StatContextRefs::Definitions(defs));
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
                context_map.insert(key, child_src);
            }
        }

        // Return a SubContext if we stored anything
        StatContextRefs::SubContext(Box::new(context_map))
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
            // ================ This is a "leaf" that has definitions ================
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

            // ================ This is a "branch" that has a hashmap context ================
            StatContextRefs::SubContext(context_map) => {
                let head = parts[0];
                let tail = &parts[1..];
    
                // If head is a context key (e.g., "root", "parent", "target", etc.), delegate lookup
                if let Some(subcontext) = context_map.get(head) {
                    return subcontext.get_parts(tail);
                }
    
                // If no explicit context, assume it's a stat lookup under "self"
                if let Some(StatContextRefs::Definitions(defs)) = context_map.get("self") {
                    let stat_name = parts.join(".");
                    return defs.get_str(&stat_name, self);
                }
    
                Err(StatError::NotFound(format!("Context '{}' not found", head)))
            }
        }
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
    
    pub fn build_with_target(&self, entity: Entity, target: Entity) -> StatContextRefs {
        let target_context = StatContextRefs::build(entity, &self.definitions, &self.contexts);
        let mut value = StatContextRefs::build(target, &self.definitions, &self.contexts);
    
        // Match by reference, so `value` is not consumed
        if let StatContextRefs::SubContext(ref mut hash_map) = value {
            hash_map.insert("target", target_context);
        }
    
        value
    }

    pub fn apply_effect(&mut self, origin: Entity, target: Entity, stat_effect: &StatEffect) {
        let effect_instance = {
            let stat_context = self.build(origin);
            stat_effect.build_instant(&stat_context)
        };

        self.apply_instant_effect(target, &effect_instance);
    }

    pub fn apply_instant_effect(&mut self, entity: Entity, effect: &InstantStatEffectInstance) {
        let Ok(mut stats) = self.definitions.get_mut(entity) else {
            return;
        };

        for (stat, value) in effect.effects.iter() {
            let _ = stats.add(stat, *value);
        }
    }
}