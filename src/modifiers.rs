use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub, SubAssign};
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::*;
use crate::value_type::ValueType;

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


#[derive(Debug, Clone, PartialEq, Default)]
pub struct Intermediate/*<T: BitAnd + BitOr + BitAndAssign + BitOrAssign + Clone>*/ {
    pub tags: HashMap<u32, (EntityHashSet, ModifierValueTotal)>, // the cached modifier value for a target tag
    // DAMAGE.FIRE.SWORD.ONE_HANDED - DAMAGE, FIRE, SWORD, 1H
    // DAMAGE.ICE.SWORD.ONE_HANDED - DAMAGE, SWORD, 1H
    pub modifiers: EntityHashMap<HashSet<u32>>,
}


impl Intermediate {
    pub fn add_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        for (tag, &mut (ref mut vset, mut modifier_value)) in self.tags.iter_mut() {
            if tag & modifier.source_tag > 1 {
                vset.insert(modifier_entity);
                modifier_value += &modifier.value;
                self.modifiers.entry(modifier_entity).or_insert(HashSet::new()).insert(*tag);
            }
        }
    }
    
    pub fn remove_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        if let Some(target_tags) = self.modifiers.get_mut(&modifier_entity) { // set of target tags that this modifier effects
            for target_tag in target_tags.drain() { // as we clear this take each target tag
                if let Some(&mut (ref mut map, mut modifier_total)) = self.tags.get_mut(&target_tag) { // find that target tag in the tags map
                    map.remove(&modifier_entity); // remove the modifier entity from the tag maps hashset of entities
                    modifier_total -= &modifier.value; // decrement the modifier total
                    if map.is_empty() {
                        self.tags.remove(&target_tag);
                    }
                };
            }
        }
        self.modifiers.remove(&modifier_entity);
    }
}


#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ModifierValueTotal {
    flat: f32,
    increased: f32,
    more: f32,
}

impl ModifierValueTotal {
    pub fn get_total(&self) -> f32 {
        self.flat * self.increased * self.more
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
#[derive(Component, Debug, Default)]
pub struct ModifierInstance {
    pub source_tag: u32,
    pub value: ModifierValue,
    pub dependencies: HashSet<String>
    // pub source_context: ModifierContext,
    // pub target_context: Option<ModifierContext>,
}


/// Entity only to draw relationship between modifiers nad their collection
#[derive(Component, Debug, Deref, DerefMut)]
#[relationship(relationship_target = ModifierCollectionRefs)]
#[require(ModifierInstance)]
pub struct ModifierCollectionRef {
    #[relationship] 
    pub modifier_collection: Entity,
}

// /// Context to provide what stat entity this modifier is applied to
// #[derive(Debug, Clone, Default)]
// pub struct ModifierContext {
//     pub entity: Option<Entity>
// }
// 
// 

// /// A component on an entity to keep track of all Modifier Entities affecting this entity
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = ModifierCollectionRef)]
pub struct ModifierCollectionRefs {
    #[relationship]
    modifiers: Vec<Entity>,
}
