
#[cfg(test)]
mod modifier_integration_tests {
    use super::*;
    use bevy::prelude::*;
    use crate::modifiers::{ModifierInstance, ModifierValue, ModifierStorageType, ModifierTarget, ModifierCollectionRefs, StatUpdatedEvent, register_modifier_triggers, ModifierStorage, BitMaskedStatModifierStorage};
    use crate::stats::{StatCollection, StatType, on_modifier_change, StatInstance};
    use crate::value_type::{StatValue, ValueType, Expression};
    use crate::prelude::AttributeInstance;
    use crate::resource::ResourceInstance;
    use std::collections::HashSet;
    use evalexpr::build_operator_tree;
    use crate::tags::TagRegistry;

    #[test]
    fn test_modifier_application() {
        // Setup app
        let mut app = App::new();

        // Add necessary resources
        app.init_resource::<TagRegistry>();
        app.add_event::<StatUpdatedEvent>();

        // Add systems
        register_modifier_triggers(&mut app);

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Add a base stat
        {
            let mut stat_collection = app.world_mut().get_mut::<StatCollection>(character_id).unwrap();
            let strength = AttributeInstance::default();
            stat_collection.insert("strength", StatInstance::new(StatType::Attribute(strength), ModifierStorage::default()));
        }

        // Create a flat modifier
        let modifier_id = app
            .world_mut()
            .spawn((ModifierInstance {
                target_stat: "strength".to_string(),
                modifier_stat_target: ModifierStorageType::Single,
                value: ModifierValue::Flat(ValueType::from_float(5.0)),
                dependencies: HashSet::new(),
            }, ModifierTarget {
                modifier_collection: character_id,
            }))
            .id();

        // Run systems
        app.update();

        // Check that the modifier was applied
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
        let strength_stat = stat_collection.stats.get("strength").unwrap();

        match &strength_stat.modifier_collection {
            ModifierStorage::Single(storage) => {
                // Check that the modifier is stored
                assert_eq!(storage.modifiers.len(), 1);
                assert!(storage.modifiers.contains_key(&modifier_id));

                // Check that the value is correct (base 10 + modifier 5 = 15)
                assert!((storage.modifier_total.get_total() - 5.0).abs() < 0.001);
            },
            _ => panic!("Expected Single modifier storage"),
        }

        // Now remove the modifier
        app.world_mut().despawn(modifier_id);
        app.update();

        // Check that the modifier was removed
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
        let strength_stat = stat_collection.stats.get("strength").unwrap();

        match &strength_stat.modifier_collection {
            ModifierStorage::Single(storage) => {
                assert_eq!(storage.modifiers.len(), 0);
                assert!((storage.modifier_total.get_total()).abs() < 0.001);
            },
            _ => panic!("Expected Single modifier storage"),
        }
    }

    #[test]
    fn test_stat_creation_with_dependencies() {
        // Setup app
        let mut app = App::new();

        // Add necessary resources
        app.init_resource::<TagRegistry>();
        app.add_event::<StatUpdatedEvent>();

        // Add systems

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Add base stats
        {
            let mut stat_collection = app.world_mut().get_mut::<StatCollection>(character_id).unwrap();

            // Create strength attribute
            let strength = AttributeInstance::new(StatValue::from_f32(10.0));
            stat_collection.insert("strength", StatInstance::new(StatType::Attribute(strength), ModifierStorage::default()));

            // Create constitution attribute
            let constitution = AttributeInstance::new(StatValue::from_f32(12.0));
            stat_collection.insert("constitution", StatInstance::new(StatType::Attribute(constitution), ModifierStorage::default()));

            // Create a derived health stat based on constitution
            let node = build_operator_tree("constitution * 5").unwrap();
            let health_expr = Expression::new(node);
            let health_stat_value = StatValue::new(ValueType::Expression(health_expr), None);
            let health = AttributeInstance::new(health_stat_value);
            stat_collection.insert("health", StatInstance::new(StatType::Attribute(health), ModifierStorage::default()));
        }

        // Run systems to calculate derived stats
        app.update();

        // Check that health was calculated correctly (constitution * 5 = 12 * 5 = 60)
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            if let StatType::Attribute(attr) = &stat_collection.stats.get("health").unwrap().stat {
                let health_value = attr.get_value_f32();
                assert!((health_value - 60.0).abs() < 0.001,
                        "Expected health to be 60.0, got {}", health_value);
            }
        }

        // Update constitution
        {
            let mut stat_collection = app.world_mut().get_mut::<StatCollection>(character_id).unwrap();
            let new_constitution = AttributeInstance::new(StatValue::from_f32(15.0));
            stat_collection.insert("constitution", StatInstance::new(StatType::Attribute(new_constitution), ModifierStorage::default()));
        }

        // Run systems again to update health
        app.update();

        // Check that health was updated (constitution * 5 = 15 * 5 = 75)
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            if let StatType::Attribute(attr) = &stat_collection.stats.get("health").unwrap().stat {
                let health_value = attr.get_value_f32();
                assert!((health_value - 75.0).abs() < 0.001,
                        "Expected health to be 75.0, got {}", health_value);
            }
        }
    }

    #[test]
    fn test_bitmask_modifiers() {
        // Setup app
        let mut app = App::new();

        // Add necessary resources
        app.init_resource::<TagRegistry>();
        app.add_event::<StatUpdatedEvent>();

        // Add systems
        register_modifier_triggers(&mut app);

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();

            // Register primary tag type
            tag_registry.register_primary_type("ATTRIBUTE");

            // Register individual attribute tags
            tag_registry.register_subtype("ATTRIBUTE", "STRENGTH");
            tag_registry.register_subtype("ATTRIBUTE", "DEXTERITY");
            tag_registry.register_subtype("ATTRIBUTE", "CONSTITUTION");
        }

        // Get tag IDs
        let str_tag = app.world().resource::<TagRegistry>().get_id("ATTRIBUTE", "STRENGTH").unwrap();
        let dex_tag = app.world().resource::<TagRegistry>().get_id("ATTRIBUTE", "DEXTERITY").unwrap();
        let con_tag = app.world().resource::<TagRegistry>().get_id("ATTRIBUTE", "CONSTITUTION").unwrap();

        // Create compound tags
        let physical_attr_tag = str_tag | dex_tag;          // Physical attributes (STR + DEX)
        let all_attr_tag = str_tag | dex_tag | con_tag;     // All attributes

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Add base stats
        {
            let mut stat_collection = app.world_mut().get_mut::<StatCollection>(character_id).unwrap();

            let strength = AttributeInstance::new(StatValue::from_f32(10.0));
            stat_collection.insert("ATTRIBUTE", StatInstance::new(StatType::Attribute(strength), ModifierStorage::BitMasked(BitMaskedStatModifierStorage::default())));

            let dexterity = AttributeInstance::new(StatValue::from_f32(8.0));
            stat_collection.insert("ATTRIBUTE", StatInstance::new(StatType::Attribute(dexterity), ModifierStorage::BitMasked(BitMaskedStatModifierStorage::default())));

            let constitution = AttributeInstance::new(StatValue::from_f32(12.0));
            stat_collection.insert("ATTRIBUTE", StatInstance::new(StatType::Attribute(constitution), ModifierStorage::BitMasked(BitMaskedStatModifierStorage::default())));
        }

        // Create BitMasked modifiers

        // 1. Modifier that affects only strength
        let str_modifier_id = app
            .world_mut()
            .spawn((ModifierInstance {
                target_stat: "ATTRIBUTE".to_string(),
                modifier_stat_target: ModifierStorageType::BitMasked(str_tag),
                value: ModifierValue::Flat(ValueType::from_float(2.0)),
                dependencies: HashSet::new(),
            }, ModifierTarget {
                modifier_collection: character_id,
            }))
            .id();

        // 2. Modifier that affects physical attributes (STR + DEX)
        let physical_modifier_id = app
            .world_mut()
            .spawn((ModifierInstance {
                target_stat: "ATTRIBUTE".to_string(),
                modifier_stat_target: ModifierStorageType::BitMasked(physical_attr_tag),
                value: ModifierValue::Flat(ValueType::from_float(3.0)),
                dependencies: HashSet::new(),
            }, ModifierTarget {
                modifier_collection: character_id,
            }))
            .id();

        // 3. Modifier that affects all attributes
        let all_attr_modifier_id = app
            .world_mut()
            .spawn((ModifierInstance {
                target_stat: "ATTRIBUTE".to_string(),
                modifier_stat_target: ModifierStorageType::BitMasked(all_attr_tag),
                value: ModifierValue::Flat(ValueType::from_float(1.0)),
                dependencies: HashSet::new(),
            }, ModifierTarget {
                modifier_collection: character_id,
            }))
            .id();

        // Run systems
        app.update();

        // Verify the modifiers were applied correctly according to bitmask qualification
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength - should have str_modifier and possibly others depending on implementation
            let strength_stat = stat_collection.stats.get("ATTRIBUTE").unwrap();
            if let ModifierStorage::BitMasked(storage) = &strength_stat.modifier_collection {
                // The strength tag should be present in storage
                assert!(storage.tags.contains_key(&str_tag), "Strength tag should be present");

                // Check that appropriate modifiers were applied
                // Implementation-specific check - you might need to adjust based on your exact structure
                let mut has_str_modifier = false;
                for (_, intermediate_value) in &storage.tags {
                    if intermediate_value.entities.contains(&str_modifier_id) {
                        has_str_modifier = true;
                        break;
                    }
                }
                assert!(has_str_modifier, "Strength-specific modifier should be applied");
            } else if let ModifierStorage::Single(storage) = &strength_stat.modifier_collection {
                // If using Single storage, just check that modifier was applied
                assert!(storage.modifiers.contains_key(&str_modifier_id),
                        "Strength-specific modifier should be applied");
            }

            // Check dexterity - should have physical_modifier but not str_modifier
            let dexterity_stat = stat_collection.stats.get("ATTRIBUTE").unwrap();
            if let ModifierStorage::BitMasked(storage) = &dexterity_stat.modifier_collection {
                // The dexterity tag should be present in storage
                assert!(storage.tags.contains_key(&dex_tag), "Dexterity tag should be present");

                // Should have physical modifier but not strength modifier
                let mut has_physical_modifier = false;
                let mut has_str_modifier = false;
                for (_, intermediate_value) in &storage.tags {
                    if intermediate_value.entities.contains(&physical_modifier_id) {
                        has_physical_modifier = true;
                    }
                    if intermediate_value.entities.contains(&str_modifier_id) {
                        has_str_modifier = true;
                    }
                }
                assert!(has_physical_modifier, "Physical modifier should be applied to dexterity");
                assert!(!has_str_modifier, "Strength modifier should NOT be applied to dexterity");
            } else if let ModifierStorage::Single(storage) = &dexterity_stat.modifier_collection {
                // If using Single storage, check appropriate modifiers
                assert!(storage.modifiers.contains_key(&physical_modifier_id),
                        "Physical modifier should be applied to dexterity");
                assert!(!storage.modifiers.contains_key(&str_modifier_id),
                        "Strength modifier should NOT be applied to dexterity");
            }

            // Check constitution - should have all_attr_modifier but not the others
            let constitution_stat = stat_collection.stats.get("ATTRIBUTE").unwrap();
            if let ModifierStorage::BitMasked(storage) = &constitution_stat.modifier_collection {
                // The constitution tag should be present in storage
                assert!(storage.tags.contains_key(&con_tag), "Constitution tag should be present");

                // Should have all_attr_modifier but not the others
                let mut has_all_attr_modifier = false;
                for (_, intermediate_value) in &storage.tags {
                    if intermediate_value.entities.contains(&all_attr_modifier_id) {
                        has_all_attr_modifier = true;
                        break;
                    }
                }
                assert!(has_all_attr_modifier, "All-attribute modifier should be applied to constitution");
            } else if let ModifierStorage::Single(storage) = &constitution_stat.modifier_collection {
                // If using Single storage, check appropriate modifiers
                assert!(storage.modifiers.contains_key(&all_attr_modifier_id),
                        "All-attribute modifier should be applied to constitution");
            }
        }

        // Now test removing a compound modifier
        app.world_mut().despawn(physical_modifier_id);
        app.update();

        // Verify the physical modifier was removed but others remain
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check dexterity no longer has the physical modifier
            let dexterity_stat = stat_collection.stats.get("ATTRIBUTE").unwrap();
            if let ModifierStorage::BitMasked(storage) = &dexterity_stat.modifier_collection {
                let mut has_physical_modifier = false;
                for (_, intermediate_value) in &storage.tags {
                    if intermediate_value.entities.contains(&physical_modifier_id) {
                        has_physical_modifier = true;
                        break;
                    }
                }
                assert!(!has_physical_modifier, "Physical modifier should be removed from dexterity");
            } else if let ModifierStorage::Single(storage) = &dexterity_stat.modifier_collection {
                assert!(!storage.modifiers.contains_key(&physical_modifier_id),
                        "Physical modifier should be removed from dexterity");
            }
        }
    }
}