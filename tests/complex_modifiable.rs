use bevy::prelude::*;
use bevy_gauge::prelude::*;

stat_macros::define_tags! {
    damage_type {
        elemental { fire, cold, lightning },
        physical,
        chaos,
    },
    weapon_type {
        melee { sword, axe },
        ranged { bow, wand },
    },
}

// Helper function for approximate equality checks
fn assert_approx_eq(a: f32, b: f32) {
    assert!((a - b).abs() < f32::EPSILON * 100.0, "left: {}, right: {}", a, b);
}

// Test adding a meta tag after initial queries
#[test]
fn test_adding_meta_tag_after_query() {
    // Setup app
    let mut app = App::new();

    // Spawn an entity with Stats component
    let entity = app.world_mut().spawn(Stats::new()).id();

    // First, add fire damage and make a query for it
    let setup_fire_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Add fire damage with permissive tag
        stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", build_permissive_tag(FIRE)), 10.0);
        
        // Get the fire damage value to cache the query
        let fire_damage = stat_accessor.evaluate(entity, &format!("Damage.{}", FIRE));
        println!("Initial Fire Damage: {}", fire_damage);
    });
    let _ = app.world_mut().run_system(setup_fire_id);
    
    // Verify initial fire damage
    let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
    let fire_value = stats.evaluate_by_string(&format!("Damage.{}", FIRE));
    assert_eq!(fire_value, 10.0);
    
    // Now add an ELEMENTAL modifier that should affect FIRE queries
    let add_elemental_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Add elemental damage (which includes fire) with permissive tag
        stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", build_permissive_tag(ELEMENTAL)), 5.0);
        
        // Log the damage values
        let fire_damage = stat_accessor.evaluate(entity, &format!("Damage.{}", FIRE));
        let elemental_damage = stat_accessor.evaluate(entity, &format!("Damage.{}", COLD));
        println!("After adding ELEMENTAL: Fire={}, Cold={}", fire_damage, elemental_damage);
    });
    let _ = app.world_mut().run_system(add_elemental_id);
    
    // Check if the fire damage was properly updated to include elemental bonus
    let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
    let fire_value = updated_stats.evaluate_by_string(&format!("Damage.{}", FIRE|AXE));
    
    println!("Final fire value: {}", fire_value);
    assert_eq!(fire_value, 15.0);
}

// Test with complex tag-based entity dependencies
#[test]
fn test_complex_tag_based_entity_dependencies() {
    // Setup app
    let mut app = App::new();

    // Spawn entities with Stats component
    let master_entity = app.world_mut().spawn(Stats::new()).id();
    let servant_entity = app.world_mut().spawn(Stats::new()).id();

    println!("Master entity: {}", master_entity);
    println!("Servant entity: {}", servant_entity);

    // Setup initial values and dependencies
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Set base elemental damage values for master with permissive tags
        stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", build_permissive_tag(FIRE)), 20.0);
        stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", build_permissive_tag(COLD)), 15.0);
        stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", build_permissive_tag(LIGHTNING)), 25.0);
        
        // Set elemental multipliers for master with permissive tags
        stat_accessor.add_modifier(master_entity, &format!("Damage.Increased.{}", build_permissive_tag(FIRE)), 0.5);
        stat_accessor.add_modifier(master_entity, &format!("Damage.Increased.{}", build_permissive_tag(COLD)), 0.3);
        stat_accessor.add_modifier(master_entity, &format!("Damage.Increased.{}", build_permissive_tag(LIGHTNING)), 0.4);
        
        // Register dependency
        stat_accessor.register_source(servant_entity, "Master", master_entity);
        
        // Create complex tag-based dependencies on the servant with permissive tags
        stat_accessor.add_modifier(
            servant_entity, 
            &format!("Damage.Added.{}", build_permissive_tag(FIRE)), 
            format!("Master@Damage.Added.{} * 0.6", FIRE)
        );
        
        stat_accessor.add_modifier(
            servant_entity, 
            &format!("Damage.Added.{}", build_permissive_tag(COLD)), 
            format!("Master@Damage.Added.{} * 0.7", COLD)
        );
        
        stat_accessor.add_modifier(
            servant_entity, 
            &format!("Damage.Added.{}", build_permissive_tag(LIGHTNING)), 
            format!("Master@Damage.Added.{} * 0.5", LIGHTNING)
        );
        
        stat_accessor.add_modifier(
            servant_entity, 
            &format!("Damage.Increased.{}", build_permissive_tag(FIRE)), 
            format!("Master@Damage.Increased.{}", FIRE)
        );
        
        stat_accessor.add_modifier(
            servant_entity, 
            &format!("Damage.Increased.{}", build_permissive_tag(COLD)), 
            format!("Master@Damage.Increased.{}", COLD)
        );
        
        stat_accessor.add_modifier(
            servant_entity, 
            &format!("Damage.Increased.{}", build_permissive_tag(LIGHTNING)), 
            format!("Master@Damage.Increased.{}", LIGHTNING)
        );
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Increase fire damage on master with permissive tag
        stat_accessor.add_modifier(master_entity, &format!("Damage.Added.{}", build_permissive_tag(FIRE)), 10.0);
    });
    let _ = app.world_mut().run_system(system_id);
    
    // Verify updated values
    let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), servant_entity).unwrap();
    
    // Calculate new expected values - only fire changes
    let updated_fire_expected = 30.0 * 0.6 * (1.0 + 0.5);
    
    let updated_fire = updated_stats.evaluate_by_string(&format!("Damage.{}", FIRE));
    assert_approx_eq(updated_fire, updated_fire_expected);
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Set base values for entity C
        stat_accessor.add_modifier(entity_c, "Power.Added", 10.0);
        
        // Entity B depends on C
        stat_accessor.register_source(entity_b, "Source", entity_c);
        stat_accessor.add_modifier(entity_b, "Strength.Added", "Source@Power.Added * 0.5");
        
        // Entity A depends on B
        stat_accessor.register_source(entity_a, "Parent", entity_b);
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Set base values for owner
        stat_accessor.add_modifier(owner_entity, "Intelligence.Added", 20.0);
        stat_accessor.add_modifier(owner_entity, "Strength.Added", 15.0);
        
        // Set base value for weapon
        stat_accessor.add_modifier(weapon_entity, "WeaponDamage.Added", 25.0);
        
        // Minion depends on both owner and weapon
        stat_accessor.register_source(minion_entity, "Owner", owner_entity);
        stat_accessor.register_source(minion_entity, "Weapon", weapon_entity);
        
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Set base values for owner
        stat_accessor.add_modifier(owner_entity, "Power.Added", 20.0);
        
        // Set local multiplier for minion
        stat_accessor.add_modifier(minion_entity, "Multiplier.Added", 2.5);
        
        // Register dependencies
        stat_accessor.register_source(minion_entity, "Owner", owner_entity);
        
        // Create a mixed dependency expression
        stat_accessor.add_modifier(minion_entity, "Damage.Added", "Owner@Power.Added * Multiplier.Added");
    });
    let _ = app.world_mut().run_system(system_id);

    // Verify the mixed dependency calculation
    let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
    let damage = stats_minion.evaluate_by_string("Damage.Added");
    
    assert_eq!(damage, 50.0);  // Owner Power 20.0 * Local Multiplier 2.5
    
    // Test updating the local multiplier
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        stat_accessor.add_modifier(minion_entity, "Multiplier.Added", 0.5);
    });
    let _ = app.world_mut().run_system(system_id);
    
    // Verify only the multiplier changed, not the owner stat
    let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
    let updated_damage = updated_stats.evaluate_by_string("Damage.Added");
    
    assert_eq!(updated_damage, 60.0);  // Owner Power 20.0 * Local Multiplier (2.5 + 0.5 = 3.0)
    
    // Test updating the owner stat
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        stat_accessor.add_modifier(owner_entity, "Power.Added", 10.0);
    });
    let _ = app.world_mut().run_system(system_id);
    
    // Verify the owner stat change propagated correctly
    let final_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
    let final_damage = final_stats.evaluate_by_string("Damage.Added");
    
    assert_eq!(final_damage, 90.0);  // Owner Power (20.0 + 10.0 = 30.0) * Local Multiplier 3.0
}