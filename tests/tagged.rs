use bevy::prelude::*;
use bevy_gauge::prelude::*;

// Helper function to create a basic config for testing
fn create_test_config() -> Config {
    let mut config = Config::default();
    // Configure for a damage stat with base/increased/more parts
    config.register_stat_type("Damage", "Tagged");
    config.register_total_expression("Damage", "base * (1 + increased) * more");
    Ok(config)
}

// Helper to create a basic StatAccessor for testing
fn setup_test_world() -> (World, StatAccessor) {
    let mut world = World::new();
    world.insert_resource(create_test_config());
    let stat_accessor = StatAccessor::from_world(&world);
    (world, stat_accessor)
}

#[test]
fn test_basic_modifier_operations() {
    let (mut world, mut stat_accessor) = setup_test_world();
    let entity = world.spawn(Stats::new()).id();

    // Add a fire damage modifier
    stat_accessor.add_modifier(
        entity,
        "Damage.increased.1", // 1 = fire tag
        50.0f32, // 50% increased fire damage
    );

    // Test the query
    let value = stat_accessor.evaluate(entity, "Damage.increased.1");
    assert_eq!(value, 50.0);

    // Remove the modifier
    stat_accessor.remove_modifier(
        entity,
        "Damage.increased.1",
        50.0f32,
    );

    // Verify removal
    let value = stat_accessor.evaluate(entity, "Damage.increased.1");
    assert_eq!(value, 0.0);
}

#[test]
fn test_query_caching() {
    let (mut world, mut stat_accessor) = setup_test_world();
    let entity = world.spawn(Stats::new()).id();

    // Add modifiers
    stat_accessor.add_modifier(entity, "Damage.increased.1", 50.0f32); // fire
    stat_accessor.add_modifier(entity, "Damage.increased.2", 30.0f32); // weapon

    // Query for fire damage with weapon (tags 1 & 2)
    let value1 = stat_accessor.evaluate(entity, "Damage.increased.3"); // 3 = fire(1) | weapon(2)
    let value2 = stat_accessor.evaluate(entity, "Damage.increased.3");

    // Values should be equal and sum of both modifiers
    assert_eq!(value1, value2);
    assert_eq!(value1, 80.0);

    // Verify cache was used (could check internal cache state if we expose it)
    let stats = world.entity(entity).get::<Stats>().unwrap();
    if let StatType::Tagged(tagged) = stats.definitions.get("Damage").unwrap() {
        assert!(tagged.query_cache.contains_key(&("increased".to_string(), 3)));
    }
}

#[test]
fn test_cache_invalidation() {
    let (mut world, mut stat_accessor) = setup_test_world();
    let entity = world.spawn(Stats::new()).id();

    // Initial setup
    stat_accessor.add_modifier(entity, "Damage.increased.1", 50.0f32);
    let initial = stat_accessor.evaluate(entity, "Damage.increased.3");

    // Add new modifier that should affect the cached query
    stat_accessor.add_modifier(entity, "Damage.increased.2", 30.0f32);
    let after_add = stat_accessor.evaluate(entity, "Damage.increased.3");

    // Cache should have been invalidated and new value computed
    assert_ne!(initial, after_add);
    assert_eq!(after_add, 80.0);
}

#[test]
fn test_source_dependency_updates() {
    let (mut world, mut stat_accessor) = setup_test_world();
    
    // Setup entities
    let source = world.spawn(Stats::new()).id();
    let target = world.spawn(Stats::new()).id();

    // Register source relationship
    stat_accessor.register_source(target, "Source", source);

    // Add modifier to source
    stat_accessor.add_modifier(source, "Damage.increased.1", 50.0f32);

    // Add expression that depends on source
    stat_accessor.add_modifier(
        target,
        "Damage.increased.1",
        "Source@Damage.increased.1", // Reference source's fire damage
    );

    // Test initial value
    let value = stat_accessor.evaluate(target, "Damage.increased.1");
    assert_eq!(value, 50.0);

    // Modify source
    stat_accessor.add_modifier(source, "Damage.increased.1", 30.0f32);

    // Check that target was updated
    let new_value = stat_accessor.evaluate(target, "Damage.increased.1");
    assert_eq!(new_value, 80.0);
}

#[test]
fn test_complex_dependency_chain() {
    let (mut world, mut stat_accessor) = setup_test_world();
    
    // Setup entities
    let grandparent = world.spawn(Stats::new()).id();
    let parent = world.spawn(Stats::new()).id();
    let child = world.spawn(Stats::new()).id();

    // Setup relationships
    stat_accessor.register_source(parent, "Parent", grandparent);
    stat_accessor.register_source(child, "Parent", parent);

    // Add base modifier to grandparent
    stat_accessor.add_modifier(grandparent, "Damage.increased.1", 50.0f32);

    // Parent depends on grandparent
    stat_accessor.add_modifier(
        parent,
        "Damage.increased.1",
        "Parent@Damage.increased.1 * 1.5", // 50% bonus to parent's value
    );

    // Child depends on parent
    stat_accessor.add_modifier(
        child,
        "Damage.increased.1",
        "Parent@Damage.increased.1 * 2.0", // Doubles parent's value
    );

    // Test the chain
    let gp_value = stat_accessor.evaluate(grandparent, "Damage.increased.1");
    let p_value = stat_accessor.evaluate(parent, "Damage.increased.1");
    let c_value = stat_accessor.evaluate(child, "Damage.increased.1");

    assert_eq!(gp_value, 50.0);
    assert_eq!(p_value, 75.0); // 50 * 1.5
    assert_eq!(c_value, 150.0); // 75 * 2.0

    // Modify grandparent and verify changes propagate
    stat_accessor.add_modifier(grandparent, "Damage.increased.1", 50.0f32);

    let new_gp_value = stat_accessor.evaluate(grandparent, "Damage.increased.1");
    let new_p_value = stat_accessor.evaluate(parent, "Damage.increased.1");
    let new_c_value = stat_accessor.evaluate(child, "Damage.increased.1");

    assert_eq!(new_gp_value, 100.0);
    assert_eq!(new_p_value, 150.0); // 100 * 1.5
    assert_eq!(new_c_value, 300.0); // 150 * 2.0
}