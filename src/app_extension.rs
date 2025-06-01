use bevy::prelude::*;
use super::prelude::*;
use super::systems::{add_stat_component_system, update_stat_component_system, resolve_writeback_component_system};

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
        self.add_systems(MutateStats, add_stat_component_system::<T>);
        self.add_systems(MutateStats, update_stat_component_system::<T>.after(add_stat_component_system::<T>));
        self
    }

    fn add_writeback_component<T: WriteBack + Component>(&mut self) -> &mut Self {
        self.add_systems(Resolution, resolve_writeback_component_system::<T>);
        self
    }

    fn add_complex_component<T: StatDerived + WriteBack + Component>(&mut self) -> &mut Self {
        self.add_stat_component::<T>();
        self.add_writeback_component::<T>();
        self
    }
}

use bevy::{app::MainScheduleOrder, ecs::schedule::ScheduleLabel};

/// Plugin function for the app extension module.
/// Sets up custom schedules used by the stat system for managing derived components and write-back mechanisms.
///
/// This ensures that stat calculations, updates to `StatDerived` components, and `WriteBack` operations
/// occur in a controlled order relative to Bevy's main update cycle.
/// Specifically, it inserts:
/// - `MutateStats`: After `PreUpdate` - All stat and component mutations happen here.
/// - `Resolution`: After `MutateStats` - Atomic conflict resolution between stats and components.
/// - `StatsReady`: After `Resolution` - Read-only access to resolved stat/component values.
pub fn plugin(app: &mut App) {
    app.init_schedule(MutateStats)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(PreUpdate, MutateStats);

    app.init_schedule(Resolution)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(MutateStats, Resolution);

    app.init_schedule(StatsReady)
        .world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(Resolution, StatsReady);
}

/// Custom Bevy schedule label for systems that perform mutations on `Stats` components and stat-derived components.
///
/// This schedule runs after `PreUpdate` and before `Resolution`.
/// It's intended for systems that directly add/remove modifiers, change stat values, or modify components directly.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MutateStats;

/// Custom Bevy schedule label for resolving conflicts between stat and component changes.
///
/// This schedule runs after `MutateStats` and before `StatsReady`.
/// It contains the atomic resolution system that combines stat and component changes.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Resolution;

/// Custom Bevy schedule label for systems that read resolved stat and component values.
///
/// This schedule runs after `Resolution` and before `Update`.
/// Systems here should only read from stats and components, not modify them.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsReady;