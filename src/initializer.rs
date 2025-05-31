use bevy::{prelude::*, utils::HashSet};
use crate::prelude::{ModifierSet, StatsMutator, Stats};

/// A component used to initialize an entity's stats with a predefined `ModifierSet`
/// when this component is added to an entity.
///
/// Upon being added, an observer system (`apply_stats_initializer`) will automatically
/// apply all modifiers contained within the `modifiers` field to the entity's `Stats`
/// component. After successful application, the `StatsInitializer` component is
/// removed from the entity to ensure one-time initialization.
///
/// This component should typically be added alongside a `Stats` component.
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_gauge::prelude::*;
/// # fn system(mut commands: Commands) {
/// let mut player_initial_stats = ModifierSet::default();
/// player_initial_stats.add("Health.base", 100.0);
/// player_initial_stats.add("Mana.base", 50.0);
/// player_initial_stats.add("Strength.base", 10.0);
///
/// commands.spawn((
///     Stats::new(), // The entity needs a Stats component
///     StatsInitializer::new(player_initial_stats),
/// ));
/// # }
/// ```
#[derive(Component, Debug, Clone)]
#[require(Stats)]
pub struct StatsInitializer {
    /// The set of modifiers to be applied to the entity.
    pub modifiers: ModifierSet,
}

impl StatsInitializer {
    /// Creates a new `StatsInitializer` with the given `ModifierSet`.
    pub fn new(modifier_set: ModifierSet) -> Self {
        Self { modifiers: modifier_set }
    }
}

/// An observer system that applies the `ModifierSet` from a `StatsInitializer`
/// component to the entity's `Stats` when `StatsInitializer` is added.
///
/// After application, the `StatsInitializer` component is removed.
pub(crate) fn apply_stats_initializer(
    trigger: Trigger<OnAdd, StatsInitializer>,
    mut stats_mutator: StatsMutator,
    query_initializer: Query<&StatsInitializer>,
    mut commands: Commands,
) {
    let entity = trigger.entity();
    if let Ok(initializer) = query_initializer.get(entity) {
        initializer.modifiers.apply_to(&mut stats_mutator, entity);
        
        commands.entity(entity).remove::<StatsInitializer>();
    }
}