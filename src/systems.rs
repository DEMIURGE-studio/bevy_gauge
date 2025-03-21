use bevy::prelude::*;
use super::prelude::*;

pub(crate) fn add_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<Entity, Without<T>>,
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

pub(crate) fn update_stat_component_system<T: StatDerived + Component<Mutability = bevy_ecs::component::Mutable>>(
    mut stats_query: Query<(Entity, &mut T)>,
    stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    for (entity, mut stat_component) in stats_query.iter_mut() {
        let stats = stat_accessor.build(entity);
        if stat_component.should_update(&stats) {
            stat_component.update_from_stats(&stats);
        }
        if !T::is_valid(&stats) {
            commands.entity(entity).remove::<T>();
        }
    }
}

pub(crate) fn update_writeback_value_system<T: WriteBack + Component>(
    mut stats_query: Query<(&mut StatCollection, &T)>,
) {
    for (mut stat_component, writeback) in stats_query.iter_mut() {
        writeback.write_back(&mut stat_component);
    }
}