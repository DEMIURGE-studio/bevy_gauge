use bevy::prelude::*;
use super::prelude::*;

pub(crate) fn add_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<Entity, (Changed<StatDefinitions>, Without<T>)>,
    stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    for entity in stats_query.iter_mut() {
        let stats = stat_accessor.build(entity);
        if T::is_valid(&stats) {
            commands.entity(entity).insert(T::from_stats(&stats));
        }
    }
}

pub(crate) fn update_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<(Entity, &mut T), Changed<StatDefinitions>>,
    stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    for (entity, mut stat_component) in stats_query.iter_mut() {
        let stats = stat_accessor.build(entity);
        stat_component.update_from_stats(&stats);
        if !T::is_valid(&stats) {
            commands.entity(entity).remove::<T>();
        }
    }
}