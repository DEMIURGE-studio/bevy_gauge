use bevy::prelude::*;
use bevy_utils::HashMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bevy_guage::prelude::{
    StatDefinitions, StatContext, StatContextRefs, Stats,
};

/// Builds the ephemeral context for an entity, using QueryState lookups
pub fn build<'a>(
    entity: Entity,
    world: &'a World,
    defs_query: &QueryState<&StatDefinitions>,
    ctx_query: &QueryState<&StatContext>,
) -> StatContextRefs<'a> {
    let mut map = HashMap::default();

    if let Ok(defs) = defs_query.get_manual(world, entity) {
        map.insert("self", StatContextRefs::Definitions(defs));
    }

    if let Ok(stat_context) = ctx_query.get_manual(world, entity) {
        for (key, child_entity) in &stat_context.sources {
            if key == "self" {
                continue;
            }
            let child_src = build(*child_entity, world, defs_query, ctx_query);
            map.insert(key.as_str(), child_src);
        }
    }

    StatContextRefs::SubContext(map)
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

    // (4) Insert Stats
    world.entity_mut(e0).insert(Stats::default());
    world.entity_mut(e1).insert(Stats::default());
    world.entity_mut(e2).insert(Stats::default());
    world.entity_mut(e3).insert(Stats::default());

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

    // Insert Stats
    world.entity_mut(e0).insert(Stats::default());
    world.entity_mut(e1).insert(Stats::default());
    world.entity_mut(e2).insert(Stats::default());
    world.entity_mut(e3).insert(Stats::default());

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

/// A simple benchmark for reading 100 stats from the `Stats` component
fn bench_stats_lookups(c: &mut Criterion) {
    let mut stats = Stats::default();

    // 1) Populate Stats with 100 keys
    for i in 0..10000 {
        let key = format!("StatKey{}", i);
        stats.0.insert(key, i as f32);
    }

    // 2) Benchmark 100 lookups
    let mut group = c.benchmark_group("stats_lookups");
    group.bench_function("10,000 stats lookups", |b| {
        b.iter(|| {
            for i in 0..10000 {
                let key = format!("StatKey{}", i);
                let val = stats.get(&key).unwrap(); 
                black_box(val);
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

// Group all benchmarks together
criterion_group!(
    benches,
    bench_deep_hierarchy_evaluation,
    bench_deep_hierarchy_build,
    bench_stats_lookups,
    bench_ecs_value_iteration,
);
criterion_main!(benches);
