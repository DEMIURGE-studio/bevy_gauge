use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_gauge::prelude::*;

// Helper function to create a basic config for testing
fn create_test_config()  {
    Konfig::reset_for_test(); // Ensure clean state
    // Configure for a damage stat with base/increased/more parts
    Konfig::register_stat_type("Damage", "Tagged");
    Konfig::register_total_expression("Damage", "base * (1.0 + increased) * more"); // Ensure 1.0 for float context
}

// New config for Modifiable "Power" stat
fn create_modifiable_power_config() {
    Konfig::reset_for_test(); // Ensure clean state
    Konfig::register_stat_type("Power", "Modifiable");
}

#[test]
fn test_basic_modifier_operations() {
    let mut app = App::new();
    create_test_config();
    app.add_plugins(MinimalPlugins); // Add minimal plugins for core Bevy systems

    let entity = app.world_mut().spawn(Stats::new()).id();

    // Add modifier using a one-shot system
    let add_mod_id = app.world_mut().register_system(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(
                entity,
                "Damage.increased.1", // 1 = fire tag
                50.0f32, // 50% increased fire damage
            );
        }
    );
    let _ = app.world_mut().run_system(add_mod_id).unwrap();

    // Test the query using a one-shot system
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(entity).unwrap();
            let value = stats_comp.get("Damage.increased.1");
            assert_eq!(value, 50.0);
        }
    );

    // Remove the modifier using a one-shot system
    let remove_mod_id = app.world_mut().register_system(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.remove_modifier(
                entity,
                "Damage.increased.1",
                50.0f32,
            );
        }
    );
    let _ = app.world_mut().run_system(remove_mod_id).unwrap();

    // Verify removal using a one-shot system
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(entity).unwrap();
            let value = stats_comp.get("Damage.increased.1");
            assert_eq!(value, 0.0); // Should be 0 after removal
        }
    );
}

#[test]
fn test_query_caching() {
    let mut app = App::new();
    create_test_config();
    app.add_plugins(MinimalPlugins);

    let entity = app.world_mut().spawn(Stats::new()).id();

    // Add modifiers using a one-shot system
    let add_mods_id = app.world_mut().register_system(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(entity, "Damage.increased.1", 50.0f32); // fire (tag=1)
            stats_mutator.add_modifier(entity, "Damage.increased.2", 30.0f32); // weapon (tag=2)
        }
    );
    let _ = app.world_mut().run_system(add_mods_id).unwrap();

    // Query for fire damage with weapon (tags 1 & 2 -> combined tag = 3)
    // Use a system to evaluate twice
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(entity).unwrap();
            let v1 = stats_comp.get("Damage.increased.3"); // 3 = fire(1) | weapon(2)
            let v2 = stats_comp.get("Damage.increased.3");
            assert_eq!(v1, v2, "Consecutive evaluations should yield the same result");
            assert_eq!(v1, 80.0, "Combined tagged modifiers should sum correctly");
        }
    );
}

#[test]
fn test_cache_invalidation() {
    let mut app = App::new();
    create_test_config();
    app.add_plugins(MinimalPlugins);

    let entity = app.world_mut().spawn(Stats::new()).id();

    // Initial setup
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(entity, "Damage.increased.1", 50.0f32);
        }
    );
    // Evaluate initial value and store it for comparison (this one needs to be captured)
    let initial_value_holder = app.world_mut().spawn_empty().id(); // Dummy entity to hold a component
    #[derive(Component)] struct TempValue(f32);
    app.world_mut().entity_mut(initial_value_holder).insert(TempValue(0.0));

    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>, mut q_temp: Query<&mut TempValue>| {
            if let Ok(mut temp_val) = q_temp.get_mut(initial_value_holder) {
                 let stats_comp = q_stats.get(entity).unwrap();
                 temp_val.0 = stats_comp.get("Damage.increased.3");
            }
        }
    );
    let initial_value = app.world().get::<TempValue>(initial_value_holder).unwrap().0;
    app.world_mut().despawn(initial_value_holder); // Clean up dummy entity


    // Add new modifier
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(entity, "Damage.increased.2", 30.0f32);
        }
    );
    
    // Evaluate after add and assert
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(entity).unwrap();
            let after_add_value = stats_comp.get("Damage.increased.3");
            assert_ne!(initial_value, after_add_value);
            assert_eq!(after_add_value, 80.0);
        }
    );
}

#[test]
fn test_source_dependency_updates() {
    let mut app = App::new();
    create_test_config();
    app.add_plugins(MinimalPlugins);
    
    let source = app.world_mut().spawn(Stats::new()).id();
    let target = app.world_mut().spawn(Stats::new()).id();

    // Register source relationship and add initial modifiers to source
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.register_source(target, "Source", source);
            stats_mutator.add_modifier(source, "Damage.increased.1", 50.0f32);
        }
    );

    // Add expression modifier to target, referencing source
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(
                target,
                "Damage.increased.1",
                "Damage.increased.1@Source",
            );
        }
    );

    // Test initial source value
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(source).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            assert_eq!(val, 50.0);
        }
    );
    
    // Test initial target value
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            assert_eq!(val, 50.0);
        }
    );

    // Modify source
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(source, "Damage.increased.1", 30.0f32);
        }
    );

    // Test updated source value
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(source).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            assert_eq!(val, 80.0);
        }
    );

    // Check that target was updated
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            assert_eq!(val, 80.0); // Failing assertion expected here if bug persists
        }
    );
}


#[test]
fn test_complex_dependency_chain() {
    let mut app = App::new();
    create_test_config();
    app.add_plugins(MinimalPlugins);
    
    let grandparent = app.world_mut().spawn(Stats::new()).id();
    let parent = app.world_mut().spawn(Stats::new()).id();
    let child = app.world_mut().spawn(Stats::new()).id();

    // Setup relationships and initial modifiers to grandparent
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.register_source(parent, "Parent", grandparent);
            stats_mutator.register_source(child, "Parent", parent);
            stats_mutator.add_modifier(grandparent, "Damage.increased.1", 50.0f32);
        }
    );

    // Add expression modifiers to parent and child
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(
                parent,
                "Damage.increased.1",
                "Damage.increased.1@Parent * 1.5",
            );
            stats_mutator.add_modifier(
                child,
                "Damage.increased.1",
                "Damage.increased.1@Parent * 2.0",
            );
        }
    );

    // Evaluate initial chain and assert
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let gp_stats = q_stats.get(grandparent).unwrap();
            let p_stats = q_stats.get(parent).unwrap();
            let c_stats = q_stats.get(child).unwrap();

            let gp_val = gp_stats.get("Damage.increased.1");
            assert_eq!(gp_val, 50.0);

            let p_val = p_stats.get("Damage.increased.1");
            assert_eq!(p_val, 75.0); // 50 * 1.5

            let c_val = c_stats.get("Damage.increased.1");
            assert_eq!(c_val, 150.0); // 75 * 2.0
        }
    );

    // Modify grandparent
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(grandparent, "Damage.increased.1", 50.0f32); // Adds to existing 50, total 100
        }
    );

    // Evaluate updated chain and assert
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let gp_stats = q_stats.get(grandparent).unwrap();
            let p_stats = q_stats.get(parent).unwrap();
            let c_stats = q_stats.get(child).unwrap();

            let new_gp_val = gp_stats.get("Damage.increased.1");
            assert_eq!(new_gp_val, 100.0);

            let new_parent_val = p_stats.get("Damage.increased.1");
            assert_eq!(new_parent_val, 150.0);

            let new_child_val = c_stats.get("Damage.increased.1");
            assert_eq!(new_child_val, 300.0);
        }
    );
}

#[test]
fn test_complex_dependency_chain_modifiable() {
    let mut app = App::new();
    create_modifiable_power_config();
    app.add_plugins(MinimalPlugins);
    
    let grandparent = app.world_mut().spawn(Stats::new()).id();
    let parent = app.world_mut().spawn(Stats::new()).id();
    let child = app.world_mut().spawn(Stats::new()).id();

    // Setup relationships and initial modifiers
    // System 1
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.register_source(parent, "Parent", grandparent);
            stats_mutator.register_source(child, "Parent", parent);
            // Add a base value to grandparent's "Power"
            stats_mutator.add_modifier(grandparent, "Power", 50.0f32); 
        }
    );

    // Add expression modifiers to parent and child
    // System 2
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            stats_mutator.add_modifier(
                parent,
                "Power", // Modifies the "Power" stat on parent
                Expression::new("Power@Parent + 10.0").unwrap() // Parent's Power = Grandparent's Power + 10
            );
            stats_mutator.add_modifier(
                child,
                "Power",  // Modifies the "Power" stat on child
                Expression::new("Power@Parent + 5.0").unwrap()  // Child's Power = Parent's Power + 5
            );
        }
    );

    // Evaluate initial chain and assert
    // System 3
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let gp_stats = q_stats.get(grandparent).unwrap();
            let p_stats = q_stats.get(parent).unwrap();
            let c_stats = q_stats.get(child).unwrap();

            let gp_val = gp_stats.get("Power");
            assert_eq!(gp_val, 50.0);

            let p_val = p_stats.get("Power");
            assert_eq!(p_val, 60.0); // 50 (from G) + 10

            let c_val = c_stats.get("Power");
            assert_eq!(c_val, 65.0); // 60 (from P) + 5
        }
    );

    // Modify grandparent
    // System 4
    let _ = app.world_mut().run_system_once(
        move |mut stats_mutator: StatsMutator| {
            // Add to existing 50, total 70 for grandparent's "Power" base
            stats_mutator.add_modifier(grandparent, "Power", 20.0f32); 
        }
    );

    // Evaluate updated chain and assert
    // System 5
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let gp_stats = q_stats.get(grandparent).unwrap();
            let p_stats = q_stats.get(parent).unwrap();
            let c_stats = q_stats.get(child).unwrap();

            let new_gp_val = gp_stats.get("Power");
            assert_eq!(new_gp_val, 70.0);

            let new_parent_val = p_stats.get("Power");
            assert_eq!(new_parent_val, 80.0); // 70 (from G) + 10

            let new_child_val = c_stats.get("Power");
            assert_eq!(new_child_val, 85.0); // 80 (from P) + 5
        }
    );
}

#[test]
fn test_add_modifier_then_register_source_tagged() {
    let mut app = App::new();
    create_test_config();
    app.add_plugins(MinimalPlugins);

    let target_entity = app.world_mut().spawn(Stats::new()).id();
    let source_entity = app.world_mut().spawn(Stats::new()).id();

    // System 1: Add modifier to source
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.add_modifier(source_entity, "Damage.increased.1", 100.0f32);
        }
    );

    // System 2: Add expression modifier to target, referencing a currently unknown source alias
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.add_modifier(
                target_entity,
                "Damage.increased.1", 
                Expression::new("Damage.increased.1@MySource").unwrap(),
            );
        }
    );

    // System 3: Evaluate target (source not registered yet)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            // Expression "Damage.increased.1@MySource" should eval to 0 as MySource provides 0
            assert_eq!(val, 0.0, "Target should be 0 before source registration"); 
        }
    );

    // System 4: Register the source
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.register_source(target_entity, "MySource", source_entity);
        }
    );

    // System 5: Evaluate target (source IS registered)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            assert_eq!(val, 100.0, "Target should be 100.0 after source registration");
        }
    );

    // System 6: Modify source stat
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.add_modifier(source_entity, "Damage.increased.1", 50.0f32); // Source is now 100 + 50 = 150
        }
    );

    // System 7: Evaluate target (should reflect source change)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            assert_eq!(val, 150.0, "Target should update to 150.0 after source modification");
        }
    );
}

#[test]
fn test_add_modifier_then_register_source_modifiable() {
    let mut app = App::new();
    create_modifiable_power_config();
    app.add_plugins(MinimalPlugins);

    let target_entity = app.world_mut().spawn(Stats::new()).id();
    let source_entity = app.world_mut().spawn(Stats::new()).id();

    // System 1: Add modifier to source
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.add_modifier(source_entity, "Power", 100.0f32);
        }
    );

    // System 2: Add expression modifier to target, referencing a currently unknown source alias
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.add_modifier(
                target_entity,
                "Power", 
                Expression::new("Power@MySource + 10.0").unwrap(),
            );
        }
    );

    // System 3: Evaluate target (source not registered yet)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Power");
            // Power@MySource = 0, so 0 + 10.0 = 10.0
            assert_eq!(val, 10.0, "Target should be 10.0 before source registration"); 
        }
    );

    // System 4: Register the source
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.register_source(target_entity, "MySource", source_entity);
        }
    );

    // System 5: Evaluate target (source IS registered)
    // Power@MySource = 100, so 100 + 10.0 = 110.0
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Power");
            assert_eq!(val, 110.0, "Target should be 110.0 after source registration");
        }
    );

    // System 6: Modify source stat
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.add_modifier(source_entity, "Power", 50.0f32); // Source Power is now 100 + 50 = 150
        }
    );

    // System 7: Evaluate target (should reflect source change)
    // Power@MySource = 150, so 150 + 10.0 = 160.0
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Power");
            assert_eq!(val, 160.0, "Target should update to 160.0 after source modification");
        }
    );
}

#[test]
fn test_source_despawn_updates_dependent_tagged() {
    let mut app = App::new();
    create_test_config();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy_gauge::plugin); // Ensure the main plugin is added for observer

    let source_entity = app.world_mut().spawn(Stats::new()).id();
    let target_entity = app.world_mut().spawn(Stats::new()).id();

    // System 1: Initial setup - Add modifier to source, register source, add dependent expression to target
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            // Add modifier to source
            sa.add_modifier(source_entity, "Damage.increased.1", 100.0f32);
            
            // Register source for target
            sa.register_source(target_entity, "MySource", source_entity);
            
            // Add expression modifier to target, referencing the source
            sa.add_modifier(
                target_entity,
                "Damage.increased.1", 
                Expression::new("Damage.increased.1@MySource").unwrap(),
            );
        }
    );

    // System 2: Evaluate target (source is live)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            assert_eq!(val, 100.0, "Target should be 100.0 before source despawn");
        }
    );

    // System 3: Despawn the source entity
    app.world_mut().despawn(source_entity);

    // System 4: Update the app to process despawn and trigger observers/systems
    // Run update twice: once for despawn/observer, once for stat propagation.
    app.update(); 
    app.update();

    // System 5: Evaluate target (source is despawned)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Damage.increased.1");
            // The expression "Damage.increased.1@MySource" should now evaluate with MySource contributing 0
            // because remove_stat_entity should have cleared the cached value for this source variable.
            assert_eq!(val, 0.0, "Target should be 0.0 after source despawn and update");
        }
    );
}

#[test]
fn test_source_despawn_updates_dependent_modifiable() {
    let mut app = App::new();
    create_modifiable_power_config();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy_gauge::plugin); // Ensure the main plugin is added for observer

    let source_entity = app.world_mut().spawn(Stats::new()).id();
    let target_entity = app.world_mut().spawn(Stats::new()).id();

    // System 1: Initial setup
    let _ = app.world_mut().run_system_once(
        move |mut sa: StatsMutator| {
            sa.add_modifier(source_entity, "Power", 75.0f32); // Source provides 75 Power
            sa.register_source(target_entity, "MyPowerSource", source_entity);
            sa.add_modifier(
                target_entity,
                "Power", 
                Expression::new("Power@MyPowerSource + 10.0").unwrap(), // Target = Source Power + 10
            );
        }
    );

    // System 2: Evaluate target (source is live)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Power");
            assert_eq!(val, 85.0, "Target should be 85.0 (75+10) before source despawn");
        }
    );
    
    // System 3: Despawn the source entity
    app.world_mut().despawn(source_entity);

    // System 4: Update the app
    app.update();
    app.update();

    // System 5: Evaluate target (source is despawned)
    let _ = app.world_mut().run_system_once(
        move |q_stats: Query<&Stats>| {
            let stats_comp = q_stats.get(target_entity).unwrap();
            let val = stats_comp.get("Power");
            // Power@MyPowerSource should be 0, so expression evaluates to 0.0 + 10.0
            assert_eq!(val, 10.0, "Target should be 10.0 after source despawn and update");
        }
    );
}