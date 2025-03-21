use std::collections::HashSet;
use bevy::prelude::*;
use bevy_ecs::component::*;
use bevy_ecs::prelude::*;
use bevy_ecs::relationship::RelationshipSourceCollection;
use bevy_ecs::world::DeferredWorld;
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
        hooks.on_remove(on_modifier_remove);
        hooks.on_add(on_modifier_add);
    }
}

fn on_modifier_remove(mut world: DeferredWorld, hook_context: HookContext){
    for ancestor in world.query_filtered::<&ChildOf, ()>().iter_ancestors(hook_context.entity) {
        if let Some(mut root) = world.get_mut::<ModifierCollectionRefs>(ancestor) {
            root.modifiers.remove(&hook_context.entity);
        }
    };
}

fn on_modifier_add(mut world: DeferredWorld, hook_context: HookContext){
    for ancestor in world.query_filtered::<&ChildOf, ()>().query(&world).iter_ancestors(hook_context.entity) {
        if let Some(mut root) = world.get_mut::<ModifierCollectionRefs>(ancestor) {
            root.modifiers.insert(hook_context.entity);
        }
    };
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
    pub modifiers: HashSet<Entity>
}