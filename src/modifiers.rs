use bevy::prelude::*;
use crate::tags::ValueTag;


#[derive(Debug, Clone)]
pub enum ModifierValue {
    Flat(f32),
    Increased(f32),
    More(f32)
}

impl Default for ModifierValue {
    fn default() -> Self {
        ModifierValue::Flat(0.0)
    }
}


/// A definiton of a modifier for construction of the instance
#[derive(Debug, Clone)]
pub struct ModifierDefinition {
    pub tag: ValueTag,
    pub value: ModifierValue
}


/// Entity only to draw relationship between modifiers nad their collection
#[derive(Component, Debug)]
#[relationship(relationship_target = ModifierCollectionRefs)]
#[require(ModifierInstance)]
pub struct ModifierEntity {
    #[relationship] 
    pub modier_collection_owner: Entity,
}


/// An instance that lives as a component, or rather a single component entity that exists as a child on a tree of a Stat Entity.
#[derive(Component, Debug, Default)]
pub struct ModifierInstance {
    pub tag: ValueTag,
    pub source_context: ModifierContext,
    pub target_context: Option<ModifierContext>,
    pub value: ModifierValue,
}

/// Context to provide what stat entity this modifier is applied to
#[derive(Debug, Clone, Default)]
pub struct ModifierContext {
    pub entity: Option<Entity>
}


/// A component on an entity to keep track of all Modifier Entities affecting this entity
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = ModifierEntity)]
pub struct ModifierCollectionRefs {
    modifiers: Vec<Entity>
}
