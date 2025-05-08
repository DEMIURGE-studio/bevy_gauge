use bevy::{prelude::*, utils::HashMap};
use crate::prelude::{StatAccessor, StatEffect, ModifierType};

#[derive(Component, Clone, Debug, Default, Deref, DerefMut)]
pub struct ModifierSet(HashMap<String, Vec<ModifierType>>);

impl ModifierSet {
    pub fn new(modifiers: HashMap<String, Vec<ModifierType>>) -> Self {
        Self(modifiers)
    }

    pub fn add<V: Into<ModifierType>>(&mut self, path: &str, value: V) {
        self.entry(path.to_string())
            .or_insert_with(Vec::new)
            .push(value.into());
    }
}

impl StatEffect for ModifierSet {
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = context;
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.add_modifier_value(*target_entity, stat, modifier.clone());
            }
        }
    }

    fn remove(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = context;
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.remove_modifier_value(*target_entity, stat, modifier);
            }
        }
    }
}