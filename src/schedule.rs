use bevy::{app::MainScheduleOrder, ecs::schedule::ScheduleLabel, prelude::*};

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
    
    app.init_schedule(AddStatComponent)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(PreUpdate, AddStatComponent);
    
    app.init_schedule(StatComponentUpdate)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(AddStatComponent, StatComponentUpdate);

    app.init_schedule(StatComponentWrite)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(StatComponentUpdate, StatComponentWrite);

    app.init_schedule(StatsUpdate)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(StatComponentWrite, StatsUpdate);

    app.init_schedule(StatsWrite)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(StatsUpdate, StatsWrite);

    app.init_schedule(StatsReady)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(StatsWrite, StatsReady);
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AddStatComponent;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatComponentUpdate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StatComponentWrite;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsUpdate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StatsWrite;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsReady;