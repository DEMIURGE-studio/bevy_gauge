use bevy::{app::MainScheduleOrder, prelude::*};
use bevy::ecs::schedule::ScheduleLabel;

/// Custom Bevy schedule label for systems that perform mutations on `Stats` components.
///
/// This schedule runs after `PreUpdate` and before `UpdateStatDerived`.
/// It's intended for systems that directly add/remove modifiers or change stat values.
///
/// This is also the correct schedule to modify stat sources (register/unregister),
/// including relationship-driven wiring via the `register_stat_relationship*` APIs.
/// Source changes must occur here so dependents evaluate correctly in later stages.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsMutation;

/// Custom Bevy schedule label for systems that update components implementing `StatDerived`.
///
/// This schedule runs after `StatsMutation` and before `UpdateWriteBack` (and `Update`).
/// Systems in this schedule read from `Stats` components and update the fields of `StatDerived` components.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UpdateStatDerived;

/// Custom Bevy schedule label for systems that write values from `WriteBack` components back to `Stats` components.
///
/// This schedule runs after `UpdateStatDerived` and before `Update` (if `UpdateWriteBack` itself is placed before `Update`).
/// Systems here detect changes in `WriteBack` components and update the underlying stats.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UpdateWriteBack;

/// Plugin that initializes the custom schedules used by the stat system.
pub fn plugin(app: &mut App) {
    app.init_schedule(StatsMutation)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(First, StatsMutation);

    app.init_schedule(UpdateStatDerived)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(StatsMutation, UpdateStatDerived);

    app.init_schedule(UpdateWriteBack)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(UpdateStatDerived, UpdateWriteBack);
}