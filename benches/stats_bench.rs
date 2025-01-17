use bevy::prelude::*;
use bevy_utils::HashMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bevy_guage::prelude::{
    Expression, HardMap, StatContext, StatContextRefs, StatContextType, StatDefinitions
};

pub fn build<'a>(
    entity: Entity,
    world: &'a World,
    defs_query: &QueryState<&StatDefinitions>,
    ctx_query: &QueryState<&StatContext>,
) -> StatContextRefs<'a> {
    // Create a HardMap with default NoContext in each slot
    let mut hard_map = HardMap::new();

    // If the entity itself has definitions, store them under the "This" slot
    if let Ok(defs) = defs_query.get_manual(world, entity) {
        hard_map.set(StatContextType::This, StatContextRefs::Definitions(defs));
    }

    // If the entity has a StatContext, build subcontexts for each known key
    if let Ok(stat_context) = ctx_query.get_manual(world, entity) {
        for (key, child_entity) in &stat_context.sources {
            // Avoid infinite recursion if an entity references itself
            if *child_entity == entity {
                continue;
            }
            // Recursively build the child subcontext
            let child_src = build(*child_entity, world, defs_query, ctx_query);

            // Match the child key to one of our 3 slots
            match key.as_str() {
                "self"   => hard_map.set(StatContextType::This, child_src),
                "parent" => hard_map.set(StatContextType::Parent, child_src),
                "target" => hard_map.set(StatContextType::Target, child_src),
                // If you have more “hard-coded” slots, handle them here
                _ => {
                    // Or ignore unknown keys
                }
            }
        }
    }

    // Return a SubContext if we stored anything
    StatContextRefs::SubContext(Box::new(hard_map))
}

/// A benchmark that tests a deep hierarchy of parent contexts, and **evaluates** `ChildLife` on E3 10,000 times.
fn bench_deep_hierarchy_evaluation(c: &mut Criterion) {
    let mut world = World::default();

    // (1) Spawn & setup: E0 -> E1 -> E2 -> E3
    let e0 = world.spawn_empty().id();
    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();
    let e3 = world.spawn_empty().id();

    // (2) Insert definitions
    {
        let mut defs = StatDefinitions::new();
        defs.add("AddedLife", 100).unwrap();
        defs.add("IncreasedLife", 50).unwrap();
        defs.set("TotalLife", bevy_guage::prelude::ExpressionPart::new(
            1, 
            "+= AddedLife * IncreasedLife / 100.0"
        ));
        world.entity_mut(e0).insert(defs);
    }
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLifeB", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.TotalLife + 20",
        ));
        world.entity_mut(e1).insert(defs);
    }
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLifeA", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.ChildLifeB + 20",
        ));
        world.entity_mut(e2).insert(defs);
    }
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLife", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.ChildLifeA + 20",
        ));
        world.entity_mut(e3).insert(defs);
    }

    // (3) Insert StatContext
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e0);
        world.entity_mut(e0).insert(ctx);
    }
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e1);
        ctx.insert("parent", e0);
        world.entity_mut(e1).insert(ctx);
    }
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e2);
        ctx.insert("parent", e1);
        world.entity_mut(e2).insert(ctx);
    }
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e3);
        ctx.insert("parent", e2);
        world.entity_mut(e3).insert(ctx);
    }

    // (5) Build QueryStates & retrieve definitions
    let mut defs_query = world.query::<&StatDefinitions>();
    let mut ctx_query  = world.query::<&StatContext>();
    let e3_defs = defs_query.get(&world, e3).unwrap();

    // Build the ephemeral context for E3 once
    let ctx_refs = build(e3, &world, &defs_query, &ctx_query);

    // Evaluate "ChildLife" 10,000 times
    let mut group = c.benchmark_group("deep_hierarchy_eval");
    group.bench_function("evaluate E3.ChildLife x10000", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let val = e3_defs.get("ChildLife", &ctx_refs).unwrap();
                black_box(val);
            }
        });
    });
    group.finish();
}

/// A benchmark that tests the *cost of building* the ephemeral context 10,000 times
/// for the same deep hierarchy.
fn bench_deep_hierarchy_build(c: &mut Criterion) {
    let mut world = World::default();

    // Same 4-entity chain
    let e0 = world.spawn_empty().id();
    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();
    let e3 = world.spawn_empty().id();

    // Insert definitions
    {
        let mut defs = StatDefinitions::new();
        defs.add("AddedLife", 100).unwrap();
        defs.add("IncreasedLife", 50).unwrap();
        defs.set("TotalLife", bevy_guage::prelude::ExpressionPart::new(
            1, 
            "+= AddedLife * IncreasedLife / 100.0"
        ));
        world.entity_mut(e0).insert(defs);
    }
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLifeB", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.TotalLife + 20",
        ));
        world.entity_mut(e1).insert(defs);
    }
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLifeA", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.ChildLifeB + 20",
        ));
        world.entity_mut(e2).insert(defs);
    }
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLife", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.ChildLifeA + 20",
        ));
        world.entity_mut(e3).insert(defs);
    }

    // Insert StatContext
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e0);
        world.entity_mut(e0).insert(ctx);
    }
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e1);
        ctx.insert("parent", e0);
        world.entity_mut(e1).insert(ctx);
    }
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e2);
        ctx.insert("parent", e1);
        world.entity_mut(e2).insert(ctx);
    }
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e3);
        ctx.insert("parent", e2);
        world.entity_mut(e3).insert(ctx);
    }

    let mut defs_query = world.query::<&StatDefinitions>();
    let mut ctx_query  = world.query::<&StatContext>();

    // Rebuild ephemeral context 10,000 times
    let mut group = c.benchmark_group("deep_hierarchy_build");
    group.bench_function("build E3 context x10,000", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let ctx_refs = build(e3, &world, &defs_query, &ctx_query);
                black_box(ctx_refs);
            }
        });
    });
    group.finish();
}
/// A simple component that holds a single f32 for ECS iteration test.
#[derive(Component)]
pub struct SimpleValue(pub f32);

/// A benchmark that spawns 10,000 entities, each with a simple f32 component,
/// and then iterates over them to measure the overhead of a basic ECS query.
fn bench_ecs_value_iteration(c: &mut Criterion) {
    let mut world = World::default();

    // 1) Spawn 10,000 entities with a SimpleValue
    for i in 0..10_000 {
        world.spawn(SimpleValue(i as f32));
    }

    // 2) Prepare a QueryState to iterate over &SimpleValue
    let mut query = world.query::<&SimpleValue>();

    // 3) Benchmark iterating over all 10,000
    let mut group = c.benchmark_group("ecs_value_iteration");
    group.bench_function("iterate 10k entities", |b| {
        b.iter(|| {
            for val in query.iter(&world) {
                // Just read the f32
                black_box(val.0);
            }
        });
    });
    group.finish();
}

/// A benchmark that tests a simple context, and **evaluates** `TotalLife` on E3 10,000 times.
fn bench_simple_evaluation(c: &mut Criterion) {
    let mut world = World::default();

    let e0 = world.spawn_empty().id();

    // (2) Insert definitions
    {
        let mut defs = StatDefinitions::new();
        defs.add("AddedLife", 100).unwrap();
        defs.add("IncreasedLife", 50).unwrap();
        defs.set("TotalLife", bevy_guage::prelude::ExpressionPart::new(
            1, 
            "+= AddedLife * IncreasedLife / 100.0"
        ));
        world.entity_mut(e0).insert(defs);
    }

    // (3) Insert StatContext
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e0);
        world.entity_mut(e0).insert(ctx);
    }

    // (5) Build QueryStates & retrieve definitions
    let mut defs_query = world.query::<&StatDefinitions>();
    let mut ctx_query  = world.query::<&StatContext>();
    let e0_defs = defs_query.get(&world, e0).unwrap();

    // Build the ephemeral context for E3 once
    let ctx_refs = build(e0, &world, &defs_query, &ctx_query);

    // Evaluate "ChildLife" 10,000 times
    let mut group = c.benchmark_group("deep_hierarchy_eval");
    group.bench_function("evaluate E0.TotalLife x10000", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let val = e0_defs.get("TotalLife", &ctx_refs).unwrap();
                black_box(val);
            }
        });
    });
    group.finish();
}

/// Benchmarks inserting 1,000 key/value pairs into a StatDefinitions,
/// using a pre-built array of keys (so the allocation for each key isn’t repeated).
fn bench_definitions_insertion(c: &mut Criterion) {
    // Pre-build a vector of 1,000 keys.
    let keys: Vec<String> = (0..1000)
        .map(|i| format!("Supercalefragilisticexpalidocious{}", i))
        .collect();
        
    let mut group = c.benchmark_group("stat_definitions_insertion");
    
    group.bench_function("insert 1000 keys", |b| {
        b.iter(|| {
            // Create a new StatDefinitions instance
            let mut defs = StatDefinitions::new();
            // Insert each key from our pre-built vector.
            for (i, key) in keys.iter().enumerate() {
                defs.set(key, Expression::from_float(i as f32));
            }
            black_box(defs);
        });
    });
    
    group.finish();
}

/// Benchmarks removing 1,000 key/value pairs from a StatDefinitions,
/// using a pre-built array of keys.
fn bench_definitions_removal(c: &mut Criterion) {
    // Pre-build a vector of 1,000 keys.
    let keys: Vec<String> = (0..1000)
        .map(|i| format!("Supercalefragilisticexpalidocious{}", i))
        .collect();
    
    let mut group = c.benchmark_group("stat_definitions_removal");
    
    group.bench_function("remove 1000 keys", |b| {
        b.iter(|| {
            // Create a new StatDefinitions and insert 1,000 key/value pairs.
            let mut defs = StatDefinitions::new();
            for (i, key) in keys.iter().enumerate() {
                defs.set(key, Expression::from_float(i as f32));
            }
            
            // Now, remove the key/value pairs.
            // (Here, we assume that your StatDefinitions internally wraps a HashMap
            // that is publicly accessible as `defs.0`. Adjust if needed.)
            for (i, key) in keys.iter().enumerate() {
                let _ = defs.subtract(key, Expression::from_float(i as f32));
            }
            
            black_box(defs);
        });
    });
    
    group.finish();
}

/// Group all benchmarks together.
criterion_group!(
    benches,
    bench_deep_hierarchy_evaluation,
    bench_deep_hierarchy_build,
    bench_ecs_value_iteration,
    bench_simple_evaluation,
    bench_definitions_insertion,
    bench_definitions_removal,
);
criterion_main!(benches);