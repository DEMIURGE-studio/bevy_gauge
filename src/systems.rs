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

    // Helper function to add a stat to the collection
    fn add_stat_to_collection(
        stat_collection: &mut StatCollection,
        group: &str,
        name: &str,
        value: f32,
        tag_registry: &Res<TagRegistry>
    ) {
        stat_collection.add_attribute(
            group,
            name,
            StatValue::from_f32(value),
            tag_registry
        );
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
    fn test_add_modifier_to_stat() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
            tag_registry.register_tag("attribute", "strength");
        }

        let strength_tag = app.world().resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .observe(on_stat_added)
            .id();

        // Add the strength stat manually
        {
            app.world_mut().trigger_targets(AttributeAddedEvent {attribute_group: "attribute".to_string(), attribute_name: "strength".to_string(), value: StatValue::from_f32(0.0)}, character_id);
        }

        // Create a modifier targeting the strength stat
        let modifier_id = app
            .world_mut()
            .spawn((create_bitmask_modifier("strength", strength_tag, 5.0),
                    ModifierTarget {
                        modifier_collection: character_id,
                    }
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the modifier was applied
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Check if attribute group exists and strength is in it
        assert!(stat_collection.attributes.contains_key("attribute"), "Attribute group should exist");
        assert!(stat_collection.attributes.get("attribute").unwrap().contains_key(&strength_tag), "Strength attribute should exist");

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
        let strength_value = strength_attr.get_total_value_f32();
        assert!((strength_value - 5.0).abs() < 0.001, "Total strength should be 5.0, got {}", strength_value);
    }

    #[test]
    fn test_bitmask_modifier() {
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
            .observe(on_stat_added)
            .id();

        // Add the strength stat manually
        {

            app.world_mut().trigger_targets(AttributeAddedEvent {attribute_group: "attribute".to_string(), attribute_name: "strength".to_string(), value: StatValue::from_f32(0.0)}, character_id);
        }

        // Create a bitmask modifier targeting the strength stat
        let modifier_id = app
            .world_mut()
            .spawn((create_bitmask_modifier("strength", strength_tag, 3.0),
                    ModifierTarget {
                        modifier_collection: character_id,
                    }))
            .id();

        // Run the app to process the systems
        app.update();

        // Get the stat collection
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Check if attribute group exists and strength is in it
        assert!(stat_collection.attributes.contains_key("attribute"), "Attribute group should exist");
        assert!(stat_collection.attributes.get("attribute").unwrap().contains_key(&strength_tag), "Strength attribute should exist");

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
        let strength_value = strength_attr.get_total_value_f32();
        assert!((strength_value - 3.0).abs() < 0.001, "Total strength should be 3.0, got {}", strength_value);
    }

    #[test]
    fn test_stat_dependencies() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
            tag_registry.register_tag("attribute", "strength");
            tag_registry.register_tag("attribute", "damage");
        }

        let strength_tag = app.world().resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        let damage_tag = app.world().resource::<TagRegistry>()
            .get_id("attribute", "damage")
            .expect("Damage tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .observe(on_stat_added)
            .id();

        // Add stats with dependencies manually
        {

            app.world_mut().trigger_targets(AttributeAddedEvent {attribute_group: "attribute".to_string(), attribute_name: "strength".to_string(), value: StatValue::from_f32(10.0)}, character_id);
            // Add base strength stat

            // Create a damage stat that depends on strength
            // Using an expression that references strength
            let damage_expr = Expression::new(
                evalexpr::build_operator_tree("attribute.strength * 0.5").unwrap()
            );
            let damage_value = StatValue::from_expression(damage_expr);


            app.world_mut().trigger_targets(AttributeAddedEvent {attribute_group: "attribute".to_string(), attribute_name: "damage".to_string(), value: damage_value}, character_id);
        }

        // Run the app to process the systems
        app.update();

        // Check the dependencies were set up correctly
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Get the attributes
        let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();
        let damage_attr = stat_collection.attributes.get("attribute").unwrap().get(&damage_tag).unwrap();

        // Check that damage depends on strength
        if let Some(dependents) = &strength_attr.dependent_attributes {
            // Check if damage is listed as a dependent of strength
            let attribute_dependents = dependents.get("attribute");
            assert!(attribute_dependents.is_some(), "Strength should have attribute dependents");
            assert!(attribute_dependents.unwrap().contains(&damage_tag),
                    "Damage should be a dependent of strength");
        } else {
            panic!("Strength should have dependents");
        }

        // Check if damage has strength as a dependency
        if let Some(dependencies) = &damage_attr.dependencies {
            // Check if strength is listed as a dependency of damage
            let attribute_deps = dependencies.get("attribute");
            assert!(attribute_deps.is_some(), "Damage should have attribute dependencies");
            assert!(attribute_deps.unwrap().contains(&strength_tag),
                    "Strength should be a dependency of damage");
        } else {
            panic!("Damage should have dependencies");
        }

        // Check damage value (should be strength * 0.5 = 10 * 0.5 = 5.0)
        let damage_value = damage_attr.get_total_value_f32();
        assert!((damage_value - 5.0).abs() < 0.001,
                "Damage should be 5.0 (strength * 0.5), got {}", damage_value);
    }

    // #[test]
    // fn test_remove_modifier() {
    //     // Setup app
    //     let mut app = setup_test_app();
    // 
    //     // Setup tag registry
    //     {
    //         let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
    //         tag_registry.register_primary_type("attribute");
    //     }
    // 
    //     // Get a tag from registry
    //     let strength_tag = app.world_mut().resource_mut::<TagRegistry>()
    //         .register_tag("attribute", "strength");
    // 
    //     // Create an entity with stat collection
    //     let character_id = app
    //         .world_mut()
    //         .spawn((
    //             StatCollection::new(),
    //             ModifierCollectionRefs::default(),
    //         ))
    //         .observe(on_modifier_change)
    //         .id();
    // 
    //     // Add the strength stat manually
    //     {
    //         let mut stat_collection = app.world_mut().get_mut::<StatCollection>(character_id).unwrap();
    //         let tag_registry = app.world().resource::<TagRegistry>();
    //         add_stat_to_collection(&mut stat_collection, "attribute", "strength", 0.0, &tag_registry);
    //     }
    // 
    //     // Create and apply two modifiers
    //     let modifier_id1 = app
    //         .world_mut()
    //         .spawn((
    //             create_bitmask_modifier("strength", strength_tag, 3.0),
    //             ModifierTarget {
    //                 modifier_collection: character_id,
    //             }
    //         ))
    //         .id();
    // 
    //     let more_modifier_id = app
    //         .world_mut()
    //         .spawn((
    //             ModifierInstance {
    //                 target_stat: AttributeId::new("attribute".to_string(), strength_tag),
    //                 value: ModifierValue::More(ValueType::Literal(0.2)), // 20% more
    //                 dependencies: HashSet::new(),
    //             },
    //             ModifierTarget {
    //                 modifier_collection: character_id,
    //             }
    //         ))
    //         .id();
    // 
    //     // Run the app to process the systems
    //     app.update();
    // 
    //     // Verify modifiers were applied correctly
    //     let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
    //     let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();
    // 
    //     // Check modifiers are present
    //     assert!(strength_attr.modifier_collection.contains_key(&flat_modifier_id), "Flat modifier should be present");
    //     assert!(strength_attr.modifier_collection.contains_key(&increased_modifier_id), "Increased modifier should be present");
    //     assert!(strength_attr.modifier_collection.contains_key(&more_modifier_id), "More modifier should be present");
    // 
    //     // Check modifier values
    //     let flat_modifier_value = strength_attr.modifier_collection.get(&flat_modifier_id).unwrap();
    //     match flat_modifier_value {
    //         ModifierValue::Flat(ValueType::Literal(val)) => {
    //             assert!((val - 5.0).abs() < 0.001, "Flat modifier value should be 5.0");
    //         },
    //         _ => panic!("Expected Flat/Literal modifier")
    //     }
    // 
    //     let increased_modifier_value = strength_attr.modifier_collection.get(&increased_modifier_id).unwrap();
    //     match increased_modifier_value {
    //         ModifierValue::Increased(ValueType::Literal(val)) => {
    //             assert!((val - 0.5).abs() < 0.001, "Increased modifier value should be 0.5");
    //         },
    //         _ => panic!("Expected Increased/Literal modifier")
    //     }
    // 
    //     let more_modifier_value = strength_attr.modifier_collection.get(&more_modifier_id).unwrap();
    //     match more_modifier_value {
    //         ModifierValue::More(ValueType::Literal(val)) => {
    //             assert!((val - 0.2).abs() < 0.001, "More modifier value should be 0.2");
    //         },
    //         _ => panic!("Expected More/Literal modifier")
    //     }
    // 
    //     // Check total value calculation
    //     // Formula should be: (base + flat) * (1 + increased) * (1 + more)
    //     // (0 + 5) * (1 + 0.5) * (1 + 0.2) = 5 * 1.5 * 1.2 = 9.0
    //     let strength_value = strength_attr.get_value_f32();
    //     assert!((strength_value - 9.0).abs() < 0.001, "Total strength should be 9.0, got {}", strength_value);
    // }

    #[test]
    fn test_modifier_update_recalculation() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
            tag_registry.register_tag("attribute", "strength");
            tag_registry.register_tag("attribute", "damage");
        }

        let strength_tag = app.world().resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        let damage_tag = app.world().resource::<TagRegistry>()
            .get_id("attribute", "damage")
            .expect("Damage tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((
                StatCollection::new(),
                ModifierCollectionRefs::default(),
            ))
            .observe(on_modifier_change)
            .observe(on_stat_added)
            .id();

        // Add stats with dependencies manually
        {
            app.world_mut().trigger_targets(AttributeAddedEvent {attribute_group: "attribute".to_string(), attribute_name: "strength".to_string(), value: StatValue::from_f32(10.0)}, character_id);
            // Add base strength stat

            // Create a damage stat that depends on strength
            // Using an expression that references strength
            let damage_expr = Expression::new(
                evalexpr::build_operator_tree("attribute.strength * 0.5").unwrap()
            );
            let damage_value = StatValue::from_expression(damage_expr);

            app.world_mut().trigger_targets(AttributeAddedEvent {attribute_group: "attribute".to_string(), attribute_name: "damage".to_string(), value: damage_value}, character_id);
        }

        // Create a modifier for strength
        let strength_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("attribute".to_string(), strength_tag),
                    value: ModifierValue::Flat(ValueType::Literal(5.0)),
                    dependencies: HashSet::new(),
                },
                ModifierTarget {
                    modifier_collection: character_id,
                }
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the modifier affects both stats through dependency
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength value (10 base + 5 modifier = 15)
            let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();
            let strength_value = strength_attr.get_total_value_f32();
            assert!((strength_value - 15.0).abs() < 0.001,
                    "Strength should be 15.0, got {}", strength_value);

            // Check damage value (damage = strength * 0.5 = 15 * 0.5 = 7.5)
            let damage_attr = stat_collection.attributes.get("attribute").unwrap().get(&damage_tag).unwrap();
            let damage_value = damage_attr.get_total_value_f32();
            assert!((damage_value - 7.5).abs() < 0.001,
                    "Damage should be 7.5, got {}", damage_value);
        }

        // Change the strength modifier
        {
            let mut modifier = app.world_mut().get_mut::<ModifierInstance>(strength_modifier_id).unwrap();
            modifier.value = ModifierValue::Flat(ValueType::Literal(10.0)); // Change from 5 to 10
        }

        // Trigger modifier update event
        app.world_mut().send_event(ModifierUpdatedEvent {
            modifier_entity: strength_modifier_id
        });

        // Run the app to process the update
        app.update();

        // Verify the change propagated through dependencies
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength value (10 base + 10 modifier = 20)
            let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();
            let strength_value = strength_attr.get_total_value_f32();
            assert!((strength_value - 20.0).abs() < 0.001,
                    "Strength should be 20.0 after update, got {}", strength_value);

            // Check damage value (damage = strength * 0.5 = 20 * 0.5 = 10.0)
            let damage_attr = stat_collection.attributes.get("attribute").unwrap().get(&damage_tag).unwrap();
            let damage_value = damage_attr.get_total_value_f32();
            assert!((damage_value - 10.0).abs() < 0.001,
                    "Damage should be 10.0 after update, got {}", damage_value);
        }

    }
}
 