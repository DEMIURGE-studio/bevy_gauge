use bevy::ecs::{entity::Entity, system::Commands};
use crate::prelude::StatsMutator;

/// A trait for defining operations that apply or remove a collection of stat changes.
///
/// `StatEffect` is intended for types that encapsulate a set of modifications (like buffs, debuffs, item effects)
/// which can be applied to a target entity or context, and potentially removed later.
/// The `ModifierSet` struct is a primary implementor of this trait.
///
/// The trait is generic over a `Context` type, which allows effects to require different
/// information when being applied or removed. By default, the context is an `Entity`,
/// but can be customized for more complex scenarios (see `StatEffectContext`).
pub trait StatEffect {
    /// The context required by this effect when being applied or removed.
    /// Defaults to `Entity`, meaning the effect operates directly on a single target entity.
    type Context: StatEffectContext = Entity;
   
    /// Applies the stat effect to the given context using the provided `StatsMutator`.
    ///
    /// # Arguments
    ///
    /// * `stats_mutator`: A mutable reference to the `StatsMutator` to enact stat changes.
    /// * `context`: A reference to the context for this effect (e.g., the target `Entity`).
    fn apply(&self, stats_mutator: &mut StatsMutator, context: &Self::Context);

    /// Removes the stat effect from the given context using the provided `StatsMutator`.
    /// This method has a default empty implementation, as not all effects are removable
    /// or require explicit removal logic.
    ///
    /// # Arguments
    ///
    /// * `stats_mutator`: A mutable reference to the `StatsMutator`.
    /// * `context`: A reference to the context for this effect.
    fn remove(&self, _stats_mutator: &mut StatsMutator, _context: &Self::Context) {}
}

/// A marker trait for types that can serve as a context for `StatEffect` operations.
///
/// This trait allows `StatEffect` implementors to define custom context types if they need more
/// than just a target `Entity` to apply or remove their effects (e.g., requiring access to a source entity,
/// RNG, or Bevy `Commands`).
///
/// `Entity` itself implements this trait, making it the default context for simple effects.
pub trait StatEffectContext {}

impl StatEffectContext for Entity {}