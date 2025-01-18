use std::marker::PhantomData;

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

pub struct TouchCommand<T: Component>(PhantomData<T>);

impl<T: Component> EntityCommand for TouchCommand<T> {
    fn apply(self, id: Entity, world: &mut World) {
        if let Some(mut touchable) = world.entity_mut(id).get_mut::<T>() {
            touchable.reborrow();
        }
    }
}

pub trait TouchCommandExt {
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