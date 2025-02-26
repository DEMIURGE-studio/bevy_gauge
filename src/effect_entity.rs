use bevy::prelude::*;
use bevy_ecs::component::StorageType;
use bevy_utils::{HashMap, HashSet};

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

pub struct EffectEntity;

// When an effect entity is destroyed it should automatically be removed from the EffectEntities list
impl Component for EffectEntity {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut bevy_ecs::component::ComponentHooks) {
        hooks.on_add(|mut world, targeted_entity, _component_id| {
            let effect_entity_parent = world.get::<Parent>(targeted_entity).unwrap().get();
            let mut effect_entities = world.get_mut::<EffectEntities>(effect_entity_parent).unwrap();
            effect_entities.0.insert(targeted_entity);
        });
        hooks.on_remove(|mut world, targeted_entity, _component_id| {
            let effect_entity_parent = world.get::<Parent>(targeted_entity).unwrap().get();
            let mut effect_entities = world.get_mut::<EffectEntities>(effect_entity_parent).unwrap();
            effect_entities.0.remove(&targeted_entity);
        });
    }
}

#[derive(Component)]
pub struct EffectEntities(pub HashSet<Entity>);

fn test() {
    let a = [
        ("AddedLife", 10.0),
        ("Entity", (
            ("OnHit", 0.0),
            ("Explosion", 3.0),
            ("EffectEntity"),
        )),
        ("Entity", (
            ("OnBlock", 0.0),
            ("Heal", 10.0),
            ("EffectEntity"),
        )),
    ];

    /// Unpacking a
    /// Check AddedLife. Add the AddedLife (10.0) to the Stats
    /// Check EffectEntity. Create an entity with the relevant components. Added to stats under "a-EffectEntity-0"
    /// Check EffectEntity. Do the same. Added to stats under "a-EffectEntity-1"
    /// 
    /// When removing the stat stick, we remove the "a-EffectEntity-0" and "a-EffectEntity-1" from Stats and destroy the entities
    /// On being destroyed, the stat effect entity will use hooks to remove itself from the EffectEntities vec
    /// On being created the stat effect entity will use hooks to add itself to the EffectEntities vec
    /// 
    /// Scrap "EffectEntity," now we just have "Entity." When an Entity entry is passed it will be built and added
    /// with a naming convention like "a-Entity-0" or "a-Entity-1"
    /// The name is just so that when the stat is removed later, the entity can be destroyed.
    /// 
    /// Is this satifying? Does this mean that removing stats will require a "commands"?
}