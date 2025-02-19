use bevy::prelude::*;
use bevy_ecs::component::StorageType;
use bevy_utils::HashMap;

/// I want to support doing stuff like 
/// "EffectEntity": {
///     "ExplosionEffect": { ... }
///     "OnMeditate": { ... }
/// }
/// 
/// Basically, some types of stats should be able to add child entities
/// instead of just adding components to the effected entity.
/// 
/// Adding effect entities from a stat should be fairly trivial. 
/// However, removing effect entities when a stat is removed may not be
/// so simple.
/// 
/// Maybe when an effect entity is added, it gets an entry in the Stats
/// hashmap. Maybe the key is {Affix}-EffectEntity-{num}, so Test-EffectEntity-1
/// This could map directly to the entity. When the stat is removed, the
/// entity is destroyed.
/// 
/// So we get the ron, which has an "EffectEntity" in it. We generate a 
/// key string for the effect. We generate the entity with appropriate components
/// and add it to the "EffectEntities" vec. We also add it to the Stats hashmap.
/// 
/// We want to remove an EffectEntity stat. We get the unique id for the 
/// effect entity, delete the effect entity, remove it from EffectEntities,
/// and remove its Stats entry.
/// 
/// An EffectEntityId component that contains the generated key string?
/// 
/// OR OR OR What if all EffectEntities were stored in a collection behind the
/// "EffectEntity" key on the hashmap?

pub struct EffectEntityId(pub String);

// When an effect entity is destroyed it should automatically be removed from the EffectEntities list
impl Component for EffectEntityId {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut bevy_ecs::component::ComponentHooks) {
        hooks.on_remove(|mut world, targeted_entity, _component_id| {
            let effect_entity_id = world.get::<EffectEntityId>(targeted_entity).unwrap().0.clone();
            let effect_entity_parent = world.get::<Parent>(targeted_entity).unwrap().get();
            let mut effect_entities = world.get_mut::<EffectEntities>(effect_entity_parent).unwrap();
            effect_entities.0.remove(&effect_entity_id);
        });
    }
}

#[derive(Component)]
pub struct EffectEntities(pub HashMap<String, Entity>);