
#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use crate::prelude::*;

    // Helper function for approximate equality checks
    fn assert_approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < f32::EPSILON * 100.0, "left: {}, right: {}", a, b);
    }

    // Helper system to add a modifier to a stat
    fn add_stat_modifier(
        mut stat_accessor: StatAccessorMut,
        query: Query<Entity, With<Stats>>,
    ) {
        for entity in &query {
            stat_accessor.add_modifier(entity, "Life.Added", 10.0);
        }
    }

    // Helper system to add a dependent modifier
    fn add_dependent_modifier(
        mut stat_accessor: StatAccessorMut,
        query: Query<Entity, With<Stats>>,
    ) {
        for entity in &query {
            // Add a modifier that depends on Life.Added
            stat_accessor.add_modifier(entity, "Damage.Added", "Life.Added * 0.5");
        }
    }

    // Helper system to add an entity-dependent modifier
    fn add_entity_dependency(
        mut stat_accessor: StatAccessorMut,
        query: Query<Entity, With<Stats>>,
    ) {
        if let Some(entity_iter) = query.iter().collect::<Vec<_>>().get(0..2) {
            let source = entity_iter[0];
            let target = entity_iter[1];
            
            // Register the dependency (source is known as "Source" to target)
            stat_accessor.register_dependency(target, "Source", source);
            
            // Add a modifier that depends on the source entity's Life.Added
            stat_accessor.add_modifier(target, "Damage.Added", "Source@Life.Added * 0.25");
        }
    }

    // Test simple stat creation and access
    #[test]
    fn test_add_simple_stat() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system once
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);

        // Check if the stat was added correctly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let value = stats.get("Life.Added").unwrap_or(0.0);
        assert_eq!(value, 10.0);
    }

    // Test evaluating a simple stat
    #[test]
    fn test_evaluate_simple_stat() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system once
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);

        // Query the Stats component directly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let value = stats.get("Life.Added").unwrap_or(0.0);
        assert_eq!(value, 10.0);
    }

    // Test dependent stats within one entity
    #[test]
    fn test_dependent_stats() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the systems in sequence
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_dependent_modifier);
        let _ = app.world_mut().run_system(add_dep_id);

        // Query the Stats component directly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let life_value = stats.get("Life.Added").unwrap_or(0.0);
        let damage_value = stats.evaluate_by_string("Damage.Added"); // Use evaluate_by_string for expressions
        
        assert_eq!(life_value, 10.0);
        assert_eq!(damage_value, 5.0); // Should be half of Life.Added
    }

    // Test inter-entity dependencies
    #[test]
    fn test_entity_dependent_stats() {
        // Setup app
        let mut app = App::new();

        // Spawn two entities with Stats component
        let source_entity = app.world_mut().spawn(Stats::new()).id();
        let target_entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the systems in sequence
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_entity_dependency);
        let _ = app.world_mut().run_system(add_dep_id);

        // Check if entity-dependent stat was calculated correctly
        let [source_stats, target_stats] = app.world_mut().query::<&Stats>().get_many(app.world(), [source_entity, target_entity]).unwrap();
        
        let source_life = source_stats.get("Life.Added").unwrap_or(0.0);
        let target_damage = target_stats.evaluate_by_string("Damage.Added"); // Use evaluate for expressions
        
        assert_eq!(source_life, 10.0);
        assert_eq!(target_damage, 2.5); // Should be 0.25 * source Life.Added
    }

    // Test update propagation through stat dependencies
    #[test]
    fn test_update_propagates_to_dependents() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the initial setup systems
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_dependent_modifier);
        let _ = app.world_mut().run_system(add_dep_id);
        
        // Verify initial values
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let initial_damage = stats.evaluate_by_string("Damage.Added");
        assert_eq!(initial_damage, 5.0);
        
        // Register and run a system to increase Life.Added
        let increase_life_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(entity, "Life.Added", 5.0);
        });
        let _ = app.world_mut().run_system(increase_life_id);
        
        // Check if dependent stat was updated
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let updated_life = updated_stats.get("Life.Added").unwrap_or(0.0);
        let updated_damage = updated_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(updated_life, 15.0); // Original 10 + 5 added
        assert_eq!(updated_damage, 7.5); // Should be half of updated Life.Added
    }

    // Test updating source entity affects dependent entity
    #[test]
    fn test_updating_source_updates_dependent() {
        // Setup app
        let mut app = App::new();

        // Spawn two entities with Stats component
        let source_entity = app.world_mut().spawn(Stats::new()).id();
        let target_entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the initial setup systems
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_entity_dependency);
        let _ = app.world_mut().run_system(add_dep_id);
        
        // Verify initial values
        let [source_stats, target_stats] = app.world_mut().query::<&Stats>().get_many(app.world(), [source_entity, target_entity]).unwrap();
        
        let initial_source_life = source_stats.get("Life.Added").unwrap_or(0.0);
        let initial_target_damage = target_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(initial_source_life, 10.0);
        assert_eq!(initial_target_damage, 2.5); // Should be 0.25 * source Life.Added
        
        // Register and run a system to update the source entity
        let update_source_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(source_entity, "Life.Added", 10.0);
        });
        let _ = app.world_mut().run_system(update_source_id);
        
        // Check if target entity's stat was updated
        let [source_stats, target_stats] = app.world_mut().query::<&Stats>().get_many(app.world(), [source_entity, target_entity]).unwrap();
        
        let updated_source_life = source_stats.get("Life.Added").unwrap_or(0.0);
        let updated_target_damage = target_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(updated_source_life, 20.0); // Original 10 + 10 added
        assert_eq!(updated_target_damage, 5.0); // Should be 0.25 * updated source Life.Added
    }

    // Test complex modifiable stat with tags
    #[test]
    fn test_complex_modifiable_stat() {
        // Define bit flags similar to the damage_type_tags test
        const TAG1: u32 = 0x01;
        const TAG2: u32 = 0x02;
        const TAG3: u32 = 0x04;
        
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system that adds complex stats
        let add_complex_stat_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Add stats with bitflag tags
                stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", TAG1), 10.0); // Tag 1
                stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", TAG2), 5.0);  // Tag 2
                stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", TAG3), 15.0); // Tag 3
            }
        });
        let _ = app.world_mut().run_system(add_complex_stat_id);

        // Check complex stat values by tag
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        // Use evaluate_by_string with the bitflag format
        let tag1_value = stats.evaluate_by_string(&format!("Damage.{}", TAG1));
        let tag2_value = stats.evaluate_by_string(&format!("Damage.{}", TAG2));
        let tag3_value = stats.evaluate_by_string(&format!("Damage.{}", TAG3));
        
        // Check that each tag evaluates to its own value
        assert_eq!(tag1_value, 10.0);
        assert_eq!(tag2_value, 5.0);
        assert_eq!(tag3_value, 15.0);
        
        // Additional test for combined tags - they should not be combined in this case
        // since we're testing individual tag access
        let combined_value = stats.evaluate_by_string(&format!("Damage.{}", TAG1 | TAG2));
        assert_eq!(combined_value, 0.0); // No value is set for the combined tags
    }

    // Test modifier removal
    #[test]
    fn test_modifier_removal() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to add a modifier
        let add_mod_id = app.world_mut().register_system(|mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Life.Added", 10.0);
            }
        });
        let _ = app.world_mut().run_system(add_mod_id);
        
        // Verify the initial value
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let initial_value = stats.get("Life.Added").unwrap_or(0.0);
        assert_eq!(initial_value, 10.0);
        
        // Register and run a system to remove the modifier
        let remove_mod_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.remove_modifier(entity, "Life.Added", 10.0);
        });
        let _ = app.world_mut().run_system(remove_mod_id);
        
        // Check if modifier was removed correctly
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let updated_value = updated_stats.get("Life.Added").unwrap_or(0.0);
        assert_eq!(updated_value, 0.0);
    }

    // Test multi-level dependency chain
    #[test]
    fn test_multi_level_dependency_chain() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to set up the dependency chain
        let setup_chain_id = app.world_mut().register_system(|mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Base", 10.0);
                stat_accessor.add_modifier(entity, "Level1", "Base * 2");
                stat_accessor.add_modifier(entity, "Level2", "Level1 + 5");
                stat_accessor.add_modifier(entity, "Level3", "Level2 * 1.5");
            }
        });
        let _ = app.world_mut().run_system(setup_chain_id);
        
        // Verify the dependency chain values
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let base_value = stats.evaluate_by_string("Base");
        let level1_value = stats.evaluate_by_string("Level1");
        let level2_value = stats.evaluate_by_string("Level2");
        let level3_value = stats.evaluate_by_string("Level3");
        
        assert_eq!(base_value, 10.0);
        assert_eq!(level1_value, 20.0); // Base * 2
        assert_eq!(level2_value, 25.0); // Level1 + 5
        assert_eq!(level3_value, 37.5); // Level2 * 1.5
        
        // Register and run a system to update the base value
        let update_base_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(entity, "Base", 5.0);
        });
        let _ = app.world_mut().run_system(update_base_id);
        
        // Check if all levels in the chain were updated
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let updated_base = updated_stats.evaluate_by_string("Base");
        let updated_level1 = updated_stats.evaluate_by_string("Level1");
        let updated_level2 = updated_stats.evaluate_by_string("Level2");
        let updated_level3 = updated_stats.evaluate_by_string("Level3");
        
        assert_eq!(updated_base, 15.0); // Original 10 + 5 added
        assert_eq!(updated_level1, 30.0); // Updated Base * 2
        assert_eq!(updated_level2, 35.0); // Updated Level1 + 5
        assert_eq!(updated_level3, 52.5); // Updated Level2 * 1.5
    }

    // Test with damage type tags similar to your existing tests
    #[test]
    fn test_damage_type_tags() {
        // Define constants similar to your Damage enum
        const DAMAGE_TYPE: u32 = 0xFF;
        const WEAPON_TYPE: u32 = 0xFF00;
        
        const FIRE: u32 = 0x01;
        const COLD: u32 = 0x02;
        const SWORD: u32 = 0x0100;
        //const BOW: u32 = 0x0200;
        
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to add tagged damage stats
        let add_damage_stats_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Add base damage
                stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", u32::MAX), 10.0);
                
                // Add elemental damage
                stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", (u32::MAX & !DAMAGE_TYPE) | FIRE), 5.0);
                stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", (u32::MAX & !DAMAGE_TYPE) | COLD), 3.0);
                
                // Add weapon damage
                stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", (u32::MAX & !WEAPON_TYPE) | SWORD), 2.0);
                
                // Add increased damage multipliers
                stat_accessor.add_modifier(entity, &format!("Damage.Increased.{}", (u32::MAX & !DAMAGE_TYPE) | FIRE), 0.2);
                stat_accessor.add_modifier(entity, &format!("Damage.Increased.{}", (u32::MAX & !WEAPON_TYPE) | SWORD), 0.1);
            }
        });
        let _ = app.world_mut().run_system(add_damage_stats_id);
        
        // Check complex tag-based stat values
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        // Calculate the expected value
        let expected_added = 10.0 + 5.0 + 2.0; // Base + Fire + Sword
        let expected_increased = 1.0 + 0.2 + 0.1; // Base + Fire + Sword
        let expected_damage = expected_added * expected_increased;
        
        let actual_damage = stats.evaluate_by_string(&format!("Damage.{}", FIRE | SWORD));
        
        assert_approx_eq(actual_damage, expected_damage);
    }

    // Test multiple levels of entity dependencies (A -> B -> C)
    #[test]
    fn test_multi_level_entity_dependencies() {
        // Setup app
        let mut app = App::new();

        // Spawn three entities with Stats component
        let entity_c = app.world_mut().spawn(Stats::new()).id();
        let entity_b = app.world_mut().spawn(Stats::new()).id();
        let entity_a = app.world_mut().spawn(Stats::new()).id();

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Set base values for entity C
            stat_accessor.add_modifier(entity_c, "Power.Added", 10.0);
            
            // Entity B depends on C
            stat_accessor.register_dependency(entity_b, "Source", entity_c);
            stat_accessor.add_modifier(entity_b, "Strength.Added", "Source@Power.Added * 0.5");
            
            // Entity A depends on B
            stat_accessor.register_dependency(entity_a, "Parent", entity_b);
            stat_accessor.add_modifier(entity_a, "Damage.Added", "Parent@Strength.Added * 2.0");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the dependency chain
        let [stats_c, stats_b, stats_a] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [entity_c, entity_b, entity_a])
            .unwrap();
        
        let c_power = stats_c.evaluate_by_string("Power.Added");
        let b_strength = stats_b.evaluate_by_string("Strength.Added");
        let a_damage = stats_a.evaluate_by_string("Damage.Added");
        
        assert_eq!(c_power, 10.0);
        assert_eq!(b_strength, 5.0);  // 10.0 * 0.5
        assert_eq!(a_damage, 10.0);   // 5.0 * 2.0
        
        // Now modify entity C and verify changes propagate through the chain
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(entity_c, "Power.Added", 10.0); // Increase by 10
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify updated values
        let [stats_c, stats_b, stats_a] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [entity_c, entity_b, entity_a])
            .unwrap();
        
        let updated_c_power = stats_c.evaluate_by_string("Power.Added");
        let updated_b_strength = stats_b.evaluate_by_string("Strength.Added");
        let updated_a_damage = stats_a.evaluate_by_string("Damage.Added");
        
        assert_eq!(updated_c_power, 20.0);      // 10.0 + 10.0
        assert_eq!(updated_b_strength, 10.0);   // 20.0 * 0.5
        assert_eq!(updated_a_damage, 20.0);     // 10.0 * 2.0
    }

    // Test multiple entity dependencies (entity depends on multiple other entities)
    #[test]
    fn test_multiple_entity_dependencies() {
        // Setup app
        let mut app = App::new();

        // Spawn three entities with Stats component
        let owner_entity = app.world_mut().spawn(Stats::new()).id();
        let weapon_entity = app.world_mut().spawn(Stats::new()).id();
        let minion_entity = app.world_mut().spawn(Stats::new()).id();

        // Setup dependencies and initial values
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Set base values for owner
            stat_accessor.add_modifier(owner_entity, "Intelligence.Added", 20.0);
            stat_accessor.add_modifier(owner_entity, "Strength.Added", 15.0);
            
            // Set base value for weapon
            stat_accessor.add_modifier(weapon_entity, "WeaponDamage.Added", 25.0);
            
            // Minion depends on both owner and weapon
            stat_accessor.register_dependency(minion_entity, "Owner", owner_entity);
            stat_accessor.register_dependency(minion_entity, "Weapon", weapon_entity);
            
            // Minion's damage depends on owner's intelligence and weapon's damage
            stat_accessor.add_modifier(minion_entity, "SpellDamage.Added", "Owner@Intelligence.Added * 0.5");
            stat_accessor.add_modifier(minion_entity, "PhysicalDamage.Added", "Weapon@WeaponDamage.Added * 0.8");
            
            // Minion's total damage depends on both types
            stat_accessor.add_modifier(minion_entity, "TotalDamage.Added", "SpellDamage.Added + PhysicalDamage.Added");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the dependencies
        let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        
        let spell_damage = stats_minion.evaluate_by_string("SpellDamage.Added");
        let physical_damage = stats_minion.evaluate_by_string("PhysicalDamage.Added");
        let total_damage = stats_minion.evaluate_by_string("TotalDamage.Added");
        
        assert_eq!(spell_damage, 10.0);     // Owner Intelligence 20.0 * 0.5
        assert_eq!(physical_damage, 20.0);  // Weapon Damage 25.0 * 0.8
        assert_eq!(total_damage, 30.0);     // 10.0 + 20.0
        
        // Now modify both dependencies and verify changes
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(owner_entity, "Intelligence.Added", 10.0);
            stat_accessor.add_modifier(weapon_entity, "WeaponDamage.Added", 15.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify updated values
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        
        let updated_spell = updated_stats.evaluate_by_string("SpellDamage.Added");
        let updated_physical = updated_stats.evaluate_by_string("PhysicalDamage.Added");
        let updated_total = updated_stats.evaluate_by_string("TotalDamage.Added");
        
        assert_eq!(updated_spell, 15.0);      // (20.0 + 10.0) * 0.5
        assert_eq!(updated_physical, 32.0);   // (25.0 + 15.0) * 0.8
        assert_eq!(updated_total, 47.0);      // 15.0 + 32.0
    }

    // Test complex expressions mixing entity dependencies and local dependencies
    #[test]
    fn test_mixed_entity_local_dependencies() {
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component
        let owner_entity = app.world_mut().spawn(Stats::new()).id();
        let minion_entity = app.world_mut().spawn(Stats::new()).id();

        // Setup dependencies and initial values
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Set base values for owner
            stat_accessor.add_modifier(owner_entity, "Power.Added", 20.0);
            
            // Set local multiplier for minion
            stat_accessor.add_modifier(minion_entity, "Multiplier.Added", 2.5);
            
            // Register dependencies
            stat_accessor.register_dependency(minion_entity, "Owner", owner_entity);
            
            // Create a mixed dependency expression
            stat_accessor.add_modifier(minion_entity, "Damage.Added", "Owner@Power.Added * Multiplier.Added");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the mixed dependency calculation
        let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let damage = stats_minion.evaluate_by_string("Damage.Added");
        
        assert_eq!(damage, 50.0);  // Owner Power 20.0 * Local Multiplier 2.5
        
        // Test updating the local multiplier
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(minion_entity, "Multiplier.Added", 0.5);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify only the multiplier changed, not the owner stat
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let updated_damage = updated_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(updated_damage, 60.0);  // Owner Power 20.0 * Local Multiplier (2.5 + 0.5 = 3.0)
        
        // Test updating the owner stat
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(owner_entity, "Power.Added", 10.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify the owner stat change propagated correctly
        let final_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let final_damage = final_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(final_damage, 90.0);  // Owner Power (20.0 + 10.0 = 30.0) * Local Multiplier 3.0
    }

    // Test entity dependency removal
    #[test]
    fn test_entity_dependency_removal() {
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component
        let owner_entity = app.world_mut().spawn(Stats::new()).id();
        let minion_entity = app.world_mut().spawn(Stats::new()).id();

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Set base values for owner
            stat_accessor.add_modifier(owner_entity, "Power.Added", 20.0);
            
            // Register dependencies
            stat_accessor.register_dependency(minion_entity, "Owner", owner_entity);
            
            // Create a dependency
            stat_accessor.add_modifier(minion_entity, "Damage.Added", "Owner@Power.Added * 1.5");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify initial dependency
        let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let initial_damage = stats_minion.evaluate_by_string("Damage.Added");
        
        assert_eq!(initial_damage, 30.0);  // Owner Power 20.0 * 1.5
        
        // Remove the entity-dependent modifier
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Remove the modifier that depends on the owner
            stat_accessor.remove_modifier(minion_entity, "Damage.Added", "Owner@Power.Added * 1.5");
            
            // Add a fixed value instead
            stat_accessor.add_modifier(minion_entity, "Damage.Added", 15.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify dependency is removed and fixed value works
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let updated_damage = updated_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(updated_damage, 15.0);  // Fixed value, no longer depends on owner
        
        // Modify the owner entity and verify it no longer affects the minion
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(owner_entity, "Power.Added", 30.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify the minion's damage didn't change
        let final_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let final_damage = final_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(final_damage, 15.0);  // Still fixed value, owner change had no effect
    }

    // Test complex tag-based entity dependencies
    #[test]
    fn test_complex_tag_based_entity_dependencies() {
        // Define bit flags for damage types
        const FIRE: u32 = 0x01;
        const COLD: u32 = 0x02;
        const LIGHTNING: u32 = 0x04;
        
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component
        let master_entity = app.world_mut().spawn(Stats::new()).id();
        let servant_entity = app.world_mut().spawn(Stats::new()).id();

        println!("Master entity: {}", master_entity);
        println!("Servant entity: {}", servant_entity);

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Set base elemental damage values for master
            stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", FIRE), 20.0);
            stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", COLD), 15.0);
            stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", LIGHTNING), 25.0);
            
            // Set elemental multipliers for master
            stat_accessor.add_modifier(master_entity, &format!("Damage.Increased.{}", FIRE), 0.5);
            stat_accessor.add_modifier(master_entity, &format!("Damage.Increased.{}", COLD), 0.3);
            stat_accessor.add_modifier(master_entity, &format!("Damage.Increased.{}", LIGHTNING), 0.4);
            
            // Register dependency
            stat_accessor.register_dependency(servant_entity, "Master", master_entity);
            
            // Create complex tag-based dependencies on the servant
            stat_accessor.add_modifier(servant_entity, &format!("Damage.Added.{}", FIRE), format!("Master@Damage.Added.{} * 0.6", FIRE));
            stat_accessor.add_modifier(servant_entity, &format!("Damage.Added.{}", COLD), format!("Master@Damage.Added.{} * 0.7", COLD));
            stat_accessor.add_modifier(servant_entity, &format!("Damage.Added.{}", LIGHTNING), format!("Master@Damage.Added.{} * 0.5", LIGHTNING));
            
            // Copy master's multipliers (simplified syntax)
            stat_accessor.add_modifier(servant_entity, &format!("Damage.Increased.{}", FIRE), format!("Master@Damage.Increased.{}", FIRE));
            stat_accessor.add_modifier(servant_entity, &format!("Damage.Increased.{}", COLD), format!("Master@Damage.Increased.{}", COLD));
            stat_accessor.add_modifier(servant_entity, &format!("Damage.Increased.{}", LIGHTNING), format!("Master@Damage.Increased.{}", LIGHTNING));
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the complex tag-based dependencies
        let stats_servant = app.world_mut().query::<&Stats>().get(app.world(), servant_entity).unwrap();
        
        // Calculate expected values
        // For each damage type: servant's damage = master's base * servant scaling * (1 + master's increased)
        let fire_expected = 20.0 * 0.6 * (1.0 + 0.5);
        let cold_expected = 15.0 * 0.7 * (1.0 + 0.3);
        let lightning_expected = 25.0 * 0.5 * (1.0 + 0.4);
        
        let fire_actual = stats_servant.evaluate_by_string(&format!("Damage.{}", FIRE));
        let cold_actual = stats_servant.evaluate_by_string(&format!("Damage.{}", COLD));
        let lightning_actual = stats_servant.evaluate_by_string(&format!("Damage.{}", LIGHTNING));
        
        assert_approx_eq(fire_actual, fire_expected);
        assert_approx_eq(cold_actual, cold_expected);
        assert_approx_eq(lightning_actual, lightning_expected);
        
        // Now increase the master's fire damage and verify the change propagates
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", FIRE), 10.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify updated values
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), servant_entity).unwrap();
        
        // Calculate new expected values - only fire changes
        let updated_fire_expected = 30.0 * 0.6 * (1.0 + 0.5);
        
        let updated_fire = updated_stats.evaluate_by_string(&format!("Damage.{}", FIRE));
        assert_approx_eq(updated_fire, updated_fire_expected);
    }

    // Test concurrent updates to multiple entity stats
    #[test]
    fn test_concurrent_entity_updates() {
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component - a buff source and multiple recipients
        let buff_source = app.world_mut().spawn(Stats::new()).id();
        let recipient_a = app.world_mut().spawn(Stats::new()).id();
        let recipient_b = app.world_mut().spawn(Stats::new()).id();
        let recipient_c = app.world_mut().spawn(Stats::new()).id();

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Set base buff value
            stat_accessor.add_modifier(buff_source, "AuraPower.Added", 10.0);
            
            // Register dependencies for all recipients
            stat_accessor.register_dependency(recipient_a, "Aura", buff_source);
            stat_accessor.register_dependency(recipient_b, "Aura", buff_source);
            stat_accessor.register_dependency(recipient_c, "Aura", buff_source);
            
            // Each recipient gets the aura buff with a different multiplier
            stat_accessor.add_modifier(recipient_a, "BuffedPower.Added", "Aura@AuraPower.Added * 1.2");
            stat_accessor.add_modifier(recipient_b, "BuffedPower.Added", "Aura@AuraPower.Added * 0.8");
            stat_accessor.add_modifier(recipient_c, "BuffedPower.Added", "Aura@AuraPower.Added * 1.5");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify initial buffed values
        let [stats_a, stats_b, stats_c] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [recipient_a, recipient_b, recipient_c])
            .unwrap();
        
        let power_a = stats_a.evaluate_by_string("BuffedPower.Added");
        let power_b = stats_b.evaluate_by_string("BuffedPower.Added");
        let power_c = stats_c.evaluate_by_string("BuffedPower.Added");
        
        assert_eq!(power_a, 12.0);  // 10.0 * 1.2
        assert_eq!(power_b, 8.0);   // 10.0 * 0.8
        assert_eq!(power_c, 15.0);  // 10.0 * 1.5
        
        // Now change the aura power value to simulate a buff strengthening
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
            // Strengthen the aura
            stat_accessor.add_modifier(buff_source, "AuraPower.Added", 5.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify all buffs updated correctly
        let [updated_a, updated_b, updated_c] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [recipient_a, recipient_b, recipient_c])
            .unwrap();
        
        let updated_power_a = updated_a.evaluate_by_string("BuffedPower.Added");
        let updated_power_b = updated_b.evaluate_by_string("BuffedPower.Added");
        let updated_power_c = updated_c.evaluate_by_string("BuffedPower.Added");
        
        assert_eq!(updated_power_a, 18.0);  // 15.0 * 1.2
        assert_eq!(updated_power_b, 12.0);  // 15.0 * 0.8
        assert_eq!(updated_power_c, 22.5);  // 15.0 * 1.5
    }
}