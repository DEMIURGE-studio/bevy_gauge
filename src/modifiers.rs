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
        for target_tags in self.modifiers.get_mut(&modifier_entity) { // set of target tags that this modifier effects
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


// /// Entity only to draw relationship between modifiers nad their collection
// #[derive(Component, Debug, Deref, DerefMut)]
// #[relationship(relationship_target = ModifierCollectionRefs)]
// #[require(ModifierInstance)]
// pub struct ModifierCollectionRef {
//     #[relationship] 
//     pub modifier_collection: Entity,
// }
// 
// /// Context to provide what stat entity this modifier is applied to
// #[derive(Debug, Clone, Default)]
// pub struct ModifierContext {
//     pub entity: Option<Entity>
// }
// 
// 
// /// A component on an entity to keep track of all Modifier Entities affecting this entity
// #[derive(Component, Debug, Default)]
// #[require(ModifierCollectionDependencyRegistry)]
// #[relationship_target(relationship = ModifierCollectionRef)]
// pub struct ModifierCollectionRefs {
//     #[relationship]
//     modifiers: Vec<Entity>,
// }
// 
// impl ModifierCollectionRefs {
//     pub fn swap(&mut self, e1: Entity, e2: Entity) {
//         let e1 = self.modifiers.iter().position(|e| *e == e1);
//         let e2 = self.modifiers.iter().position(|e| *e == e2);
//         
//         if let (Some(e1), Some(e2)) = (e1, e2) {
//             self.modifiers.swap(e1, e2);
//         } else {
//             warn!("Entity not found in swap")
//         }
//     }
// }
// 
// #[derive(Component, Debug, Default)]
// pub struct ModifierCollectionDependencyRegistry {
//     dependency_mapping: EntityHashMap<HashSet<String>>,
//     dependents_mapping: HashMap<String, EntityHashSet>,
//     
//     // TODO add separate ordering and maintain via onremove or hook
//     
//     // Whether resolution order needs updating
//     needs_update: bool,
// }


// fn handle_added_modifiers(
//     _trigger: Trigger<OnAdd, ModifierInstance>,
//     mut _q_modifier_instances: Query<(&ModifierInstance, &ChildOf)>,
//     mut _q_modifier_collections: Query<(&mut ModifierCollectionRefs, &mut ModifierCollectionDependencyRegistry) >,
// ) {
//     // if let Ok((modifier, modifier_entity, parent)) = q_modifier_instances.get_mut(trigger.target()) {
//     //     if let Ok((mut modifier_collection, mut dependency_registry)) = q_modifier_collections.get_mut(modifier_entity.modifier_collection) {
//     //         if let Some(dependencies) = modifier.definition.value.get_value_type().extract_dependencies() {
//     //             dependency_registry.dependency_mapping.insert(trigger.target(), dependencies.clone());
//     //             
//     //             for dependency in dependencies.iter() {
//     //                 dependency_registry.dependents_mapping
//     //                     .entry(dependency.clone())
//     //                     .or_default()
//     //                     .insert(trigger.target());
//     //             }
//     //             
//     //             // TODO reorder dependency ordering in the modifier collection
//     //         }
//     //     }
//     // }
// }
// 
// fn handle_removed_modifiers(
//     _trigger: Trigger<OnRemove, ModifierInstance>,
// ) {
//     
// }




// pub struct ModifierRegistry {
//     // Keep modifiers indexed by relevant attributes
//     primary_index: HashMap<String, Vec<(ValueTag, ModifierValue)>>,
//     group_index: HashMap<String, Vec<(ValueTag, ModifierValue)>>,
//     value_index: HashMap<(String, String), Vec<(ValueTag, )>>,
//     all_modifiers: Vec<(ValueTag, f32)>,
// }
// 
// impl ModifierRegistry {
//     pub fn new() -> Self {
//         ModifierRegistry {
//             primary_index: HashMap::new(),
//             group_index: HashMap::new(),
//             value_index: HashMap::new(),
//             all_modifiers: Vec::new(),
//         }
//     }
// 
//     pub fn register(&mut self, _tag: ValueTag, _modifier_value: f32) {
//     }
// 
//     pub fn find_matching_modifiers(&self, _action: &ValueTag) -> Vec<(f32, &ValueTag)> {
//         Vec::new()
//     }
// }




// pub fn plugin(app: &mut App) {
//     app.add_observer(handle_added_modifiers)
//         .add_observer(handle_removed_modifiers);
// }