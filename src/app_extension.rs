use bevy::{ecs::component::Mutable, prelude::*};
use super::prelude::*;
use crate::schedule::{StatsMutation, UpdateWriteBack};

/// An extension trait for `bevy::prelude::App` to simplify the setup of components
/// that derive their values from the stat system or write their values back to it.
///
/// This provides convenient methods to register the necessary systems for components
/// that implement `StatDerived` and/or `WriteBack`.
pub trait StatsAppExtension {
    /// Registers systems to manage a component `T` that implements `StatDerived`.
    ///
    /// This includes a system to update the fields of `T` based on values from the `Stats` component
    /// when the `StatsProxy` changes. The component must already exist on the entity.
    ///
    /// # Type Parameters
    ///
    /// * `T`: The component type that implements `StatDerived`.
    fn add_stat_component<T: StatDerived + Component<Mutability = Mutable>>(&mut self) -> &mut Self;

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
    fn add_complex_component<T: StatDerived + WriteBack + Component<Mutability = Mutable>>(&mut self) -> &mut Self;

}

impl StatsAppExtension for App {
    fn add_stat_component<T: StatDerived + Component<Mutability = Mutable>>(&mut self) -> &mut Self {
        self.add_systems(StatsMutation, update_stat_component_system::<T>
            .before(update_stats_proxy_system)
        );
        self
    }

    fn add_writeback_component<T: WriteBack + Component>(&mut self) -> &mut Self {
        self.add_systems(UpdateWriteBack, update_writeback_value_system::<T>);
        self
    }

    fn add_complex_component<T: StatDerived + WriteBack + Component<Mutability = Mutable>>(&mut self) -> &mut Self {
        self.add_stat_component::<T>();
        self.add_writeback_component::<T>();
        self
    }

}

// schedules moved to crate::schedule

// relationship systems moved to crate::sources