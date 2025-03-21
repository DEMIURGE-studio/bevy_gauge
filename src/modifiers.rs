use bevy::prelude::*;
use bevy_ecs::component::*;
use bevy_ecs::prelude::*;
use crate::tags::StatTag;


#[derive(Debug, Clone)]
pub enum ModifierValue {
    Flat(f32),
    Increased(f32),
    More(f32)
}


#[derive(Clone, Debug)]
pub struct Modifier {
    pub tag: StatTag,
    pub source_context: ModifierContext,
    pub target_context: Option<ModifierContext>,
    pub value: ModifierValue,
}

impl Component for Modifier {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Mutable;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
    }
}



#[derive(Debug, Clone)]
pub struct ModifierContext {
    pub entity: Entity,
}


/// A component on a stat owning entity to keep track of all Modifier Entities affecting this player
/// 
/// Attach to an entity with a stat collection
#[derive(Component, Debug)]
pub struct ModifierCollectionRefs {
    pub modifiers: Vec<Entity>
}