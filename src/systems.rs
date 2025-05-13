use bevy::prelude::*;
use super::prelude::*;

pub(crate) fn add_stat_component_system<T: StatDerived + Component>(
    stats_query: Query<Entity, (Dirty<Stats>, Without<T>)>,
    stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    for entity in stats_query.iter() {
        let Ok(stats) = stat_accessor.get_stats(entity) else {
            continue;
        };
        if T::is_valid(stats) {
            commands.entity(entity).insert(T::from_stats(stats));
        }
    }
}

pub(crate) fn update_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<(Entity, &mut T), Dirty<Stats>>,
    stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    for (entity, mut stat_component) in stats_query.iter_mut() {
        let Ok(stats) = stat_accessor.get_stats(entity) else {
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

pub(crate) fn update_writeback_value_system<T: WriteBack + Component>(
    stats_query: Query<(Entity, &T), Dirty<T>>,
    mut stat_accessor: StatAccessor,
) {
    for (entity, write_back) in stats_query.iter() {
        write_back.write_back(entity, &mut stat_accessor);
    }
}