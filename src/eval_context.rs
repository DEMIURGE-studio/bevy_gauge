use bevy::prelude::*;
use bevy_utils::HashMap;
use crate::prelude::*;

#[derive(Debug)]
pub enum StatContextRefs<'a> {
    Definitions(&'a StatDefinitions),
    SubContext(HashMap<&'a str, StatContextRefs<'a>>),
}

impl<'a> StatContextRefs<'a> {
    /// Look up a stat like `"parent.Strength.max"` by splitting on '.' and recursing.
    pub fn get(&self, var: &str) -> Result<f32, StatError> {
        let parts: Vec<&str> = var.split('.').collect();
        self.get_parts(&parts)
    }

    fn is_stat_name_segment(segment: &str) -> bool {
        // For example, check if the first char is uppercase
        // If empty, treat it as not a stat name
        segment
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
    }

    fn get_parts(&self, parts: &[&str]) -> Result<f32, StatError> {
        if parts.is_empty() {
            return Err(StatError::NotFound("Empty stat identifier".to_string()));
        }

        match self {
            StatContextRefs::Definitions(defs) => {
                // If there's only one part, e.g. "Life", look it up directly.
                if parts.len() == 1 {
                    return defs.get_str(parts[0], self);
                } else {
                    // e.g. "Life.max" => let the definitions parse the dot
                    let joined = parts.join(".");
                    return defs.get_str(&joined, self);
                }
            }
            StatContextRefs::SubContext(sub_map) => {
                // First, check if the "head" is uppercase => treat as direct stat in "self"
                let head = parts[0];
                
                // If "head" is uppercase (like "Life" or "Juice"), 
                // we interpret *all* the parts as a single stat name: "Life.max"
                if Self::is_stat_name_segment(head) {
                    let joined = parts.join(".");
                    // Attempt to look up Definitions under "self" or any other fallback
                    // you might want. For example:
                    if let Some(StatContextRefs::Definitions(defs)) = sub_map.get("self") {
                        return defs.get_str(&joined, self);
                    } else {
                        // or you might want to return an error if there's no "self"
                        return Err(StatError::NotFound(
                            format!("No 'self' context in SubContext for stat: {:?}", joined),
                        ));
                    }
                }

                // Otherwise, if "head" is lowercase, treat it as a subcontext key
                // If there is only one part (like "parent"), we try "parent" as a context key
                // but that still might not make sense if there's no stat after it.
                if parts.len() == 1 {
                    // We only have e.g. "parent". That by itself doesn't pick a stat, 
                    // so you might choose to do:
                    return Err(StatError::NotFound(format!(
                        "Got a single-lowercase-part {:?}, but no stat name was provided",
                        head
                    )));
                }

                let tail = &parts[1..];

                // Try to find the subcontext for `head`
                match sub_map.get(head) {
                    Some(StatContextRefs::Definitions(defs)) => {
                        // If there's exactly one part left in `tail`, 
                        // e.g. "parent.Strength", we can do:
                        if tail.len() == 1 {
                            defs.get_str(tail[0], self)
                        } else {
                            // e.g. "parent.Strength.max"
                            let joined = tail.join(".");
                            defs.get_str(&joined, self)
                        }
                    }
                    Some(StatContextRefs::SubContext(child_map)) => {
                        // We still have multiple parts, so we continue recursing
                        if tail.is_empty() {
                            return Err(StatError::NotFound("Empty tail".to_string()));
                        }

                        let head2 = tail[0];
                        let tail2 = &tail[1..];

                        // If that next part is uppercase, treat it as a stat
                        if Self::is_stat_name_segment(head2) {
                            let joined = tail.join(".");
                            if let Some(StatContextRefs::Definitions(defs)) = child_map.get("self")
                            {
                                return defs.get_str(&joined, self);
                            } else {
                                return Err(StatError::NotFound(format!(
                                    "No 'self' in subcontext to handle stat: {}",
                                    joined
                                )));
                            }
                        } else {
                            // It's still a subcontext key => proceed similarly
                            match child_map.get(head2) {
                                Some(child_src) => child_src.get_parts(tail2),
                                None => Err(StatError::NotFound(format!(
                                    "No subcontext for {:?}",
                                    head2
                                ))),
                            }
                        }
                    }
                    None => Err(StatError::NotFound(format!(
                        "Key {:?} not found among SubContext keys",
                        head
                    ))),
                }
            }
        }
    }

    pub fn build(
        entity: Entity,
        stats_query: &'a Query<'_, '_, &StatDefinitions>,
        ctx_query: &'a Query<'_, '_, &StatContext>,
    ) -> StatContextRefs<'a> {
        // We'll create a map for all sub-entries of this entity.
        let mut map = HashMap::default();
    
        // If the entity itself has definitions, we store them under “self”.
        if let Ok(defs) = stats_query.get(entity) {
            map.insert("self", StatContextRefs::Definitions(defs));
        }
    
        // If the entity has a StatContext, build subcontext entries for each.
        if let Ok(stat_context) = ctx_query.get(entity) {
            for (key, child_entity) in &stat_context.sources {
                if *child_entity == entity {
                    continue;
                }
                // Build a sub tree for that child.
                let child_source_ref = Self::build(*child_entity, stats_query, ctx_query);
                map.insert(key.as_str(), child_source_ref);
            }
        }
    
        // If we ended up with an empty map, then there's no data at all. Return None.
        // Otherwise, it's a "branch" node storing those sub references.
        StatContextRefs::SubContext(map)
    }
}

// Your data container, storing Entities only
#[derive(Component, Default)]
pub struct StatContext {
    sources: HashMap<String, Entity>,
}

impl StatContext {
    pub fn insert(&mut self, context: &str, entity: Entity) {
        self.sources.insert(context.to_string(), entity);
    }

    pub fn trigger_change_detection(&mut self) {}
}

pub enum StatContextType {
    This,
    Parent,
    Target,
}