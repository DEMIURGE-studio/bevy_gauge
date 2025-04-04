#[cfg(test)]
mod tests {
    use crate::expressions::Expression;
    use crate::modifier_events::{
        on_modifier_change, register_modifier_triggers, ModifierUpdatedEvent,
    };
    use crate::prelude::*;
    use crate::stat_events::{
        on_stat_added, register_stat_triggers, AttributeAddedEvent, AttributeUpdatedEvent,
    };
    use crate::stat_value::StatValue;
    use bevy::prelude::*;
    use std::collections::HashSet;

    fn setup_test_app() -> App {
        let mut app = App::new();
        println!("Starting app");

        // Add necessary resources
        app.init_resource::<TagRegistry>();

        // Add systems
        register_modifier_triggers(&mut app);
        register_stat_triggers(&mut app);

        app
    }

    // Strength -> 10
    // MaxLife -> 10 * Strength = 100
    // Modifier -> MaxLife * 10% = 110
    // Modifier -> Strength * 10% -> 11
    // MaxLife -> 10 * Strength = 110
    // MaxLife -> MaxLife * 10% -> 121

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
            value: ModifierValue::Flat(StatValue::from_f32(value)),
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

        let strength_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add the strength stat manually
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "strength".to_string(),
                    value: StatValue::from_f32(0.0),
                },
                character_id,
            );
        }

        // Create a modifier targeting the strength stat
        let modifier_id = app
            .world_mut()
            .spawn((
                create_bitmask_modifier("strength", strength_tag, 5.0),
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the modifier was applied
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Check if attribute group exists and strength is in it
        assert!(
            stat_collection.attributes.contains_key("attribute"),
            "Attribute group should exist"
        );
        assert!(
            stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .contains_key(&strength_tag),
            "Strength attribute should exist"
        );

        // Check the modifier was applied
        let strength_attr = stat_collection
            .attributes
            .get("attribute")
            .unwrap()
            .get(&strength_tag)
            .unwrap();

        // The modifier should be in the storage
        assert!(
            strength_attr.read().unwrap().modifier_collection.contains_key(&modifier_id),
            "Modifier should be present in the attribute"
        );

        // Check the modifier value
        let strength_attribute = strength_attr.read().unwrap();
        let modifier_value = strength_attribute.modifier_collection.get(&modifier_id).unwrap();
        match modifier_value {
            ModifierValue::Flat(stat_value) => {
                assert!(
                    (stat_value.get_value_f32() - 5.0).abs() < 0.001,
                    "Expected modifier value to be 5.0"
                );
            }
            _ => panic!("Expected Flat/Literal modifier"),
        }

        // Check total value (base value + modifier)
        let strength_value = strength_attribute.get_total_value_f32();
        assert!(
            (strength_value - 5.0).abs() < 0.001,
            "Total strength should be 5.0, got {}",
            strength_value
        );
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
        let strength_tag = app
            .world_mut()
            .resource_mut::<TagRegistry>()
            .register_tag("attribute", "strength");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add the strength stat manually
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "strength".to_string(),
                    value: StatValue::from_f32(0.0),
                },
                character_id,
            );
        }

        // Create a bitmask modifier targeting the strength stat
        let modifier_id = app
            .world_mut()
            .spawn((
                create_bitmask_modifier("strength", strength_tag, 3.0),
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Get the stat collection
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Check if attribute group exists and strength is in it
        assert!(
            stat_collection.attributes.contains_key("attribute"),
            "Attribute group should exist"
        );
        assert!(
            stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .contains_key(&strength_tag),
            "Strength attribute should exist"
        );

        // Check the modifier was applied
        let strength_attr = stat_collection
            .attributes
            .get("attribute")
            .unwrap()
            .get(&strength_tag)
            .unwrap()
            .read()
            .unwrap();
        
        // The modifier should be in the storage
        assert!(
            strength_attr.modifier_collection.contains_key(&modifier_id),
            "Modifier should be present in the attribute"
        );

        // Check the modifier value
        let modifier_value = strength_attr.modifier_collection.get(&modifier_id).unwrap();
        match modifier_value {
            ModifierValue::Flat(val) => {
                assert!(
                    (val.get_value_f32() - 3.0).abs() < 0.001,
                    "Expected modifier value to be 3.0"
                );
            }
            _ => panic!("Expected Flat/Literal modifier"),
        }

        // Check total value (base value + modifier)
        let strength_value = strength_attr.get_total_value_f32();
        assert!(
            (strength_value - 3.0).abs() < 0.001,
            "Total strength should be 3.0, got {}",
            strength_value
        );
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

        let strength_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        let damage_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "damage")
            .expect("Damage tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add stats with dependencies manually
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "strength".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );
            // Add base strength stat

            // Create a damage stat that depends on strength
            // Using an expression that references strength
            let damage_expr =
                Expression::new(evalexpr::build_operator_tree("attribute.strength * 0.5").unwrap());
            let damage_value = StatValue::from_expression(damage_expr);

            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "damage".to_string(),
                    value: damage_value,
                },
                character_id,
            );
        }

        // Run the app to process the systems
        app.update();

        // Check the dependencies were set up correctly
        let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

        // Get the attributes
        let strength_attr = stat_collection
            .attributes
            .get("attribute")
            .unwrap()
            .get(&strength_tag)
            .unwrap()
            .read()
            .unwrap();
        let damage_attr = stat_collection
            .attributes
            .get("attribute")
            .unwrap()
            .get(&damage_tag)
            .unwrap()
            .read()
            .unwrap();

        // Check that damage depends on strength
        if let Some(dependents) = &strength_attr.dependent_attributes {
            // Check if damage is listed as a dependent of strength
            let attribute_dependents = dependents.get("attribute");
            assert!(
                attribute_dependents.is_some(),
                "Strength should have attribute dependents"
            );
            assert!(
                attribute_dependents.unwrap().contains(&damage_tag),
                "Damage should be a dependent of strength"
            );
        } else {
            panic!("Strength should have dependents");
        }

        // Check if damage has strength as a dependency
        if let Some(dependencies) = &damage_attr.dependencies {
            // Check if strength is listed as a dependency of damage
            let attribute_deps = dependencies.get("attribute");
            assert!(
                attribute_deps.is_some(),
                "Damage should have attribute dependencies"
            );
            assert!(
                attribute_deps.unwrap().contains(&strength_tag),
                "Strength should be a dependency of damage"
            );
        } else {
            panic!("Damage should have dependencies");
        }

        // Check damage value (should be strength * 0.5 = 10 * 0.5 = 5.0)
        let damage_value = damage_attr.get_total_value_f32();
        assert!(
            (damage_value - 5.0).abs() < 0.001,
            "Damage should be 5.0 (strength * 0.5), got {}",
            damage_value
        );
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
        let strength_tag = app
            .world_mut()
            .resource_mut::<TagRegistry>()
            .register_tag("attribute", "strength");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add the strength stat manually
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "strength".to_string(),
                    value: StatValue::from_f32(0.0),
                },
                character_id,
            );
        }

        // Create and apply two modifiers
        let flat_modifier_id = app
            .world_mut()
            .spawn((
                create_bitmask_modifier("strength", strength_tag, 3.0),
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        let more_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("attribute".to_string(), strength_tag),
                    value: ModifierValue::More(StatValue::from_f32(0.2)),
                    dependencies: HashSet::new(),
                },
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify both modifiers were applied
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            let strength_attr = stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .get(&strength_tag)
                .unwrap()
                .read()
                .unwrap();

            // Check modifiers are present
            assert!(
                strength_attr
                    .modifier_collection
                    .contains_key(&flat_modifier_id),
                "Flat modifier should be present"
            );
            assert!(
                strength_attr
                    .modifier_collection
                    .contains_key(&more_modifier_id),
                "More modifier should be present"
            );

            // Check flat modifier value
            let flat_modifier_value = strength_attr
                .modifier_collection
                .get(&flat_modifier_id)
                .unwrap();
            match flat_modifier_value {
                ModifierValue::Flat(val) => {
                    assert!(
                        (val.get_value_f32() - 3.0).abs() < 0.001,
                        "Flat modifier value should be 3.0"
                    );
                }
                _ => panic!("Expected Flat modifier"),
            }

            // Check more modifier value
            let more_modifier_value = strength_attr
                .modifier_collection
                .get(&more_modifier_id)
                .unwrap();
            match more_modifier_value {
                ModifierValue::More(val) => {
                    assert!(
                        (val.get_value_f32() - 0.2).abs() < 0.001,
                        "More modifier value should be 0.2"
                    );
                }
                _ => panic!("Expected More modifier"),
            }

            // Check total value calculation: (base + flat) * (1 + more) = (0 + 3) * (1 + 0.2) = 3.6
            let strength_value = strength_attr.get_total_value_f32();
            assert!(
                (strength_value - 3.6).abs() < 0.001,
                "Total strength should be 3.6, got {}",
                strength_value
            );
        }

        // Remove the more modifier
        app.world_mut().despawn(more_modifier_id);
        app.update();

        // Verify only flat modifier remains
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            let strength_attr = stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .get(&strength_tag)
                .unwrap()
                .read()
                .unwrap();

            // Check flat modifier is still present, more modifier is gone
            assert!(
                strength_attr
                    .modifier_collection
                    .contains_key(&flat_modifier_id),
                "Flat modifier should still be present"
            );
            assert!(
                !strength_attr
                    .modifier_collection
                    .contains_key(&more_modifier_id),
                "More modifier should be removed"
            );

            // Check total value (should be base + flat = 0 + 3 = 3.0)
            let strength_value = strength_attr.get_total_value_f32();
            assert!(
                (strength_value - 3.0).abs() < 0.001,
                "Total strength should be 3.0, got {}",
                strength_value
            );
        }

        // Remove the flat modifier too
        app.world_mut().despawn(flat_modifier_id);
        app.update();

        // Verify no modifiers remain
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            let strength_attr = stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .get(&strength_tag)
                .unwrap()
                .read()
                .unwrap();

            // Check all modifiers are gone
            assert!(
                !strength_attr
                    .modifier_collection
                    .contains_key(&flat_modifier_id),
                "Flat modifier should be removed"
            );
            assert!(
                !strength_attr
                    .modifier_collection
                    .contains_key(&more_modifier_id),
                "More modifier should be removed"
            );

            // Check total value (should be just base = 0.0)
            let strength_value = strength_attr.get_total_value_f32();
            assert!(
                (strength_value - 0.0).abs() < 0.001,
                "Total strength should be 0.0, got {}",
                strength_value
            );
        }
    }

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

        let strength_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        let damage_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "damage")
            .expect("Damage tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add stats with dependencies manually
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "strength".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );
            // Add base strength stat

            // Create a damage stat that depends on strength
            // Using an expression that references strength
            let damage_expr =
                Expression::new(evalexpr::build_operator_tree("attribute.strength * 0.5").unwrap());
            let damage_value = StatValue::from_expression(damage_expr);

            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "damage".to_string(),
                    value: damage_value,
                },
                character_id,
            );
        }
        println!("before modifier");

        // Create a modifier for strength
        let strength_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("attribute".to_string(), strength_tag),
                    value: ModifierValue::Flat(StatValue::from_f32(5.0)),
                    dependencies: HashSet::new(),
                },
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        println!("strength modifier: {:?}", strength_modifier_id);
        // Run the app to process the systems
        app.update();

        // Verify the modifier affects both stats through dependency
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength value (10 base + 5 modifier = 15)
            let strength_attr = stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .get(&strength_tag)
                .unwrap()
                .read()
                .unwrap();
            let strength_value = strength_attr.get_total_value_f32();
            assert!(
                (strength_value - 15.0).abs() < 0.001,
                "Strength should be 15.0, got {}",
                strength_value
            );

            // Check damage value (damage = strength * 0.5 = 15 * 0.5 = 7.5)
            let damage_attr = stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .get(&damage_tag)
                .unwrap()
                .read()
                .unwrap();
            let damage_value = damage_attr.get_total_value_f32();
            assert!(
                (damage_value - 7.5).abs() < 0.001,
                "Damage should be 7.5, got {}",
                damage_value
            );
        }

        app.update();

        // Change the strength modifier
        {
            app.world_mut().trigger_targets(
                ModifierUpdatedEvent {
                    new_value: Some(ModifierValue::Flat(StatValue::from_f32(10.0))),
                },
                strength_modifier_id,
            );
        }

        // Run the app to process the update
        app.update();

        // Verify the change propagated through dependencies
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength value (10 base + 10 modifier = 20)
            // let strength_attr = stat_collection.attributes.get("attribute").unwrap().get(&strength_tag).unwrap();
            // let strength_value = strength_attr.get_total_value_f32();
            let strength_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), strength_tag))
                .unwrap();
            assert!(
                (strength_value - 20.0).abs() < 0.001,
                "Strength should be 20.0 after update, got {}",
                strength_value
            );

            // Check damage value (damage = strength * 0.5 = 20 * 0.5 = 10.0)
            let damage_attr = stat_collection
                .attributes
                .get("attribute")
                .unwrap()
                .get(&damage_tag)
                .unwrap()
                .read()
                .unwrap();
            let damage_value = damage_attr.get_total_value_f32();
            assert!(
                (damage_value - 10.0).abs() < 0.001,
                "Damage should be 10.0 after update, got {}",
                damage_value
            );
        }
    }

    #[test]
    fn test_group_modifier_application() {
        // Setup appF
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
            tag_registry.register_tag("attribute", "strength");
            tag_registry.register_tag("attribute", "dexterity");
            tag_registry.register_tag("attribute", "intelligence");
        }

        let strength_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        let dexterity_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "dexterity")
            .expect("Dexterity tag should be registered");

        let intelligence_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "intelligence")
            .expect("Intelligence tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add multiple stats
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "strength".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );

            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "dexterity".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );

            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "intelligence".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );
        }

        // Create a global "ALL" modifier that affects all attributes
        let global_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("attribute".to_string(), u32::MAX), // ALL attributes
                    value: ModifierValue::Increased(StatValue::from_f32(0.5)), // 50% increased
                    dependencies: HashSet::new(),
                },
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the global modifier was applied to all attributes
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength value (10 base * (1 + 0.5) = 15)
            let strength_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), strength_tag))
                .unwrap();
            assert!(
                (strength_value - 15.0).abs() < 0.001,
                "Strength should be 15.0, got {}",
                strength_value
            );

            // Check dexterity value (10 base * (1 + 0.5) = 15)
            let dexterity_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), dexterity_tag))
                .unwrap();
            assert!(
                (dexterity_value - 15.0).abs() < 0.001,
                "Dexterity should be 15.0, got {}",
                dexterity_value
            );

            // Check intelligence value (10 base * (1 + 0.5) = 15)
            let intelligence_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), intelligence_tag))
                .unwrap();
            assert!(
                (intelligence_value - 15.0).abs() < 0.001,
                "Intelligence should be 15.0, got {}",
                intelligence_value
            );
        }

        // Add another modifier that affects just one attribute
        let strength_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("attribute".to_string(), strength_tag),
                    value: ModifierValue::More(StatValue::from_f32(0.2)), // 20% more
                    dependencies: HashSet::new(),
                },
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the individual modifier was applied correctly along with the global one
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength value (10 base * (1 + 0.5) * (1 + 0.2) = 15 * 1.2 = 18)
            let strength_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), strength_tag))
                .unwrap();
            assert!(
                (strength_value - 18.0).abs() < 0.001,
                "Strength should be 18.0, got {}",
                strength_value
            );

            // Other attributes should remain unchanged
            let dexterity_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), dexterity_tag))
                .unwrap();
            assert!(
                (dexterity_value - 15.0).abs() < 0.001,
                "Dexterity should still be 15.0, got {}",
                dexterity_value
            );
        }

        // Remove the global modifier
        app.world_mut().despawn(global_modifier_id);
        app.update();

        // Verify the effect was removed
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check strength value (10 base * (1 + 0.2) = 12)
            let strength_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), strength_tag))
                .unwrap();
            assert!(
                (strength_value - 12.0).abs() < 0.001,
                "Strength should be 12.0, got {}",
                strength_value
            );

            // Other attributes should go back to base value
            let dexterity_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), dexterity_tag))
                .unwrap();
            assert!(
                (dexterity_value - 10.0).abs() < 0.001,
                "Dexterity should be back to 10.0, got {}",
                dexterity_value
            );
        }
    }

    #[test]
    fn test_compound_tag_modifiers() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("damage");
            tag_registry.register_tag("damage", "fire");
            tag_registry.register_tag("damage", "cold");
            tag_registry.register_tag("damage", "lightning");
        }

        // Get individual damage type tags
        let fire_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("damage", "fire")
            .expect("Fire tag should be registered");

        let cold_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("damage", "cold")
            .expect("Cold tag should be registered");

        let lightning_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("damage", "lightning")
            .expect("Lightning tag should be registered");

        // Create a compound "elemental" tag (fire | cold | lightning)
        let elemental_tag = fire_tag | cold_tag | lightning_tag;

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add damage stats
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "damage".to_string(),
                    attribute_name: "fire".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );

            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "damage".to_string(),
                    attribute_name: "cold".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );

            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "damage".to_string(),
                    attribute_name: "lightning".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );
        }

        // Create a modifier for all elemental damage
        let elemental_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("damage".to_string(), elemental_tag),
                    value: ModifierValue::Increased(StatValue::from_f32(0.5)), // 50% increased elemental damage
                    dependencies: HashSet::new(),
                },
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the compound modifier was applied to all elemental damage types
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check fire damage (10 base * (1 + 0.5) = 15)
            let fire_value = stat_collection
                .get_f32(AttributeId::new("damage".to_string(), fire_tag))
                .unwrap();
            assert!(
                (fire_value - 15.0).abs() < 0.001,
                "Fire damage should be 15.0, got {}",
                fire_value
            );

            // Check cold damage (10 base * (1 + 0.5) = 15)
            let cold_value = stat_collection
                .get_f32(AttributeId::new("damage".to_string(), cold_tag))
                .unwrap();
            assert!(
                (cold_value - 15.0).abs() < 0.001,
                "Cold damage should be 15.0, got {}",
                cold_value
            );

            // Check lightning damage (10 base * (1 + 0.5) = 15)
            let lightning_value = stat_collection
                .get_f32(AttributeId::new("damage".to_string(), lightning_tag))
                .unwrap();
            assert!(
                (lightning_value - 15.0).abs() < 0.001,
                "Lightning damage should be 15.0, got {}",
                lightning_value
            );
        }

        // Add a specific fire damage modifier
        let fire_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("damage".to_string(), fire_tag),
                    value: ModifierValue::More(StatValue::from_f32(0.2)), // 20% more fire damage
                    dependencies: HashSet::new(),
                },
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        // Run the app to process the systems
        app.update();

        // Verify the specific modifier was applied along with the compound one
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Check fire damage (10 base * (1 + 0.5) * (1 + 0.2) = 15 * 1.2 = 18)
            let fire_value = stat_collection
                .get_f32(AttributeId::new("damage".to_string(), fire_tag))
                .unwrap();
            assert!(
                (fire_value - 18.0).abs() < 0.001,
                "Fire damage should be 18.0, got {}",
                fire_value
            );

            // Other damage types should still be at 15
            let cold_value = stat_collection
                .get_f32(AttributeId::new("damage".to_string(), cold_tag))
                .unwrap();
            assert!(
                (cold_value - 15.0).abs() < 0.001,
                "Cold damage should still be 15.0, got {}",
                cold_value
            );
        }

        // Test total combined elemental damage
    }

    #[test]
    fn test_dynamic_expression_modifier() {
        // Setup app
        let mut app = setup_test_app();

        // Setup tag registry
        {
            let mut tag_registry = app.world_mut().resource_mut::<TagRegistry>();
            tag_registry.register_primary_type("attribute");
            tag_registry.register_tag("attribute", "strength");
            tag_registry.register_tag("attribute", "damage");
        }

        let strength_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "strength")
            .expect("Strength tag should be registered");

        let damage_tag = app
            .world()
            .resource::<TagRegistry>()
            .get_id("attribute", "damage")
            .expect("Damage tag should be registered");

        // Create an entity with stat collection
        let character_id = app
            .world_mut()
            .spawn((StatCollection::new(), ModifierCollectionRefs::default()))
            .id();

        // Add strength stat
        {
            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "strength".to_string(),
                    value: StatValue::from_f32(10.0),
                },
                character_id,
            );

            println!("strength added");

            app.world_mut().trigger_targets(
                AttributeAddedEvent {
                    attribute_group: "attribute".to_string(),
                    attribute_name: "damage".to_string(),
                    value: StatValue::from_f32(5.0),
                },
                character_id,
            );

            println!("damage added");
        }

        // Create a modifier that depends on strength
        let mut dependency_set = HashSet::new();
        dependency_set.insert(AttributeId::new("attribute".to_string(), strength_tag));

        let dynamic_expr =
            Expression::new(evalexpr::build_operator_tree("attribute.strength * 0.1").unwrap());

        let dynamic_modifier_id = app
            .world_mut()
            .spawn((
                ModifierInstance {
                    target_stat: AttributeId::new("attribute".to_string(), damage_tag),
                    value: ModifierValue::Increased(StatValue::from_expression(dynamic_expr)),
                    dependencies: dependency_set,
                },
                ModifierTarget {
                    modifier_collection: character_id,
                },
            ))
            .id();

        println!("dynamic modifier added");

        // Run the app to process the systems
        app.update();

        app.update();
        // Verify the initial modifier effect
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();
            let modifier = app
                .world()
                .get::<ModifierInstance>(dynamic_modifier_id)
                .unwrap();
            assert_eq!(modifier.value.get_value().get_value_f32(), 1.0);
            // Expression should evaluate to strength * 0.1 = 10 * 0.1 = 1.0
            // Damage should be 5 * (1 + 1.0) = 10.0
            let damage_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), damage_tag))
                .unwrap();
            assert!(
                (damage_value - 10.0).abs() < 0.001,
                "Damage should be 10.0, got {}",
                damage_value
            );
        }

        // Update strength
        {
            app.world_mut().trigger_targets(
                AttributeUpdatedEvent {
                    stat_id: AttributeId::new("attribute".to_string(), strength_tag),
                    value: Some(StatValue::from_f32(20.0)),
                },
                character_id,
            );
        }

        // Run the app to process the update
        app.update();

        // Verify the modifier was updated automatically
        {
            let stat_collection = app.world().get::<StatCollection>(character_id).unwrap();

            // Expression should now evaluate to strength * 0.1 = 20 * 0.1 = 2.0
            // Damage should be 5 * (1 + 2.0) = 15.0
            let damage_value = stat_collection
                .get_f32(AttributeId::new("attribute".to_string(), damage_tag))
                .unwrap();
            assert!(
                (damage_value - 15.0).abs() < 0.001,
                "Damage should be 15.0 after strength update, got {}",
                damage_value
            );
        }
    }
}
