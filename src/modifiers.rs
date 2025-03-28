use crate::stats::{AttributeId, StatCollection};
use crate::value_type::ValueType;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign, Sub, SubAssign};
use crate::tags::TagRegistry;

#[derive(Debug, Clone)]
pub enum ModifierValue {
    Flat(ValueType),
    Increased(ValueType),
    More(ValueType),
}

impl ModifierValue {
    pub fn get_value_type(&self) -> ValueType {
        match self {
            ModifierValue::Flat(vt) => vt.clone(),
            ModifierValue::Increased(vt) => vt.clone(),
            ModifierValue::More(vt) => vt.clone(),
        }
    }
}

impl Default for ModifierValue {
    fn default() -> Self {
        ModifierValue::Flat(ValueType::default())
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
            ModifierValue::Flat(val) => self.flat += val.evaluate(),
            ModifierValue::Increased(val) => self.increased += val.evaluate(),
            ModifierValue::More(val) => self.more *= 1.0 + val.evaluate(),
        }
    }
}

impl SubAssign<&ModifierValue> for ModifierValueTotal {
    fn sub_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => self.flat -= val.evaluate(),
            ModifierValue::Increased(val) => self.increased -= val.evaluate(),
            ModifierValue::More(val) => self.more /= 1.0 + val.evaluate(),
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
            stat_collection.add_or_replace_modifier(modifier, trigger.target(), &tag_registry);
            commands.trigger_targets(
                AttributeUpdatedEvent {
                    stat_id: modifier.target_stat.clone(),
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
    modifier_query: Query<Entity, With<ModifierInstance>>,
    mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
    mut commands: Commands,
    tag_registry: Res<TagRegistry>,
) {
    if let Ok((entity, mut stat_collection)) = stat_query.single_mut() {
        if let Ok(modifier_entity) = modifier_query.get(trigger.target()) {
            stat_collection.remove_modifier(trigger.target(), &tag_registry);
            commands.trigger_targets(
                ModifierUpdatedEvent {
                    modifier_entity,
                },
                entity,
            );
        }
    }
}


/// Event sent when a stat is updated
#[derive(Event)]
pub struct AttributeUpdatedEvent {
    pub stat_id: AttributeId,
}

#[derive(Event)]
pub struct ModifierUpdatedEvent {
    pub modifier_entity: Entity,
}

/// Register the trigger handlers
pub fn register_modifier_triggers(app: &mut App) {
    app.add_event::<AttributeUpdatedEvent>()
        .add_observer(on_modifier_added)
        .add_observer(on_modifier_removed);
}
