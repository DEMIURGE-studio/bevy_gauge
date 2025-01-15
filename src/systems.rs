use bevy::prelude::*;
use super::prelude::*;

pub(crate) fn add_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<(Entity, &Stats), (Changed<Stats>, Without<T>)>,
    mut commands: Commands,
) {
    for (entity, stats) in stats_query.iter_mut() {
        if T::is_valid(stats) {
            commands.entity(entity).insert(T::from_stats(stats));
        }
    }
}

pub(crate) fn update_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<(Entity, &Stats, &mut T), Changed<Stats>>,
    mut commands: Commands,
) {
    for (entity, stats, mut stat_component) in stats_query.iter_mut() {
        stat_component.update_from_stats(stats);
        if !T::is_valid(stats) {
            commands.entity(entity).remove::<T>();
        }
    }
}