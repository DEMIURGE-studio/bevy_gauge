use crate::prelude::{AttributeId, StatCollection, TagRegistry};
use crate::stat_value::StatValue;
use bevy::app::App;
use bevy::prelude::{Commands, Event, Query, Res, Trigger};

pub fn on_stat_added(
    trigger: Trigger<AttributeAddedEvent>,
    mut stat_query: Query<&mut StatCollection>,
    registry: Res<TagRegistry>,
    mut commands: Commands,
) {
    let mut stat_collection = stat_query.get_mut(trigger.target()).unwrap();
    println!("on stat added");
    stat_collection.add_attribute(
        &trigger.attribute_group,
        &trigger.attribute_name,
        trigger.value.clone(),
        &registry,
        &mut commands,
        trigger.target(),
    );
}

pub fn on_stat_updated(
    trigger: Trigger<AttributeUpdatedEvent>,
    mut stat_query: Query<&mut StatCollection>,
    registry: Res<TagRegistry>,
    mut commands: Commands,
) {
    println!("trigger received");
    let mut stat_collection = stat_query.get_mut(trigger.target()).unwrap();
    if let Some(value) = stat_collection.get_attribute_instance_mut(trigger.stat_id.clone()) {
        let mut attribute_write = value.write().unwrap();
        attribute_write.value = trigger.value.clone().unwrap();
        stat_collection.recalculate_attribute_and_dependents(
            trigger.stat_id.clone(),
            &registry,
            &mut commands,
            trigger.target()
        );
    }
    // add_stat_to_cVollection(&mut stat_collection, &trigger.attribute_group, &trigger.attribute_name, trigger.value.clone(), &registry, commands);
}


pub fn on_attribute_should_recalculate(
    trigger: Trigger<AttributeShouldRecalculate>,
    mut stat_query: Query<&mut StatCollection>,
    registry: Res<TagRegistry>,
    mut commands: Commands,
) {
    let mut stat_collection = stat_query.get_mut(trigger.target()).unwrap();
    stat_collection.recalculate_attributes(&trigger.attribute_id, &registry, &mut commands, trigger.target());

    //     self.update_dependent_attribute(attr_id.clone(), tag_registry, commands);
    // let stats = stat_collection.get_attribute_instances_mut(trigger.attribute_id.clone());
    // for attribute in stats {
    //     attribute.value.update_value_with_context()
    // }
    
    
}

#[derive(Event, Debug)]
pub struct AttributeAddedEvent {
    pub attribute_name: String,
    pub attribute_group: String,
    pub value: StatValue,
}

#[derive(Event)]
pub struct AttributeUpdatedEvent {
    pub stat_id: AttributeId,
    pub value: Option<StatValue>,
}

#[derive(Event)]
pub struct AttributeShouldRecalculate {
    pub attribute_id: AttributeId,
}




pub fn register_stat_triggers(app: &mut App) {
    app.add_event::<AttributeAddedEvent>()
        .add_event::<AttributeUpdatedEvent>()
        .add_event::<AttributeShouldRecalculate>()
        .add_observer(on_stat_updated)
        .add_observer(on_stat_added)
        .add_observer(on_attribute_should_recalculate);
}
