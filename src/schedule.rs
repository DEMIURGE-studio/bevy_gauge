use bevy::{app::MainScheduleOrder, prelude::*};
use bevy_ecs::schedule::ScheduleLabel;

/// Order goes StatsUpdate -> CompositeStatsUpdate -> SideEffectsUpdate
/// All of this happens in PreUpdate so that by Update stats are ready to be
/// used.
/// 
/// If a system manipulates Stats, it should be inside of StatsUpdate.
/// 
/// We could support "..WriteBack" style stuff with proper scheduling
/// Say we processed writeback components and their changes in one step
/// and then processed Stat changes in another. 
/// 
/// StatComponentUpdate - Life.current updated
/// > WriteBackWrite - Life.current written to "Life.current"
/// StatsUpdate - "Life.current" updated
/// > StatsUpdateWrite - "Life.current" written to Life.current
/// StatsReady - Canonical value for "Life.current" and Life.current available
///     for use elsewhere
/// 
/// All of this allows you to treat "Life.current" and Life.current as the
/// same value. This is handy because we might want to handle some effects 
/// via the components value like dealing damage via a hit. We might also 
/// want to be able to access "Life.current" via a 1-shot stat effect.
/// 
/// Gotta think on it.
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