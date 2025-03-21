use std::marker::PhantomData;

use bevy::prelude::*;
use bevy_ecs::component::{ComponentMutability, Mutable};
use crate::systems::update_writeback_value_system;

use super::{prelude::*, systems::{add_stat_component_system, update_stat_component_system}};

pub trait StatsAppExtension {
    fn add_stat_component<T: StatDerived + Component<Mutability = Mutable>>(&mut self) -> &mut Self;
    fn add_writeback_component<T: WriteBack + Component<Mutability = Mutable>>(&mut self) -> &mut Self;
    fn add_complex_component<T: StatDerived + WriteBack + Component<Mutability = Mutable>>(&mut self) -> &mut Self;
}

impl StatsAppExtension for App {
    fn add_stat_component<T: StatDerived + Component<Mutability = Mutable>>(&mut self) -> &mut Self {
        self.add_systems(AddStatComponent, add_stat_component_system::<T>);
        self.add_systems(StatComponentUpdate, update_stat_component_system::<T>);
        self
    }

    fn add_writeback_component<T: WriteBack + Component<Mutability = Mutable>>(&mut self) -> &mut Self {
        self.add_systems(StatsWrite, update_writeback_value_system::<T>);
        self
    }

    fn add_complex_component<T: StatDerived + WriteBack + Component<Mutability = Mutable>>(&mut self) -> &mut Self {
        self.add_stat_component::<T>();
        self.add_writeback_component::<T>();
        self
    }
}

// pub struct TouchCommand<T: Component>(PhantomData<T>);
// 
// impl<T: Component> EntityCommand for TouchCommand<T> {
//     fn apply(self, world: &mut World) {
//         if let Some(mut touchable) = world.entity_mut(id).get_mut::<T>() {
//             touchable.reborrow();
//         }
//     }
// }
// 
// pub trait TouchCommandExt {
//     fn touch<T: Component>(&mut self);
// }
// 
// impl<'w>
//     TouchCommandExt
//     for EntityCommands<'w>
// {
//     fn touch<T: Component>(&mut self) {
//         self.queue(TouchCommand(PhantomData::<T>));
//     }
// }