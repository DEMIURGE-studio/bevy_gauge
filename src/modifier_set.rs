use bevy::{platform::collections::HashMap, prelude::*};
use crate::prelude::{StatsMutator, ModifierType};

/// A component that represents a collection of stat modifiers, typically grouped by a common source
/// like an item, buff, or skill.
///
/// `ModifierSet` allows defining multiple modifiers that can be applied or removed together.
///
/// It dereferences to `HashMap<String, Vec<ModifierType>>`, allowing direct manipulation
/// of the underlying map if needed, though `add` provides a convenient way to insert modifiers.
#[derive(Component, Clone, Debug, Default)]
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
    /// * `path`: The string representation of the stat path (e.g., "Life.base", "CritChance.added").
    /// * `value`: The modifier to add, convertible into `ModifierType` (e.g., `10.0f32` or an `Expression`).
    pub fn add<V: Into<ModifierType>>(&mut self, path: &str, value: V) {
        self.0.entry(path.to_string())
            .or_insert_with(Vec::new)
            .push(value.into());
    }

    /// Applies all modifiers contained in this `ModifierSet` to the target entity.
    ///
    /// This iterates through each stat path and its associated modifiers in the set,
    /// calling `StatsMutator::add_modifier_value` for each one on the target entity.
    ///
    /// # Arguments
    ///
    /// * `stats_mutator`: A mutable reference to the `StatsMutator` for applying modifiers.
    /// * `target_entity`: The entity to apply the modifiers to.
    pub fn apply_to(&self, stats_mutator: &mut StatsMutator, target_entity: Entity) {
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stats_mutator.add_modifier_value(target_entity, stat, modifier.clone());
            }
        }
    }

    /// Removes all modifiers contained in this `ModifierSet` from the target entity.
    ///
    /// This iterates through each stat path and its associated modifiers in the set,
    /// calling `StatsMutator::remove_modifier_value` for each one on the target entity.
    /// It assumes the modifiers were previously applied in a similar manner.
    ///
    /// # Arguments
    ///
    /// * `stats_mutator`: A mutable reference to the `StatsMutator` for removing modifiers.
    /// * `target_entity`: The entity to remove the modifiers from.
    pub fn remove_from(&self, stats_mutator: &mut StatsMutator, target_entity: Entity) {
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stats_mutator.remove_modifier_value(target_entity, stat, modifier);
            }
        }
    }
}