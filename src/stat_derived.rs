use bevy::ecs::entity::Entity;

use crate::prelude::{StatsMutator, Stats};

pub trait StatDerived {
    fn from_stats(stats: &Stats) -> Self;

    fn should_update(&self, stats: &Stats) -> bool;

    fn update_from_stats(&mut self, stats: &Stats);

    fn is_valid(stats: &Stats) -> bool;
}

pub trait WriteBack {
    fn write_back(&self, target_entity: Entity, stats_mutator: &mut StatsMutator);
    
    /// Returns true if this component should write back to stats.
    /// This should only return true if the writeback-specific fields have actually changed
    /// from what's currently stored in the stats system.
    fn should_write_back(&self, target_entity: Entity, stats_mutator: &StatsMutator) -> bool;
}