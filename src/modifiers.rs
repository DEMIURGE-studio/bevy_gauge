use crate::stats::{AttributeId, StatCollection};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign, Sub, SubAssign};
use crate::prelude::{AttributeUpdatedEvent, StatValue};
use crate::tags::TagRegistry;

#[derive(Debug, Clone)]
pub enum ModifierValue {
    Flat(StatValue),
    Increased(StatValue),
    More(StatValue),
}

impl ModifierValue {
    pub fn update_value_with_ctx(&mut self, stat_collection: &StatCollection, tag_registry: &Res<TagRegistry>) {
        match self {
            ModifierValue::Flat(vt) => {vt.update_value_with_context(stat_collection, tag_registry);}
            ModifierValue::Increased(vt) => {vt.update_value_with_context(stat_collection, tag_registry);}
            ModifierValue::More(vt) => {vt.update_value_with_context(stat_collection, tag_registry);}
        }
    }
    
    pub fn get_value(&self) -> &StatValue {
        match self {
            ModifierValue::Flat(val) => {val}
            ModifierValue::Increased(value) => {value}
            ModifierValue::More(value) => {value}
        }
    }
    
}

impl Default for ModifierValue {
    fn default() -> Self {
        ModifierValue::Flat(StatValue::default())
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierValueTotal {
    flat: f32,
    increased: f32,
    more: f32,
}

impl ModifierValueTotal {
    pub fn get_total(&self) -> f32 {
        self.flat * (1.0 + self.increased) * self.more
    }
    
    pub fn get_total_with_base(&self, base: f32) -> f32 {
        (self.flat + base) * (1.0 + self.increased) * self.more
    }
}

impl Default for ModifierValueTotal {
    fn default() -> Self {
        Self {
            flat: 0.0,
            increased: 0.0,
            more: 1.0,
        }
    }
}

impl Add for ModifierValueTotal {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            flat: self.flat + other.flat,
            increased: self.increased + self.increased,
            more: (1.0 + self.more) * (1.0 + other.more),
        }
    }
}

impl Sub for ModifierValueTotal {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            flat: self.flat - other.flat,
            increased: self.increased - self.increased,
            more: (1.0 + self.more) / (1.0 + other.more),
        }
    }
}

impl AddAssign<&ModifierValue> for ModifierValueTotal {
    fn add_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => self.flat += val.get_value_f32(),
            ModifierValue::Increased(val) => self.increased += val.get_value_f32(),
            ModifierValue::More(val) => self.more *= 1.0 + val.get_value_f32(),
        }
    }
}

impl SubAssign<&ModifierValue> for ModifierValueTotal {
    fn sub_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => self.flat -= val.get_value_f32(),
            ModifierValue::Increased(val) => self.increased -= val.get_value_f32(),
            ModifierValue::More(val) => self.more /= 1.0 + val.get_value_f32(),
        }
    }
}

/// An instance that lives as a component, or rather a single component entity that exists as a child on a tree of a Stat Entity.
#[derive(Component, Debug)]
#[require(ModifierInstance)]
#[relationship(relationship_target = ModifierCollectionRefs)]
pub struct ModifierTarget {
    pub modifier_collection: Entity,
}

#[derive(Component, Debug, Default)]
pub struct ModifierInstance {
    pub target_stat: AttributeId,
    pub value: ModifierValue,
    pub dependencies: HashSet<AttributeId>,
}

impl ModifierInstance {
    pub fn update_value(&mut self, value: ModifierValue) {
        self.value = value;
    }
}


// /// A component on an entity to keep track of all Modifier Entities affecting this entity
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = ModifierTarget)]
pub struct ModifierCollectionRefs {
    #[relationship]
    modifiers: Vec<Entity>,
}

fn on_modifier_added(
    trigger: Trigger<OnAdd, ModifierInstance>,
    modifier_query: Query<(&ModifierInstance, &ModifierTarget)>,
    mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
    mut commands: Commands,
    tag_registry: Res<TagRegistry>,
) {
    
    if let Ok((modifier, stat_entity)) = modifier_query.get(trigger.target()) {
        if let Ok((entity, mut stat_collection)) =
            stat_query.get_mut(stat_entity.modifier_collection)
        {
            stat_collection.add_or_replace_modifier(modifier, trigger.target(), &tag_registry, &mut commands);
            commands.trigger_targets(
                AttributeUpdatedEvent {
                    stat_id: modifier.target_stat.clone(),
                    value: stat_collection.get_stat_value(modifier.target_stat.clone()),
                },
                entity,
            );

            println!(
                "modifier: {:?}, affects tags: {:?}, total_value: {:?}",
                &modifier, &modifier.target_stat, modifier.value
            );
        }
    }
}

/// Triggered when a modifier is removed from an entity
fn on_modifier_removed(
    trigger: Trigger<OnRemove, ModifierInstance>,
    modifier_query: Query<(&ModifierInstance, &ModifierTarget)>,
    mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
    mut commands: Commands,
    tag_registry: Res<TagRegistry>,
) {
    if let Ok((entity, mut stat_collection)) = stat_query.single_mut() {
        if let Ok((modifier, stat_entity)) = modifier_query.get(trigger.target()) {
            stat_collection.remove_modifier(trigger.target(), &tag_registry, &mut commands);
            commands.trigger_targets(
                AttributeUpdatedEvent {
                    stat_id: modifier.target_stat.clone(),
                    value: stat_collection.get_stat_value(modifier.target_stat.clone()),
                },
                entity,
            );
        }
    }
}


pub fn on_modifier_change(
    trigger: Trigger<ModifierUpdatedEvent>,
    mut modifier_query: Query<(&mut ModifierInstance, &ModifierTarget)>,
    mut stat_query: Query<&mut StatCollection, With<ModifierCollectionRefs>>,
    registry: Res<TagRegistry>,
    mut commands: Commands,
) {

    println!("on_modifier_change");
    // modifier.value.update_value_with_ctx(&stats, &tag_registry);
    // stats.update_modifier(trigger.target(), &tag_registry, &mut commands);
    if let Ok((mut modifier_instance, modifier_target)) = modifier_query.get_mut(trigger.target()) {
        if let Some(new_val) = &trigger.new_value {
            modifier_instance.value = new_val.clone();
        }
        if let Ok(mut stats) = stat_query.get_mut(modifier_target.modifier_collection) {
            modifier_instance.value.update_value_with_ctx(&stats, &registry);
            println!("modifier change: {:?}", &modifier_instance.value);
            let mut attributes_to_recalculate = Vec::new();
            if let Some(attribute_ids) = stats.attribute_modifiers.get(&trigger.target()) {
                for attribute_id in attribute_ids {
                    attributes_to_recalculate.push(attribute_id.clone());
                }
            }

            for attribute_id in attributes_to_recalculate {
                if let Some(attribute_instance) = stats.get_attribute_instance_mut(attribute_id.clone()) {
                    attribute_instance.modify_modifier(&modifier_instance, trigger.target());
                    //assert_eq!(attribute_instance.modifier_collection.get(&trigger.target()).unwrap().get_value(), trigger.new_value.get_value());
                }
            }
            stats.update_modifier(trigger.target(), &registry, &mut commands);

        }
    }
}

// commands.trigger_targets(
// ModifierUpdatedEvent {
// modifier_entity,
// 
// },
// entity,
// );


// pub fn on_modifier_changed(
//     trigger: Trigger<ModifierUpdatedEvent>,
//     modifier_query: Query<Entity, With<ModifierInstance>>,
//     mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
//     mut commands: Commands,
//     tag_registry: Res<TagRegistry>,
// ) {
//     if let Ok((entity, mut stat_collection)) = stat_query.single_mut() {
//         if let Ok(modifier_entity) = modifier_query.get(trigger.target()) {
//             if let Some(attribute_ids) = stat_collection.attribute_modifiers.get(&modifier_entity) {
//                 for attribute_id in attribute_ids {
//                     if let Some(attribute_instance) = stat_collection.get_attribute_instance(attribute_id.clone()) {
//                         attribute_instance.value.
//                     }
//                 }
//             }
//         }
//     }
// }


// pub fn on_modifier_should_update(
//     trigger: Trigger<ModifierShouldUpdate>,
//     mut modifier_query: Query<(&mut ModifierInstance, &ModifierTarget)>,
//     mut stat_query: Query<&mut StatCollection, With<ModifierCollectionRefs>>,
//     tag_registry: Res<TagRegistry>,
//     mut commands: Commands,
// ) {
//     if let Ok((mut modifier, modifier_target)) = modifier_query.get_mut(trigger.target()) {
//         if let Ok(mut stats) = stat_query.get_mut(modifier_target.modifier_collection) {
//             modifier.value.update_value_with_ctx(&stats, &tag_registry);
//             stats.update_modifier(trigger.target(), &tag_registry, &mut commands);
//         }
//     }
// }




/// Event sent when a stat is updated

#[derive(Event)]
pub struct ModifierUpdatedEvent {
    pub new_value: Option<ModifierValue>,
}

/// Register the trigger handlers
pub fn register_modifier_triggers(app: &mut App) {
    app
        .add_event::<ModifierUpdatedEvent>()
        .add_observer(on_modifier_added)
        .add_observer(on_modifier_removed)
        .add_observer(on_modifier_change);
}
