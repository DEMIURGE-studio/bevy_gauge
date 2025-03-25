use std::collections::{HashMap, HashSet};
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::*;
use log::warn;
use crate::tags::{TagGroup, ValueTag};
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

#[derive(Debug, Clone, Default, PartialEq)]
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


/// A definiton of a modifier for construction of the instance
#[derive(Debug, Clone, Default)]
pub struct ModifierDefinition {
    pub tag: ValueTag,
    pub value: ModifierValue
}


/// Entity only to draw relationship between modifiers nad their collection
#[derive(Component, Debug, Deref, DerefMut)]
#[relationship(relationship_target = ModifierCollectionRefs)]
#[require(ModifierInstance)]
pub struct ModifierCollectionRef {
    #[relationship] 
    pub modifier_collection: Entity,
}


/// An instance that lives as a component, or rather a single component entity that exists as a child on a tree of a Stat Entity.
#[derive(Component, Debug, Default)]
pub struct ModifierInstance {
    pub definition: ModifierDefinition,
    pub source_context: ModifierContext,
    pub target_context: Option<ModifierContext>,
}

/// Context to provide what stat entity this modifier is applied to
#[derive(Debug, Clone, Default)]
pub struct ModifierContext {
    pub entity: Option<Entity>
}


/// A component on an entity to keep track of all Modifier Entities affecting this entity
#[derive(Component, Debug, Default)]
#[require(ModifierCollectionDependencyRegistry)]
#[relationship_target(relationship = ModifierCollectionRef)]
pub struct ModifierCollectionRefs {
    #[relationship]
    modifiers: Vec<Entity>,
}

impl ModifierCollectionRefs {
    pub fn swap(&mut self, e1: Entity, e2: Entity) {
        let e1 = self.modifiers.iter().position(|e| *e == e1);
        let e2 = self.modifiers.iter().position(|e| *e == e2);
        
        if let (Some(e1), Some(e2)) = (e1, e2) {
            self.modifiers.swap(e1, e2);
        } else {
            warn!("Entity not found in swap")
        }
    }
}

#[derive(Component, Debug, Default)]
pub struct ModifierCollectionDependencyRegistry {
    dependency_mapping: EntityHashMap<HashSet<String>>,
    dependents_mapping: HashMap<String, EntityHashSet>,
    
    // TODO add separate ordering and maintain via onremove or hook
    
    // Whether resolution order needs updating
    needs_update: bool,
}


fn handle_added_modifiers(
    trigger: Trigger<OnAdd, ModifierInstance>,
    mut q_modifier_instances: Query<(&ModifierInstance, &ChildOf)>,
    mut q_modifier_collections: Query<(&mut ModifierCollectionRefs, &mut ModifierCollectionDependencyRegistry) >,
) {
    // if let Ok((modifier, modifier_entity, parent)) = q_modifier_instances.get_mut(trigger.target()) {
    //     if let Ok((mut modifier_collection, mut dependency_registry)) = q_modifier_collections.get_mut(modifier_entity.modifier_collection) {
    //         if let Some(dependencies) = modifier.definition.value.get_value_type().extract_dependencies() {
    //             dependency_registry.dependency_mapping.insert(trigger.target(), dependencies.clone());
    //             
    //             for dependency in dependencies.iter() {
    //                 dependency_registry.dependents_mapping
    //                     .entry(dependency.clone())
    //                     .or_default()
    //                     .insert(trigger.target());
    //             }
    //             
    //             // TODO reorder dependency ordering in the modifier collection
    //         }
    //     }
    // }
}

fn handle_removed_modifiers(
    trigger: Trigger<OnRemove, ModifierInstance>,
) {
    
}




pub struct ModifierRegistry {
    // Keep modifiers indexed by relevant attributes
    primary_index: HashMap<String, Vec<(ValueTag, ModifierValue)>>,
    group_index: HashMap<String, Vec<(ValueTag, ModifierValue)>>,
    value_index: HashMap<(String, String), Vec<(ValueTag, )>>,
    all_modifiers: Vec<(ValueTag, f32)>,
}

impl ModifierRegistry {
    pub fn new() -> Self {
        ModifierRegistry {
            primary_index: HashMap::new(),
            group_index: HashMap::new(),
            value_index: HashMap::new(),
            all_modifiers: Vec::new(),
        }
    }

    pub fn register(&mut self, tag: ValueTag, modifier_value: f32) {
    }

    pub fn find_matching_modifiers(&self, action: &ValueTag) -> Vec<(f32, &ValueTag)> {
        Vec::new()
    }
}




pub fn plugin(app: &mut App) {
    app.add_observer(handle_added_modifiers)
        .add_observer(handle_removed_modifiers);
}