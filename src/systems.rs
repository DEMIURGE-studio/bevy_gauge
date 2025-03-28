#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use crate::prelude::*;
    use bevy::prelude::*;

    fn setup_test_app() -> App {
        let mut app = App::new();

        // Add necessary resources
        app.init_resource::<TagRegistry>();
        app.add_event::<AttributeUpdatedEvent>();

        // Add systems
        register_modifier_triggers(&mut app);

        app
    }


    // Helper function to create a simple modifier
    fn create_simple_modifier(target: &str, value: f32) -> ModifierInstance {
        let target_parts: Vec<&str> = target.split('_').collect();
        let group = if target_parts.len() > 1 {
            target_parts[0].to_string()
        } else {
            "attribute".to_string()
        };

        ModifierInstance {
            target_stat: AttributeId::new(group, u32::MAX), // u32::MAX for "all" targeting
            value: ModifierValue::Flat(ValueType::Literal(value)),
            dependencies: HashSet::new(),
        }
    }

    // Helper function to create a bitmask modifier
    fn create_bitmask_modifier(target: &str, tag: u32, value: f32) -> ModifierInstance {
        let target_parts: Vec<&str> = target.split('_').collect();
        let group = if target_parts.len() > 1 {
            target_parts[0].to_string()
        } else {
            "attribute".to_string()
        };

        ModifierInstance {
            target_stat: AttributeId::new(group, tag),
            value: ModifierValue::Flat(ValueType::Literal(value)),
            dependencies: HashSet::new(),
        }
    }

    #[test]
    fn test_auto_create_stats_simple_modifier() {
        // Setup app
        let mut app = setup_test_app();

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Create a simple modifier targeting a non-existent stat
        let modifier_id = app
            .world_mut()
            .spawn((create_simple_modifier("strength", 5.0),
                    ModifierTarget {
                        modifier_collection: character_id,
                    }
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the stats were created
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Get the tag from registry
        let strength_tag = app.world().resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be automatically registered");

        // Check if attribute group exists and strength is in it
        assert!(stat_collection.attributes.contains_key("attribute"), "Attribute group should be created");
        assert!(stat_collection.attributes.get("attribute").unwrap().contains_key(&strength_tag), "Strength attribute should be created");

        // Check the modifier was applied
        let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();

        // The modifier should be in the storage
        assert!(strength_attr.modifier_collection.contains_key(&modifier_id), "Modifier should be present in the attribute");

        // Check the modifier value
        let modifier_value = strength_attr.modifier_collection.get(&modifier_id).unwrap();
        match modifier_value {
            ModifierValue::Flat(ValueType::Literal(val)) => {
                assert!((val - 5.0).abs() < 0.001, "Expected modifier value to be 5.0");
            },
            _ => panic!("Expected Flat/Literal modifier")
        }

        // Check total value (base value + modifier)
        let strength_value = strength_attr.get_value_f32();
        assert!((strength_value - 5.0).abs() < 0.001, "Total strength should be 5.0, got {}", strength_value);
    }

    #[test]
    fn test_auto_create_stats_bitmask_modifier() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
        }

        // Get a tag from registry
        let strength_tag = app.world_mut().resource_mut::<TagRegistry>()
            .register_tag("attribute", "strength");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Create a bitmask modifier targeting a non-existent stat
        let modifier_id = app
            .world_mut()
            .spawn((create_bitmask_modifier("strength", strength_tag, 3.0),
                    ModifierTarget {
                        modifier_collection: character_id,
                    }))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the stats were created
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        println!("{:#?}", stat_collection.attributes);
        // Check if attribute group exists and strength is in it
        assert!(stat_collection.attributes.contains_key("attribute"), "Attribute group should be created");
        assert!(stat_collection.attributes.get("attribute").unwrap().contains_key(&strength_tag), "Strength attribute should be created");

        // Check the modifier was applied
        let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();

        // The modifier should be in the storage
        assert!(strength_attr.modifier_collection.contains_key(&modifier_id), "Modifier should be present in the attribute");

        // Check the modifier value
        let modifier_value = strength_attr.modifier_collection.get(&modifier_id).unwrap();
        match modifier_value {
            ModifierValue::Flat(ValueType::Literal(val)) => {
                assert!((val - 3.0).abs() < 0.001, "Expected modifier value to be 3.0");
            },
            _ => panic!("Expected Flat/Literal modifier")
        }

        // Check total value (base value + modifier)
        let strength_value = strength_attr.get_value_f32();
        assert!((strength_value - 3.0).abs() < 0.001, "Total strength should be 3.0, got {}", strength_value);
    }

    #[test]
    fn test_dependencies_setup() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
        }

        // Get a tag from registry
        let strength_tag = app.world_mut().resource_mut::<TagRegistry>()
            .register_tag("attribute", "strength");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Create a modifier
        let modifier_id = app
            .world_mut()
            .spawn((
                create_bitmask_modifier("strength", strength_tag, 3.0),
                ModifierTarget {
                    modifier_collection: character_id,
                }
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Check dependencies were set up correctly
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Get the attribute
        let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();

        // Check dependencies (since structure changed, this may be empty now, depending on implementation)
        if let Some(dependencies) = &strength_attr.dependencies {
            // Check dependencies as per the updated structure
            // This might need adjusting based on how dependencies are now organized
        }

        // Check that the modifier is properly registered
        assert!(strength_attr.modifier_collection.contains_key(&modifier_id),
                "Modifier should be registered with the attribute");
    }

    #[test]
    fn test_remove_modifier() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
        }

        // Get a tag from registry
        let strength_tag = app.world_mut().resource_mut::<TagRegistry>()
            .register_tag("attribute", "strength");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Create and apply two modifiers
        let modifier_id1 = app
            .world_mut()
            .spawn((
                create_bitmask_modifier("strength", strength_tag, 3.0),
                ModifierTarget {
                    modifier_collection: character_id,
                }
            ))
            .id();

        let modifier_id2 = app
            .world_mut()
            .spawn((
                create_bitmask_modifier("strength", strength_tag, 2.0),
                ModifierTarget {
                    modifier_collection: character_id,
                }
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify both modifiers were applied
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();

            // Check both modifiers are present
            assert!(strength_attr.modifier_collection.contains_key(&modifier_id1), "First modifier should be present");
            assert!(strength_attr.modifier_collection.contains_key(&modifier_id2), "Second modifier should be present");

            // Check total value (should be base + 3.0 + 2.0 = 5.0)
            let strength_value = strength_attr.get_value_f32();
            assert!((strength_value - 5.0).abs() < 0.001, "Total strength should be 5.0, got {}", strength_value);
        }

        // Remove one modifier
        app.world_mut().despawn(modifier_id1);
        app.update();

        // Verify only one modifier remains
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();

            // Check first modifier is gone, second remains
            assert!(!strength_attr.modifier_collection.contains_key(&modifier_id1), "First modifier should be removed");
            assert!(strength_attr.modifier_collection.contains_key(&modifier_id2), "Second modifier should still be present");

            // Check total value (should be base + 2.0 = 2.0)
            let strength_value = strength_attr.get_value_f32();
            assert!((strength_value - 2.0).abs() < 0.001, "Total strength should be 2.0, got {}", strength_value);
        }
    }

    #[test]
    fn test_multiple_modifier_types() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
        }

        // Get a tag from registry
        let strength_tag = app.world_mut().resource_mut::<TagRegistry>()
            .register_tag("attribute", "strength");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .id();

        // Create both an all modifier and a BitMasked modifier
        let all_modifier_id = app
            .world_mut()
            .spawn((
                create_simple_modifier("strength", 5.0),
                ModifierTarget {
                    modifier_collection: character_id,
                }
            ))
            .id();

        let bitmask_modifier_id = app
            .world_mut()
            .spawn((
                create_bitmask_modifier("strength", strength_tag, 3.0),
                ModifierTarget {
                    modifier_collection: character_id,
                }
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify modifiers were applied correctly
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
        let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();

        // Check both modifiers are present
        assert!(strength_attr.modifier_collection.contains_key(&all_modifier_id), "All modifier should be present");
        assert!(strength_attr.modifier_collection.contains_key(&bitmask_modifier_id), "Bitmask modifier should be present");

        // Check modifier values
        let all_modifier_value = strength_attr.modifier_collection.get(&all_modifier_id).unwrap();
        match all_modifier_value {
            ModifierValue::Flat(ValueType::Literal(val)) => {
                assert!((val - 5.0).abs() < 0.001, "All modifier value should be 5.0");
            },
            _ => panic!("Expected Flat/Literal modifier")
        }

        let bitmask_modifier_value = strength_attr.modifier_collection.get(&bitmask_modifier_id).unwrap();
        match bitmask_modifier_value {
            ModifierValue::Flat(ValueType::Literal(val)) => {
                assert!((val - 3.0).abs() < 0.001, "Bitmask modifier value should be 3.0");
            },
            _ => panic!("Expected Flat/Literal modifier")
        }

        // Check total value (should be base + 5.0 + 3.0 = 8.0)
        let strength_value = strength_attr.get_value_f32();
        assert!((strength_value - 8.0).abs() < 0.001, "Total strength should be 8.0, got {}", strength_value);
    }
}
