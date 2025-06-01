use bevy::prelude::*;
use super::prelude::*;

pub(crate) fn add_stat_component_system<T: StatDerived + Component>(
    stats_query: Query<Entity, (Dirty<Stats>, Without<T>)>,
    stats_mutator: StatsMutator,
    mut commands: Commands,
) {
    for entity in stats_query.iter() {
        let Ok(stats) = stats_mutator.get_stats(entity) else {
            continue;
        };
        if T::is_valid(stats) {
            commands.entity(entity).insert(T::from_stats(stats));
        }
    }
}

pub(crate) fn update_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<(Entity, &mut T), Dirty<Stats>>,
    stats_mutator: StatsMutator,
    mut commands: Commands,
) {
    for (entity, mut stat_component) in stats_query.iter_mut() {
        let Ok(stats) = stats_mutator.get_stats(entity) else {
            continue;
        };
        if stat_component.should_update(stats) {
            stat_component.update_from_stats(stats);
        }
        if !T::is_valid(stats) {
            commands.entity(entity).remove::<T>();
        }
    }
}

/// Generic system for resolving writeback conflicts for a specific component type.
/// This should be scheduled in the Resolution schedule for each WriteBack component.
/// Only processes components that have actually changed to avoid infinite Changed loops.
pub(crate) fn resolve_writeback_component_system<T: WriteBack + Component>(
    mut component_query: Query<(Entity, &mut T), Changed<T>>,
    mut stats_mutator: StatsMutator,
) {
    for (entity, mut component) in component_query.iter_mut() {
        if component.should_write_back(entity, &stats_mutator) {
            component.write_back(entity, &mut stats_mutator);
        }
    }
} 