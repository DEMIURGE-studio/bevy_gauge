use std::collections::{HashMap, HashSet};
use crate::value_type::{Expression, StatValue, ValueBounds, ValueType};
use std::fmt::Debug;
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::Entity;
use crate::modifiers::{ModifierInstance, ModifierValue, ModifierValueTotal};

#[derive(Debug, Clone, Default)]
pub struct AttributeInstance {
    pub value: StatValue,
    pub modifier_collection: EntityHashMap<ModifierValue>,
    pub modifier_total: ModifierValueTotal,
    pub dependencies: Option<HashMap<String, HashSet<u32>>>,
    pub dependent_attributes: Option<HashMap<String, HashSet<u32>>>,
    pub dependent_modifiers: EntityHashSet
}

impl AttributeInstance {
    
    pub fn value(&self) -> &StatValue {
        &self.value
    }
    
    pub fn value_mut(&mut self) -> &mut StatValue {
        &mut self.value
    }
    
    pub fn get_value_f32(&self) -> f32 {
        self.value.get_value_f32()
    }

    pub fn add_or_replace_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        self.modifier_total += &modifier.value;
        self.modifier_collection.entry(modifier_entity).insert(modifier.value.clone());
    }

    pub fn remove_modifier(&mut self, modifier_entity: Entity) {
        let value = self.modifier_collection.remove(&modifier_entity);
        if let Some(value) = value {
            self.modifier_total -= &value; // decrement the modifier total
        }
        self.modifier_collection.remove(&modifier_entity);
    }
}

