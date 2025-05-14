use super::prelude::*;

/// Defines the fundamental behavior and interface for all stat types within the system.
///
/// This trait is implemented by the various internal stat representations (e.g., `Flat`, `Modifiable`, `Tagged`)
/// to provide a consistent way for the `Stats` component and `StatAccessor` to interact with them.
/// It covers creation, initialization, modifier application, direct setting, and value evaluation.
pub trait Stat {
    /// Creates a new instance of a stat type based on the provided path and configuration.
    ///
    /// This is called when a stat is first accessed or defined for an entity if it doesn't already exist.
    /// The `config` is used to determine the specific kind of stat (e.g., Flat, Tagged) and its properties.
    ///
    /// # Arguments
    ///
    /// * `path`: The `StatPath` identifying the stat being created.
    /// * `config`: A reference to the global `Config` resource.
    fn new(path: &StatPath, config: &Config) -> Self;

    /// Called after a stat is first created and added to an entity's `Stats` component.
    /// Allows for any type-specific initialization logic that might require access to the `Stats` component itself.
    /// The default implementation does nothing.
    ///
    /// # Arguments
    ///
    /// * `_path`: The `StatPath` of the stat being initialized.
    /// * `_stats`: A mutable reference to the parent `Stats` component.
    fn initialize(&self, _path: &StatPath, _stats: &mut Stats) {}

    /// Adds a modifier to this stat.
    ///
    /// The specifics of how the modifier is stored and applied depend on the implementing stat type.
    ///
    /// # Arguments
    ///
    /// * `path`: The `StatPath` indicating which specific part of the stat (if applicable) the modifier targets.
    /// * `modifier`: The `ModifierType` (literal or expression) to add.
    /// * `config`: A reference to the global `Config` resource, which might be needed to determine modifier behavior.
    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType, config: &Config);

    /// Removes a previously added modifier from this stat.
    ///
    /// The modifier to be removed should match one that was added earlier.
    ///
    /// # Arguments
    ///
    /// * `path`: The `StatPath` indicating where the modifier was applied.
    /// * `modifier`: A reference to the `ModifierType` to remove.
    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType);

    /// Directly sets a value for a stat or a part of it, potentially overwriting existing values or base amounts.
    /// The exact behavior is type-dependent. For simple stats like `Flat`, this might set its only value.
    /// For more complex stats, this might target a specific component like a base value.
    /// The default implementation does nothing.
    ///
    /// # Arguments
    ///
    /// * `_path`: The `StatPath` indicating which stat or part to set.
    /// * `_value`: The `f32` value to set.
    fn set(&mut self, _path: &StatPath, _value: f32) {}

    /// Evaluates the final value of this stat, considering all its current modifiers and internal logic.
    ///
    /// # Arguments
    ///
    /// * `path`: The `StatPath` specifying which aspect of the stat to evaluate (e.g., total value, a specific tagged part).
    /// * `stats`: A reference to the parent `Stats` component, providing context (like cached values or source entity data)
    ///            that might be needed for evaluation.
    ///
    /// # Returns
    ///
    /// An `f32` representing the calculated value of the stat.
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32;

    /// Clears any internal caches that the stat might hold, potentially for a specific path.
    /// This is useful when underlying data changes and cached evaluations need to be invalidated.
    /// The default implementation does nothing.
    fn clear_internal_cache(&mut self, _path: &StatPath) { /* default no-op */ }
}