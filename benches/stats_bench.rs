use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::HashMap;
use evalexpr::build_operator_tree;
use bevy::prelude::*;

// Replace `bevy_gauge::stat_modifiers` with the actual module path.
use bevy_gauge::stat_modifiers::{StatDefinitions, ValueType, Expression};

/// Sets up a StatDefinitions instance with a single "Health" stat.
/// - Adds a literal modifier of 100.0 to "Health.Add"
/// - Adds several modifiers (including one from an expression)
/// - Sets the total expression of "Health" to return the "Add" value from the context.
fn setup_stat_definitions() -> StatDefinitions {
    let mut stats = StatDefinitions(HashMap::new());
    
    // Add a literal modifier to "Health.Add"
    stats.add_modifier("Health.Add", 100.0_f32);
    
    // Create a couple of modifiers.
    stats.add_modifier("Strength.Add", ValueType::from(100.0));
    stats.add_modifier("Health.Add", ValueType::from(100.0));
    stats.add_modifier("Health.Add", ValueType::from("Strength / 5"));
    
    // Set the total expression for Health to be the "Add" modifier.
    // This causes the evaluation to return the value of the "Add" key from the context.
    if let Some(stat) = stats.0.get_mut("Health") {
        stat.total = Expression(build_operator_tree("Add").unwrap());
    }
    if let Some(stat) = stats.0.get_mut("Strength") {
        stat.total = Expression(build_operator_tree("Add").unwrap());
    }
    
    stats
}

/// Benchmarks the evaluation of the "Health" stat without caching.
fn bench_evaluate(c: &mut Criterion) {
    let stats = setup_stat_definitions();
    c.bench_function("evaluate_health", |b| {
        b.iter(|| {
            let result = stats.evaluate("Health");
            black_box(result);
        });
    });
}

/// Benchmarks the evaluation of the "Health" stat using a cache.
fn bench_evaluate_cached(c: &mut Criterion) {
    let stats = setup_stat_definitions();
    let mut cache = HashMap::new();
    c.bench_function("evaluate_health_cached", |b| {
        b.iter(|| {
            cache.clear();
            let result = stats.evaluate_cached("Health", &mut cache);
            black_box(result);
        });
    });
}

/// A simple component for plain Bevy access.
#[derive(Component)]
struct SimpleComponent {
    value: f32,
}

/// Sets up a Bevy world with two entities:
/// 1. An entity with a SimpleComponent.
/// 2. An entity with a StatDefinitions component containing a "TestStat" stat.
fn setup_bevy_world() -> World {
    let mut world = World::new();
    
    // Spawn an entity with SimpleComponent.
    world.spawn(SimpleComponent { value: 42.0 });
    
    // Create a StatDefinitions instance with a "TestStat" stat.
    let mut stat_def = StatDefinitions(HashMap::new());
    stat_def.add_modifier("TestStat.Add", 200.0_f32);
    if let Some(stat) = stat_def.0.get_mut("TestStat") {
        stat.total = Expression(build_operator_tree("Add").unwrap());
    }
    // Spawn an entity with StatDefinitions.
    world.spawn(stat_def);
    
    world
}

/// Benchmarks accessing a plain component value 500 times via a Bevy query.
fn bench_plain_component(c: &mut Criterion) {
    let mut world = setup_bevy_world();
    let mut query = world.query::<&SimpleComponent>();
    c.bench_function("bevy_plain_component_get", |b| {
        b.iter(|| {
            // Simulate retrieving the component's value 500 times.
            for _ in 0..500 {
                for simple in query.iter(&world) {
                    black_box(simple.value);
                }
            }
        });
    });
}

/// Benchmarks evaluating a stat value from a StatDefinitions component 500 times via a Bevy query.
fn bench_stat_component_get(c: &mut Criterion) {
    let mut world = setup_bevy_world();
    let mut query = world.query::<&StatDefinitions>();
    c.bench_function("bevy_stat_component_get", |b| {
        b.iter(|| {
            // Simulate evaluating "TestStat" 500 times.
            for _ in 0..500 {
                for stat_def in query.iter(&world) {
                    let val = stat_def.evaluate("TestStat");
                    black_box(val);
                }
            }
        });
    });
}

criterion_group!(
    benches,
    bench_evaluate,
    bench_evaluate_cached,
    bench_plain_component,
    bench_stat_component_get
);
criterion_main!(benches);
