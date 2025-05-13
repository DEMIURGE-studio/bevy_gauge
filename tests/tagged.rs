use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_gauge::prelude::*;

// Helper function to create a basic config for testing
fn create_test_config() -> Config {
    let mut config = Config::default();
    // Configure for a damage stat with base/increased/more parts
    config.register_stat_type("Damage", "Tagged");
    config.register_total_expression("Damage", "base * (1 + increased) * more");
    // For Modifiable test
    // config.register_stat_type("Power", "Modifiable"); // This will be in its own config
    // config.register_relationship_type("Power", ModType::Add); // Default for Modifiable is Add if not specified
    config
}

// New config for Modifiable "Power" stat
fn create_modifiable_power_config() -> Config {
    let mut config = Config::default();
    config.register_stat_type("Power", "Modifiable");
    // Modifiable::new will use ModType::Add by default if not found for "Power" or "Power.base" etc.
    // Or, we can explicitly set relationship for "Power" if needed.
    // For this test, the default ModType::Add for the Modifiable stat "Power" is fine.
    // When we add_modifier with a literal, it adds to Modifiable.base.
    // When we add_modifier with an Expression, it's added to Modifiable.mods and summed.
    config
}

#[test]
fn test_basic_modifier_operations() {
    let mut app = App::new();
    app.insert_resource(create_test_config());
    app.add_plugins(MinimalPlugins); // Add minimal plugins for core Bevy systems

    let entity = app.world_mut().spawn(Stats::new()).id();

    // Add modifier using a one-shot system
    let add_mod_id = app.world_mut().register_system(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(
                entity,
                "Damage.increased.1", // 1 = fire tag
                50.0f32, // 50% increased fire damage
            );
        }
    );
    let _ = app.world_mut().run_system(add_mod_id).unwrap();

    // Test the query using a one-shot system
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let value = stat_accessor.evaluate(entity, "Damage.increased.1");
            assert_eq!(value, 50.0);
        }
    );

    // Remove the modifier using a one-shot system
    let remove_mod_id = app.world_mut().register_system(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.remove_modifier(
                entity,
                "Damage.increased.1",
                50.0f32,
            );
        }
    );
    let _ = app.world_mut().run_system(remove_mod_id).unwrap();

    // Verify removal using a one-shot system
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let value = stat_accessor.evaluate(entity, "Damage.increased.1");
            assert_eq!(value, 0.0); // Should be 0 after removal
        }
    );
}

#[test]
fn test_query_caching() {
    let mut app = App::new();
    app.insert_resource(create_test_config());
    app.add_plugins(MinimalPlugins);

    let entity = app.world_mut().spawn(Stats::new()).id();

    // Add modifiers using a one-shot system
    let add_mods_id = app.world_mut().register_system(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(entity, "Damage.increased.1", 50.0f32); // fire (tag=1)
            stat_accessor.add_modifier(entity, "Damage.increased.2", 30.0f32); // weapon (tag=2)
        }
    );
    let _ = app.world_mut().run_system(add_mods_id).unwrap();

    // Query for fire damage with weapon (tags 1 & 2 -> combined tag = 3)
    // Use a system to evaluate twice
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let v1 = stat_accessor.evaluate(entity, "Damage.increased.3"); // 3 = fire(1) | weapon(2)
            let v2 = stat_accessor.evaluate(entity, "Damage.increased.3");
            assert_eq!(v1, v2, "Consecutive evaluations should yield the same result");
            assert_eq!(v1, 80.0, "Combined tagged modifiers should sum correctly");
        }
    );
}

#[test]
fn test_cache_invalidation() {
    let mut app = App::new();
    app.insert_resource(create_test_config());
    app.add_plugins(MinimalPlugins);

    let entity = app.world_mut().spawn(Stats::new()).id();

    // Initial setup
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(entity, "Damage.increased.1", 50.0f32);
        }
    );
    // Evaluate initial value and store it for comparison (this one needs to be captured)
    let mut initial_value_holder = app.world_mut().spawn_empty().id(); // Dummy entity to hold a component
    #[derive(Component)] struct TempValue(f32);
    app.world_mut().entity_mut(initial_value_holder).insert(TempValue(0.0));

    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor, mut q_temp: Query<&mut TempValue>| {
            if let Ok(mut temp_val) = q_temp.get_mut(initial_value_holder) {
                 temp_val.0 = stat_accessor.evaluate(entity, "Damage.increased.3");
            }
        }
    );
    let initial_value = app.world().get::<TempValue>(initial_value_holder).unwrap().0;
    app.world_mut().despawn(initial_value_holder); // Clean up dummy entity


    // Add new modifier
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(entity, "Damage.increased.2", 30.0f32);
        }
    );
    
    // Evaluate after add and assert
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let after_add_value = stat_accessor.evaluate(entity, "Damage.increased.3");
            assert_ne!(initial_value, after_add_value);
            assert_eq!(after_add_value, 80.0);
        }
    );
}

#[test]
fn test_source_dependency_updates() {
    let mut app = App::new();
    app.insert_resource(create_test_config());
    app.add_plugins(MinimalPlugins);
    
    let source = app.world_mut().spawn(Stats::new()).id();
    let target = app.world_mut().spawn(Stats::new()).id();

    // Register source relationship and add initial modifiers to source
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.register_source(target, "Source", source);
            stat_accessor.add_modifier(source, "Damage.increased.1", 50.0f32);
        }
    );

    // Add expression modifier to target, referencing source
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(
                target,
                "Damage.increased.1",
                "Damage.increased.1@Source",
            );
        }
    );

    // Test initial source value
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let val = stat_accessor.evaluate(source, "Damage.increased.1");
            assert_eq!(val, 50.0);
        }
    );
    
    // Test initial target value
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let val = stat_accessor.evaluate(target, "Damage.increased.1");
            assert_eq!(val, 50.0);
        }
    );

    // Modify source
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(source, "Damage.increased.1", 30.0f32);
        }
    );

    // Test updated source value
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let val = stat_accessor.evaluate(source, "Damage.increased.1");
            assert_eq!(val, 80.0);
        }
    );

    // Check that target was updated
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let val = stat_accessor.evaluate(target, "Damage.increased.1");
            assert_eq!(val, 80.0); // Failing assertion expected here if bug persists
        }
    );
}


#[test]
fn test_complex_dependency_chain() {
    let mut app = App::new();
    app.insert_resource(create_test_config());
    app.add_plugins(MinimalPlugins);
    
    let grandparent = app.world_mut().spawn(Stats::new()).id();
    let parent = app.world_mut().spawn(Stats::new()).id();
    let child = app.world_mut().spawn(Stats::new()).id();

    // Setup relationships and initial modifiers to grandparent
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.register_source(parent, "Parent", grandparent);
            stat_accessor.register_source(child, "Parent", parent);
            stat_accessor.add_modifier(grandparent, "Damage.increased.1", 50.0f32);
        }
    );

    // Add expression modifiers to parent and child
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(
                parent,
                "Damage.increased.1",
                "Damage.increased.1@Parent * 1.5",
            );
            stat_accessor.add_modifier(
                child,
                "Damage.increased.1",
                "Damage.increased.1@Parent * 2.0",
            );
        }
    );

    // Evaluate initial chain and assert
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let gp_val = stat_accessor.evaluate(grandparent, "Damage.increased.1");
            assert_eq!(gp_val, 50.0);

            let p_val = stat_accessor.evaluate(parent, "Damage.increased.1");
            assert_eq!(p_val, 75.0); // 50 * 1.5

            let c_val = stat_accessor.evaluate(child, "Damage.increased.1");
            assert_eq!(c_val, 150.0); // 75 * 2.0
        }
    );

    // Modify grandparent
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(grandparent, "Damage.increased.1", 50.0f32); // Adds to existing 50, total 100
        }
    );

    // Evaluate updated chain and assert
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let new_gp_val = stat_accessor.evaluate(grandparent, "Damage.increased.1");
            assert_eq!(new_gp_val, 100.0);

            let new_parent_val = stat_accessor.evaluate(parent, "Damage.increased.1");
            assert_eq!(new_parent_val, 150.0);

            let new_child_val = stat_accessor.evaluate(child, "Damage.increased.1");
            assert_eq!(new_child_val, 300.0);
        }
    );
}

#[test]
fn test_complex_dependency_chain_modifiable() {
    let mut app = App::new();
    app.insert_resource(create_modifiable_power_config());
    app.add_plugins(MinimalPlugins);
    
    let grandparent = app.world_mut().spawn(Stats::new()).id();
    let parent = app.world_mut().spawn(Stats::new()).id();
    let child = app.world_mut().spawn(Stats::new()).id();

    // Setup relationships and initial modifiers
    // System 1
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.register_source(parent, "Parent", grandparent);
            stat_accessor.register_source(child, "Parent", parent);
            // Add a base value to grandparent's "Power"
            stat_accessor.add_modifier(grandparent, "Power", 50.0f32); 
        }
    );

    // Add expression modifiers to parent and child
    // System 2
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(
                parent,
                "Power", // Modifies the "Power" stat on parent
                Expression::new("Power@Parent + 10.0").unwrap() // Parent's Power = Grandparent's Power + 10
            );
            stat_accessor.add_modifier(
                child,
                "Power",  // Modifies the "Power" stat on child
                Expression::new("Power@Parent + 5.0").unwrap()  // Child's Power = Parent's Power + 5
            );
        }
    );

    // Evaluate initial chain and assert
    // System 3
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let gp_val = stat_accessor.evaluate(grandparent, "Power");
            assert_eq!(gp_val, 50.0);

            let p_val = stat_accessor.evaluate(parent, "Power");
            assert_eq!(p_val, 60.0); // 50 (from G) + 10

            let c_val = stat_accessor.evaluate(child, "Power");
            assert_eq!(c_val, 65.0); // 60 (from P) + 5
        }
    );

    // Modify grandparent
    // System 4
    let _ = app.world_mut().run_system_once(
        move |mut stat_accessor: StatAccessor| {
            // Add to existing 50, total 70 for grandparent's "Power" base
            stat_accessor.add_modifier(grandparent, "Power", 20.0f32); 
        }
    );

    // Evaluate updated chain and assert
    // System 5
    let _ = app.world_mut().run_system_once(
        move |stat_accessor: StatAccessor| {
            let new_gp_val = stat_accessor.evaluate(grandparent, "Power");
            assert_eq!(new_gp_val, 70.0);

            let new_parent_val = stat_accessor.evaluate(parent, "Power");
            assert_eq!(new_parent_val, 80.0); // 70 (from G) + 10

            let new_child_val = stat_accessor.evaluate(child, "Power");
            assert_eq!(new_child_val, 85.0); // 80 (from P) + 5
        }
    );
}