use bevy::{app::MainScheduleOrder, prelude::*};
use bevy_ecs::schedule::ScheduleLabel;

/// Order goes StatsUpdate -> CompositeStatsUpdate -> SideEffectsUpdate
/// All of this happens in PreUpdate so that by Update stats are ready to be
/// used.
/// 
/// If a system manipulates Stats, it should be inside of StatsUpdate.
pub fn plugin(app: &mut App) {
    app.init_schedule(StatsReady)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_before(Update, StatsReady);

    app.init_schedule(SideEffectsUpdate)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_before(StatsReady, SideEffectsUpdate);

    app.init_schedule(CompositeStatsUpdate)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_before(SideEffectsUpdate, CompositeStatsUpdate);

    app.init_schedule(StatsUpdate)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_before(CompositeStatsUpdate, StatsUpdate);
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsUpdate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CompositeStatsUpdate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SideEffectsUpdate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsReady;