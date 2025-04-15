use bevy::prelude::*;
use bevy_gauge::{plugin, prelude::*};

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

// Helper system to add a modifier to a stat
fn add_stat_modifier(
    mut stat_accessor: StatAccessor,
    query: Query<Entity, With<Stats>>,
) {
    for entity in &query {
        stat_accessor.add_modifier(entity, "Life.Added", 10.0);
    }
}

// Helper system to add a dependent modifier
fn add_dependent_modifier(
    mut stat_accessor: StatAccessor,
    query: Query<Entity, With<Stats>>,
) {
    for entity in &query {
        // Add a modifier that depends on Life.Added
        stat_accessor.add_modifier(entity, "Damage.Added", "Life.Added * 0.5");
    }
}

// Helper system to add an entity-dependent modifier
fn add_entity_dependency(
    mut stat_accessor: StatAccessor,
    query: Query<Entity, With<Stats>>,
) {
    if let Some(entity_iter) = query.iter().collect::<Vec<_>>().get(0..2) {
        let source = entity_iter[0];
        let target = entity_iter[1];
        
        // Register the dependency (source is known as "Source" to target)
        stat_accessor.register_source(target, "Source", source);
        
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
    let increase_life_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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
    let update_source_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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

// Test modifier removal
#[test]
fn test_modifier_removal() {
    // Setup app
    let mut app = App::new();

    // Spawn an entity with Stats component
    let entity = app.world_mut().spawn(Stats::new()).id();

    // Register and run the system to add a modifier
    let add_mod_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
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
    let remove_mod_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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
    let setup_chain_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
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
    let update_base_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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
    // Setup app
    let mut app = App::new();

    // Spawn an entity with Stats component
    let entity = app.world_mut().spawn(Stats::new()).id();

    // Register and run the system to add tagged damage stats
    let add_damage_stats_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
        for entity in &query {
            // Add base damage (universal)
            stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", u32::MAX), 10.0);
            
            // Add elemental damage with permissive tags
            stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", build_permissive_tag(FIRE)), 5.0);
            stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", build_permissive_tag(COLD)), 3.0);
            
            // Add weapon damage with permissive tags
            stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", build_permissive_tag(SWORD)), 2.0);
            
            // Add increased damage multipliers with permissive tags
            stat_accessor.add_modifier(entity, &format!("Damage.Increased.{}", build_permissive_tag(FIRE)), 0.2);
            stat_accessor.add_modifier(entity, &format!("Damage.Increased.{}", build_permissive_tag(SWORD)), 0.1);
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Set base buff value
        stat_accessor.add_modifier(buff_source, "AuraPower.Added", 10.0);
        
        // Register dependencies for all recipients
        stat_accessor.register_source(recipient_a, "Aura", buff_source);
        stat_accessor.register_source(recipient_b, "Aura", buff_source);
        stat_accessor.register_source(recipient_c, "Aura", buff_source);
        
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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

#[test]
fn test_entity_dependency_removal() {
    // Setup app
    let mut app = App::new();

    // Spawn entities with Stats component
    let owner_entity = app.world_mut().spawn(Stats::new()).id();
    let minion_entity = app.world_mut().spawn(Stats::new()).id();

    // Setup initial values and dependencies
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Set base values for owner
        stat_accessor.add_modifier(owner_entity, "Power.Added", 20.0);
        
        // Register dependencies
        stat_accessor.register_source(minion_entity, "Owner", owner_entity);
        
        // Create a dependency
        stat_accessor.add_modifier(minion_entity, "Damage.Added", "Owner@Power.Added * 1.5");
    });
    let _ = app.world_mut().run_system(system_id);

    // Verify initial dependency
    let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
    let initial_damage = stats_minion.evaluate_by_string("Damage.Added");
    
    assert_eq!(initial_damage, 30.0);  // Owner Power 20.0 * 1.5
    
    // Remove the entity-dependent modifier
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
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
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        stat_accessor.add_modifier(owner_entity, "Power.Added", 30.0);
    });
    let _ = app.world_mut().run_system(system_id);
    
    // Verify the minion's damage didn't change
    let final_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
    let final_damage = final_stats.evaluate_by_string("Damage.Added");
    
    assert_eq!(final_damage, 15.0);  // Still fixed value, owner change had no effect
}

#[test]
fn test_destroy_source_entity() {
    // Setup app
    let mut app = App::new();
    app.add_plugins(plugin);

    // Spawn entities with Stats component
    let source_entity = app.world_mut().spawn(Stats::new()).id();
    let target_entity = app.world_mut().spawn(Stats::new()).id();

    // Setup initial values and dependencies
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        // Set base values for owner
        stat_accessor.add_modifier(source_entity, "Power.Added", 20.0);
        
        // Register dependencies
        stat_accessor.register_source(target_entity, "Source", source_entity);
        
        // Create a dependency
        stat_accessor.add_modifier(target_entity, "Damage.Added", "(Source@Power.Added * 1.5) + 5");
    });
    let _ = app.world_mut().run_system(system_id);

    // Verify initial dependency
    let stats_target = app.world_mut().query::<&Stats>().get(app.world(), target_entity).unwrap();
    let initial_damage = stats_target.evaluate_by_string("Damage.Added");
    
    assert_eq!(initial_damage, 35.0);  // Source Power 20.0 * 1.5
    
    let system_id = app.world_mut().register_system(move |mut commands: Commands| {
        // Set base values for owner
        commands.entity(source_entity).despawn();
    });
    let _ = app.world_mut().run_system(system_id);

    let stats_target = app.world_mut().query::<&Stats>().get(app.world(), target_entity).unwrap();
    let modified_damage = stats_target.evaluate_by_string("Damage.Added");

    assert_eq!(modified_damage, 5.0);  // Source Power 0.0 * 1.5
}