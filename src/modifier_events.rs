use crate::modifiers::{ModifierCollectionRefs, ModifierInstance, ModifierTarget, ModifierValue};
use crate::prelude::{StatCollection, TagRegistry};
use crate::stat_events::{AttributeShouldRecalculate};
use bevy::app::App;
use bevy::prelude::{Commands, Entity, Event, OnAdd, OnRemove, Query, Res, Trigger, With};

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
            stat_collection.add_or_replace_modifier(
                modifier,
                trigger.target(),
                &tag_registry,
                &mut commands,
                entity
            );
            commands.trigger_targets(
                AttributeShouldRecalculate {
                    attribute_id: modifier.target_stat.clone(),
                },
                entity
            );
            
        }
    }
}

/// Triggered when a modifier is removed from an entity
fn on_modifier_removed(
    trigger: Trigger<OnRemove, ModifierInstance>,
    modifier_query: Query<(&ModifierInstance, &ModifierTarget)>,
    mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
    mut commands: Commands,
    tag_registry: Res<TagRegistry>,
) {
    if let Ok((entity, mut stat_collection)) = stat_query.single_mut() {
        if let Ok((modifier, stat_entity)) = modifier_query.get(trigger.target()) {
            stat_collection.remove_modifier(trigger.target(), &tag_registry, &mut commands, entity);

            let mut modifier_deps = Vec::new();

            if let Some(target_group) = stat_collection
                .attributes
                .get_mut(&modifier.target_stat.group)
            {
                for (key, value) in target_group {
                    if key & modifier.target_stat.tag > 0 {
                        let mut attribute_write = value.write().unwrap();
                        modifier_deps = modifier.dependencies.clone().iter().collect();
                        attribute_write.remove_modifier(trigger.target());
                        // NEED TO REMOVE FROM COLLECTION TODO

                        //value.add_or_replace_modifier(modifier, modifier_entity);
                        //self.attribute_modifiers.entry(modifier_entity).or_insert_with(HashSet::new).insert(modifier.target_stat.clone());
                    }
                }
                // self.recalculate_attribute_and_dependents(modifier.target_stat.clone(), tag_registry, commands)
            }

            commands.trigger_targets(
                AttributeShouldRecalculate {
                    attribute_id: modifier.target_stat.clone(),
                },
                entity
            );

            // stat_collection.recalculate_attribute_and_dependents(
            //     modifier.target_stat.clone(),
            //     &tag_registry,
            //     &mut commands,
            // );
            // commands.trigger_targets(
            //     AttributeUpdatedEvent {
            //         stat_id: modifier.target_stat.clone(),
            //         value: stat_collection.get_stat_value(modifier.target_stat.clone()),
            //     },
            //     entity,
            // );
        }
    }
}

pub fn on_modifier_change(
    trigger: Trigger<ModifierUpdatedEvent>,
    mut modifier_query: Query<(&mut ModifierInstance, &ModifierTarget)>,
    mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
    registry: Res<TagRegistry>,
    mut commands: Commands,
) {
    println!("on_modifier_change");
    // modifier.value.update_value_with_ctx(&stats, &tag_registry);
    // stats.update_modifier(trigger.target(), &tag_registry, &mut commands);
    if let Ok((mut modifier_instance, modifier_target)) = modifier_query.get_mut(trigger.target()) {
        if let Some(new_val) = &trigger.new_value {
            modifier_instance.value = new_val.clone();
        }
        if let Ok((entity, mut stats)) = stat_query.get_mut(modifier_target.modifier_collection) {
            if let Some(dependencies) = modifier_instance.value.get_value().extract_dependencies() {
                let context = stats.get_stat_relevant_context(&dependencies, &registry);
                modifier_instance
                    .value
                    .update_value_with_ctx(context, &registry);
                println!("modifier change: {:?}", &modifier_instance.value);
            }
            let mut attributes_to_recalculate = Vec::new();
            if let Some(attribute_ids) = stats.attribute_modifiers.get(&trigger.target()) {
                for attribute_id in attribute_ids {
                    attributes_to_recalculate.push(attribute_id.clone());
                }
            }

            for attribute_id in attributes_to_recalculate {
                if let Some(attribute_instance) =
                    stats.get_attribute_instance_mut(attribute_id.clone())
                {
                    let mut attribute_write = attribute_instance.write().unwrap();
                    attribute_write.modify_modifier(&modifier_instance, trigger.target());
                    //assert_eq!(attribute_instance.modifier_collection.get(&trigger.target()).unwrap().get_value(), trigger.new_value.get_value());
                }
            }
            stats.update_modifier(trigger.target(), &registry, &mut commands, entity);
        }
    }
}

#[derive(Event)]
pub struct ModifierUpdatedEvent {
    pub new_value: Option<ModifierValue>,
}

/// Register the trigger handlers
pub fn register_modifier_triggers(app: &mut App) {
    app.add_event::<ModifierUpdatedEvent>()
        .add_observer(on_modifier_added)
        .add_observer(on_modifier_removed)
        .add_observer(on_modifier_change);
}
