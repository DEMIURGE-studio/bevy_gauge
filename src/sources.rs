use bevy::prelude::*;
use crate::prelude::{StatAccessor, Stats};

pub fn register_parent(
    parent_query: Query<(Entity, &Parent), (With<Stats>, Changed<Parent>)>,
    mut stat_accessor: StatAccessor,
) {
    for (entity, parent) in parent_query.iter() {
        stat_accessor.register_source(entity, "Parent", parent.get());
    }
}