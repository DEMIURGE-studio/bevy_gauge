use bevy::prelude::*;
use crate::prelude::{StatsMutator, Stats};

pub fn register_parent(
    parent_query: Query<(Entity, &ChildOf), (With<Stats>, Changed<ChildOf>)>,
    mut stats_mutator: StatsMutator,
) {
    for (entity, parent) in parent_query.iter() {
        stats_mutator.register_source(entity, "Parent", parent.parent());
    }
}