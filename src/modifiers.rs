use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub, SubAssign};
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::*;
use crate::stats::{StatCollection, StatType};
use crate::value_type::{Expression, StatValue, ValueType};

#[derive(Debug, Clone)]
pub enum ModifierStorage {
    Single(SimpleStatModifierStorage),
    BitMasked(BitMaskedStatModifierStorage),
}

#[derive(Debug, Clone, Default)]
pub struct SimpleStatModifierStorage {
    pub modifiers: EntityHashMap<ModifierValue>,
    pub modifier_total: ModifierValueTotal,
}

#[derive(Debug, Clone, Default)]
pub struct BitMaskedStatModifierStorage {
    pub tags: HashMap<u32, IntermediateModifierValue>, // the cached modifier value for a target tag
    pub modifiers: EntityHashMap<HashSet<u32>>,
}

impl Default for ModifierStorage {
    fn default() -> Self {
        Self::Single(SimpleStatModifierStorage::default())
    }
}

impl ModifierStorage {
    pub fn add_or_replace_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        match self {
            ModifierStorage::Single(storage) => {
                let ModifierStorageType::Single = &modifier.modifier_stat_target else { return };
                if let Some(modifier_value) = storage.modifiers.get_mut(&modifier_entity) {
                    storage.modifier_total -= modifier_value;
                    *modifier_value = modifier.value.clone();
                } else {
                    storage.modifiers.insert(modifier_entity, modifier.value.clone());
                }

                storage.modifier_total += &modifier.value;
            }
            ModifierStorage::BitMasked(storage) => {
                let ModifierStorageType::BitMasked(source_tag) = &modifier.modifier_stat_target else { return };
                if !storage.tags.contains_key(&source_tag){
                    storage.tags.insert(*source_tag, IntermediateModifierValue::default());
                    println!("{:?}", storage.tags);
                }

                // need to store the cache differently. they are combining with one another?
                
                for (tag, intermediate_modifier_value) in storage.tags.iter_mut() {
                    if tag & source_tag > 0 {
                        intermediate_modifier_value.entities.insert(modifier_entity);
                        intermediate_modifier_value.modifier_value += &modifier.value;
                        if let Some(tag_set) = storage.modifiers.get_mut(&modifier_entity){
                            tag_set.insert(*tag);
                        } else {
                            storage.modifiers.insert(modifier_entity, HashSet::from([*tag]));
                        }
                        //println!("{:?}", intermediate_modifier_value);
                    }
                }

                
            }
        }
    }

    pub fn remove_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        match self {
            ModifierStorage::Single(storage) => {
                storage.modifier_total -= &modifier.value;
                storage.modifiers.remove(&modifier_entity);
            }
            ModifierStorage::BitMasked(storage) => {
                if let Some(target_tags) = storage.modifiers.get_mut(&modifier_entity) { // set of target tags that this modifier effects
                    for target_tag in target_tags.drain() { // as we clear this take each target tag
                        if let Some(intermediate_modifier_value) = storage.tags.get_mut(&target_tag) { // find that target tag in the tags map
                            intermediate_modifier_value.entities.remove(&modifier_entity); // remove the modifier entity from the tag maps hashset of entities
                            intermediate_modifier_value.modifier_value -= &modifier.value; // decrement the modifier total
                            if intermediate_modifier_value.entities.is_empty() {
                                storage.tags.remove(&target_tag);
                            }
                        };
                    }
                }
                storage.modifiers.remove(&modifier_entity);
            }
        }
    }
}


#[derive(Debug, Clone)]
pub enum ModifierValue {
    Flat(ValueType),
    Increased(ValueType),
    More(ValueType)
}

impl ModifierValue {
    pub fn get_value_type(&self) -> ValueType {
        match self {
            ModifierValue::Flat(vt) => {vt.clone()}
            ModifierValue::Increased(vt) => {vt.clone()}
            ModifierValue::More(vt) => {vt.clone()}
        }
    }
}

impl Default for ModifierValue {
    fn default() -> Self {
        ModifierValue::Flat(ValueType::default())
    }
}


// #[derive(Debug, Clone, PartialEq, Default)]
// pub struct Intermediate/*<T: BitAnd + BitOr + BitAndAssign + BitOrAssign + Clone>*/ {
//     pub tags: HashMap<u32, IntermediateModifierValue>, // the cached modifier value for a target tag
//     pub modifiers: EntityHashMap<HashSet<u32>>,
//     // DAMAGE.FIRE.SWORD.ONE_HANDED - DAMAGE, FIRE, SWORD, 1H
//     // DAMAGE.ICE.SWORD.ONE_HANDED - DAMAGE, SWORD, 1H
// }


#[derive(Debug, Clone, PartialEq, Default)]
pub struct IntermediateModifierValue {
    pub entities: EntityHashSet,
    pub modifier_value: ModifierValueTotal
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
            more: (1.0 + self.more) * (1.0 + other.more)
        }
        
    }
}

impl Sub for ModifierValueTotal {
    type Output = Self;
    
    fn sub(self, other: Self) -> Self {
        Self {
            flat: self.flat - other.flat,
            increased: self.increased - self.increased,
            more: (1.0 + self.more) / (1.0 + other.more)
        }
    }
}


impl AddAssign<&ModifierValue> for ModifierValueTotal {
    fn add_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => { self.flat += val.evaluate()}
            ModifierValue::Increased(val) => { self.increased += val.evaluate() }
            ModifierValue::More(val) => { self.more *= 1.0 + val.evaluate() }
        }
    }
}

impl SubAssign<&ModifierValue> for ModifierValueTotal {
    fn sub_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => { self.flat -= val.evaluate()}
            ModifierValue::Increased(val) => { self.increased -= val.evaluate() }
            ModifierValue::More(val) => { self.more /= 1.0 + val.evaluate() }
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
    pub target_stat: String,
    pub modifier_stat_target: ModifierStorageType,
    pub value: ModifierValue,
    pub dependencies: HashSet<String>
}

#[derive(Debug, Default)]
pub enum ModifierStorageType {
    #[default]
    Single,
    BitMasked(u32),
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
    mut commands: Commands
) {
    if let Ok((modifier, stat_entity)) = modifier_query.get(trigger.target()) {
        if let Ok((entity, mut stat_collection)) = stat_query.get_mut(stat_entity.modifier_collection) {
            stat_collection.add_replace_modifier(&modifier.target_stat, modifier, trigger.target());
            commands.trigger_targets(StatUpdatedEvent {stat_name: modifier.target_stat.clone()}, entity);

            println!("modifier: {:?}, affects tags: {:?}, total_value: {:?}", &modifier, &modifier.modifier_stat_target, modifier.value);
        }
    }
}

/// Triggered when a modifier is removed from an entity
fn on_modifier_removed(
    trigger: Trigger<OnRemove, ModifierInstance>,
    modifier_query: Query<&ModifierInstance>,
    mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
    mut commands: Commands
) {
    if let Ok((entity, mut stat_collection)) = stat_query.single_mut() {
        if let Ok(modifier) = modifier_query.get(trigger.target()) {
            stat_collection.remove_modifier(&modifier.target_stat, modifier, trigger.target());
            commands.trigger_targets(StatUpdatedEvent {stat_name: modifier.target_stat.clone()}, entity);
        }
    }
}


/// Event sent when a stat is updated
#[derive(Event)]
pub struct StatUpdatedEvent {
    pub stat_name: String,
}

/// Register the trigger handlers
pub fn register_modifier_triggers(app: &mut App) {
    app
        .add_event::<StatUpdatedEvent>()
        .add_observer(on_modifier_added)
        .add_observer(on_modifier_removed);
}
