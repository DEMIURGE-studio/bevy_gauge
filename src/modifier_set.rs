use bevy::{prelude::*, utils::HashMap};
use crate::prelude::{StatAccessor, StatEffect, ModifierType};

/// A component that represents a collection of stat modifiers, typically grouped by a common source
/// like an item, buff, or skill.
///
/// `ModifierSet` allows defining multiple modifiers that can be applied or removed together
/// from an entity using the `StatEffect` trait methods. This simplifies managing complex
/// sets of stat changes.
///
/// It dereferences to `HashMap<String, Vec<ModifierType>>`, allowing direct manipulation
/// of the underlying map if needed, though `add` provides a convenient way to insert modifiers.
#[derive(Component, Clone, Debug, Default, Deref, DerefMut)]
pub struct ModifierSet(HashMap<String, Vec<ModifierType>>);

impl ModifierSet {
    /// Creates a new `ModifierSet` from a pre-existing map of stat paths to modifier lists.
    ///
    /// # Arguments
    ///
    /// * `modifiers`: A `HashMap` where keys are stat path strings (e.g., "Damage.increased")
    ///                and values are `Vec<ModifierType>` containing the modifiers for that path.
    pub fn new(modifiers: HashMap<String, Vec<ModifierType>>) -> Self {
        Self(modifiers)
    }

    /// Adds a modifier to a specific stat path within this set.
    ///
    /// If the path does not already exist in the set, it will be created.
    /// The modifier is added to the list of modifiers for that path.
    ///
    /// # Arguments
    ///
    /// * `path`: The string representation of the stat path (e.g., "Health.base", "CritChance.added").
    /// * `value`: The modifier to add, convertible into `ModifierType` (e.g., `10.0f32` or an `Expression`).
    pub fn add<V: Into<ModifierType>>(&mut self, path: &str, value: V) {
        self.entry(path.to_string())
            .or_insert_with(Vec::new)
            .push(value.into());
    }
}

impl StatEffect for ModifierSet {
    /// Applies all modifiers contained in this `ModifierSet` to the target entity.
    ///
    /// This iterates through each stat path and its associated modifiers in the set,
    /// calling `StatAccessor::add_modifier_value` for each one on the target entity.
    ///
    /// # Arguments
    ///
    /// * `stat_accessor`: A mutable reference to the `StatAccessor` used to apply the modifiers.
    /// * `context`: A reference to the target `Entity` to which the modifiers will be applied.
    ///              (Note: `Self::Context` for `ModifierSet` is `Entity`).
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = context;
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.add_modifier_value(*target_entity, stat, modifier.clone());
            }
        }
    }

    /// Removes all modifiers contained in this `ModifierSet` from the target entity.
    ///
    /// This iterates through each stat path and its associated modifiers in the set,
    /// calling `StatAccessor::remove_modifier_value` for each one on the target entity.
    /// It assumes the modifiers were previously applied in a similar manner.
    ///
    /// # Arguments
    ///
    /// * `stat_accessor`: A mutable reference to the `StatAccessor` used to remove the modifiers.
    /// * `context`: A reference to the target `Entity` from which the modifiers will be removed.
    ///              (Note: `Self::Context` for `ModifierSet` is `Entity`).
    fn remove(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = context;
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.remove_modifier_value(*target_entity, stat, modifier);
            }
        }
    }
}