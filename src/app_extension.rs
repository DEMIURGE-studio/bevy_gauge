use std::marker::PhantomData;
use bevy::prelude::*;
use super::prelude::*;

/// An extension trait for `bevy::prelude::App` to simplify the setup of components
/// that derive their values from the stat system or write their values back to it.
///
/// This provides convenient methods to register the necessary systems for components
/// that implement `StatDerived` and/or `WriteBack`.
pub trait StatsAppExtension {
    /// Registers systems to manage a component `T` that implements `StatDerived`.
    ///
    /// This typically includes:
    /// - A system to add a `Stats` component to entities that have `T` but not `Stats`.
    /// - A system to update the fields of `T` based on values from the `Stats` component.
    ///
    /// # Type Parameters
    ///
    /// * `T`: The component type that implements `StatDerived`.
    fn add_stat_component<T: StatDerived + Component>(&mut self) -> &mut Self;

    /// Registers systems to manage a component `T` that implements `WriteBack`.
    ///
    /// This typically includes a system to update stat values in the `Stats` component
    /// based on changes detected in the fields of `T`.
    ///
    /// # Type Parameters
    ///
    /// * `T`: The component type that implements `WriteBack`.
    fn add_writeback_component<T: WriteBack + Component>(&mut self) -> &mut Self;

    /// A convenience method that combines `add_stat_component` and `add_writeback_component`.
    /// Use this for components that both derive from stats and write back to them.
    ///
    /// # Type Parameters
    ///
    /// * `T`: The component type that implements both `StatDerived` and `WriteBack`.
    fn add_complex_component<T: StatDerived + WriteBack + Component>(&mut self) -> &mut Self;
}

impl StatsAppExtension for App {
    fn add_stat_component<T: StatDerived + Component>(&mut self) -> &mut Self {
        self.add_systems(StatsMutation, add_stat_component_system::<T>);
        self.add_systems(StatsMutation, update_stat_component_system::<T>.after(add_stat_component_system::<T>));
        self
    }

    fn add_writeback_component<T: WriteBack + Component>(&mut self) -> &mut Self {
        self.add_systems(UpdateWriteBack, update_writeback_value_system::<T>);
        self
    }

    fn add_complex_component<T: StatDerived + WriteBack + Component>(&mut self) -> &mut Self {
        self.add_stat_component::<T>();
        self.add_writeback_component::<T>();
        self
    }
}

/// An `EntityCommand` used to mark a component `T` on an entity as "touched".
///
/// This is primarily an internal mechanism that can be used by systems to trigger
/// change detection for a component `T` even if its data hasn't strictly changed,
/// by forcing a `reborrow()` on it.
#[derive(Debug)] // Added Debug for clarity, as it's a struct
pub struct TouchCommand<T: Component>(PhantomData<T>);

impl<T: Component> EntityCommand for TouchCommand<T> {
    fn apply(self, id: Entity, world: &mut World) {
        if let Some(mut touchable) = world.entity_mut(id).get_mut::<T>() {
            touchable.reborrow();
        }
    }
}

/// An extension trait for `EntityCommands` to provide a `touch` method.
///
/// This allows easily queueing a `TouchCommand` for a specific component type on an entity.
pub trait TouchCommandExt {
    /// Queues a command to "touch" the component `T` on the current entity.
    ///
    /// This can be used to ensure change detection runs for `T` in subsequent systems,
    /// even if the component's data itself wasn't directly mutated in a way Bevy detects.
    ///
    /// # Type Parameters
    ///
    /// * `T`: The component type to touch.
    fn touch<T: Component>(&mut self);
}

impl<'w>
    TouchCommandExt
    for EntityCommands<'w>
{
    fn touch<T: Component>(&mut self) {
        self.queue(TouchCommand(PhantomData::<T>));
    }
}

use bevy::{app::MainScheduleOrder, ecs::schedule::ScheduleLabel};

/// Plugin function for the app extension module.
/// Sets up custom schedules used by the stat system for managing derived components and write-back mechanisms.
///
/// This ensures that stat calculations, updates to `StatDerived` components, and `WriteBack` operations
/// occur in a controlled order relative to Bevy's main update cycle.
/// Specifically, it inserts:
/// - `StatsMutation`: After `PreUpdate`.
/// - `UpdateStatDerived`: After `StatsMutation`.
/// - `UpdateWriteBack`: After `UpdateStatDerived`.
pub fn plugin(app: &mut App) {
    
    app.init_schedule(StatsMutation)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(PreUpdate, StatsMutation);

    app.init_schedule(UpdateStatDerived)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(StatsMutation, UpdateStatDerived);

    app.init_schedule(UpdateWriteBack)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(UpdateStatDerived, UpdateWriteBack);
}

/// Custom Bevy schedule label for systems that perform mutations on `Stats` components.
///
/// This schedule runs after `PreUpdate` and before `UpdateStatDerived`.
/// It's intended for systems that directly add/remove modifiers or change stat values.
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