use bevy::prelude::*;
use super::{prelude::*, systems::{add_stat_component_system, update_stat_component_system}};

pub trait StatsAppExtension {
    fn add_stat_component<T: StatDerived + Component>(&mut self) -> &mut Self;
}

impl StatsAppExtension for App {
    fn add_stat_component<T: StatDerived + Component>(&mut self) -> &mut Self {
        self.add_systems(SideEffectsUpdate, add_stat_component_system::<T>);
        self.add_systems(SideEffectsUpdate, update_stat_component_system::<T>);
        self
    }
}
