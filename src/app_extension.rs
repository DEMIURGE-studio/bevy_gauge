use std::marker::PhantomData;
use bevy::prelude::*;
use super::{prelude::*, systems::{add_stat_component_system, update_stat_component_system}};

pub trait StatsAppExtension {
    fn add_stat_component<T: StatDerived + Component>(&mut self) -> &mut Self;
    fn add_writeback_component<T: WriteBack + Component>(&mut self) -> &mut Self;
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

use bevy::{app::MainScheduleOrder, ecs::schedule::ScheduleLabel};

// order: end of pre-update -> safe Stats changes -> update StatDerived -> Update -> writeback
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


#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatsMutation;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UpdateStatDerived;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UpdateWriteBack;