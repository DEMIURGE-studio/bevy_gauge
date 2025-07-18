use bevy::{ecs::component::Mutable, prelude::*};
use super::prelude::*;

pub(crate) fn update_stat_component_system<T: StatDerived + Component<Mutability = Mutable>>(
    mut stats_query: Query<(Entity, &mut T), Changed<StatsProxy>>,
    stats_mutator: StatsMutator,
) {
    for (entity, mut stat_component) in stats_query.iter_mut() {
        let Ok(stats) = stats_mutator.get_stats(entity) else {
            continue;
        };
        if stat_component.should_update(stats) {
            stat_component.update_from_stats(stats);
        }
    }
}

pub(crate) fn update_writeback_value_system<T: WriteBack + Component>(
    stats_query: Query<(Entity, &T), Changed<T>>,
    mut stats_mutator: StatsMutator,
) {
    for (entity, write_back) in stats_query.iter() {
        if write_back.should_write_back(entity, &stats_mutator) {
            write_back.write_back(entity, &mut stats_mutator);
        }
    }
}