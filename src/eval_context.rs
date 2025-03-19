use bevy::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_utils::HashMap;
use crate::{prelude::*, stat_effect::InstantStatEffectInstance};

#[derive(Debug)]
pub struct HardMap<'a> {
    this: Option<StatContextRefs<'a>>,
    parent: Option<StatContextRefs<'a>>,
    root: Option<StatContextRefs<'a>>,
    target: Option<StatContextRefs<'a>>,
}

impl<'a> HardMap<'a> {
    pub fn new() -> Self {
        Self {
            this: None,
            parent: None,
            root: None,
            target: None,
        }
    }

    pub fn set(&mut self, key: &str, val: StatContextRefs<'a>) {
        match key {
            "self"   => self.this = Some(val),
            "parent" => self.parent = Some(val),
            "root" => self.root = Some(val),
            "target" => self.target = Some(val),
            _        => (),
        }
    }

    pub fn get(&self, key: &str) -> &Option<StatContextRefs<'a>> {
        let result = match key {
            "self"   => &self.this,
            "parent" => &self.parent,
            "root" => &self.root,
            "target" => &self.target,
            _        => &None,
        };

        return result;
    }
}

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
    SubContext(Box<HardMap<'a>>),
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
        self.get_parts(&var.split('.').collect::<Vec<&str>>())
    }

    pub fn get_taggable(&self, stat_name: &str, tag_mask: u32) -> Result<f32, StatError> {
        match self {
            StatContextRefs::Definitions(defs) => defs.get_taggable(stat_name, tag_mask, self),
            StatContextRefs::SubContext(hard_map) => {
                if let Some(StatContextRefs::Definitions(defs)) = hard_map.get("self") {
                    defs.get_taggable(stat_name, tag_mask, self)
                } else {
                    Err(StatError::NotFound(format!(
                        "No 'self' definitions to handle stat '{stat_name}[{tag_mask}]'"
                    )))
                }
            }
        }
    }

    fn get_parts(&self, parts: &[&str]) -> Result<f32, StatError> {
        if parts.is_empty() {
            return Err(StatError::NotFound("Empty stat identifier".to_string()));
        }
    
        match self {
            // ================ 1) If this is a Definitions Leaf =================
            StatContextRefs::Definitions(defs) => {
                if parts.len() == 1 {
                    let key = parts[0];
    
                    // Handle Taggable Queries (e.g., "Damage[fire, spell]")
                    if let Some((stat_name, tag_mask)) = parse_tagged_stat(key) {
                        return defs.get_taggable(stat_name, tag_mask, self);
                    }
    
                    return defs.get_str(key, self);
                } else {
                    let joined = parts.join(".");
                    return defs.get_str(&joined, self);
                }
            }
    
            // ================ 2) If this is a Branch with Contexts ================
            StatContextRefs::SubContext(hard_map) => {
                let head = parts[0];
                let tail = &parts[1..];
    
                if let Some(subcontext) = hard_map.get(head) {
                    return subcontext.get_parts(tail);
                }
    
                // If the head is uppercase, assume it's a stat in "self"
                if Self::is_stat_name_segment(head) {
                    let joined = parts.join(".");
                    if let Some(subcontext) = hard_map.get("self") {
                        return subcontext.get_parts(&[&joined]);
                    } else {
                        return Err(StatError::NotFound(
                            format!("No 'self' definitions to handle stat {:?}", joined)
                        ));
                    }
                }
    
                // If parts.len() == 1, it's an incomplete path like "parent" without a stat name
                if parts.len() == 1 {
                    return Err(StatError::NotFound(format!(
                        "Got a single-lowercase-part {:?}, but no stat name was provided",
                        head
                    )));
                }
    
                Err(StatError::NotFound(format!("Key '{}' not found in context", head)))
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
    
    pub fn build_with_target(&self, entity: Entity, target: Entity) -> StatContextRefs {
        let target_context = StatContextRefs::build(entity, &self.definitions, &self.contexts);
        let mut value = StatContextRefs::build(target, &self.definitions, &self.contexts);
    
        // Match by reference, so `value` is not consumed
        if let StatContextRefs::SubContext(ref mut hard_map) = value {
            hard_map.set("target", target_context);
        }
    
        value
    }

    pub fn apply_effect(&mut self, entity: Entity, effect: &StatEffect) {
        let stat_context = self.build(entity);

        let effect = effect.build_instant(&stat_context);

        self.apply_instant_effect(entity, &effect);
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

fn parse_tagged_stat(var: &str) -> Option<(&str, u32)> {
    if let Some(start) = var.find('[') {
        if let Some(end) = var.find(']') {
            let base_stat = &var[..start]; // "Damage"
            let tag_str = &var[start + 1..end]; // "fire, spell"

            println!("Parsing stat: {} with tags: {}", base_stat, tag_str);

            let mut tag_mask = 0;
            for tag in tag_str.split(',').map(|s| s.trim()) {
                tag_mask |= match_tag(tag);
            }

            return Some((base_stat, tag_mask));
        }
    }
    None
}

fn match_tag(var: &str) -> u32 {
    match var {
        "fire"          => 0b00000001,
        "ice"           => 0b00000010,
        "lightning"     => 0b00000100,
        "physical"      => 0b00001000,
        "elemental"     => 0b00000111,
        "any_damage"    => 0b00001111,

        "spell"         => 0b00010000,
        "attack"        => 0b00100000,
        "ranged"        => 0b01000000,
        "melee"         => 0b10000000,
        "any_type"      => 0b11110000,
        _               => 0b00000000,
    }
}