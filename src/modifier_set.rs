use bevy::{prelude::*, utils::HashMap};
use crate::prelude::{StatAccessor, ValueType};

#[derive(Clone, Deref, DerefMut)]
pub struct ModifierSet(HashMap<String, Vec<ValueType>>);

impl ModifierSet {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add<V: Into<ValueType>>(&mut self, stat_path: &str, value: V) {
        self.entry(stat_path.to_string())
            .or_insert_with(Vec::new)
            .push(value.into());
    }

    pub fn apply(&self, stat_accessor: &mut StatAccessor, target_entity: Entity) {
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.add_modifier_value(target_entity, stat, modifier.clone());
            }
        }
    }

    pub fn remove(&self, stat_accessor: &mut StatAccessor, target_entity: Entity) {
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.remove_modifier_value(target_entity, stat, modifier);
            }
        }
    }
}