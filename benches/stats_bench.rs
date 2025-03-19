use bevy::prelude::*;
use bevy_utils::HashMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Your new simplified stats + expression system:
use bevy_gauge::prelude::{
    Stats, StatType, StatContext, StatContextRefs, 
    // If needed:
    Expression
};

pub fn build<'a>(
    entity: Entity,
    world: &'a World,
    defs_query: &QueryState<&Stats>,
    ctx_query: &QueryState<&StatContext>,
) -> StatContextRefs<'a> {
    let mut context_map = HashMap::new();

    // If the entity itself has definitions, store them under the "This" slot
    if let Ok(defs) = defs_query.get_manual(world, entity) {
        context_map.insert("self", StatContextRefs::Definitions(defs));
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
            context_map.insert(key, child_src);
        }
    }

    // Return a SubContext if we stored anything
    StatContextRefs::SubContext(Box::new(context_map))
}

/// 1) A benchmark that tests a deep hierarchy of parent contexts, and
/// **evaluates** `ChildLife` on E3 10,000 times.
fn bench_deep_hierarchy_evaluation(c: &mut Criterion) {
    let mut world = World::default();

    // (1) Spawn & setup: E0 -> E1 -> E2 -> E3
    let e0 = world.spawn_empty().id();
    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();
    let e3 = world.spawn_empty().id();

    // (2) Insert Stats
    {
        let mut stats = Stats::new();
        stats.set("AddedLife", StatType::Literal(100.0));
        stats.set("IncreasedLife", StatType::Literal(50.0));
        // Expression: "Total = AddedLife * IncreasedLife / 100.0"
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = AddedLife * IncreasedLife / 100.0"
        ).unwrap());
        stats.set("TotalLife", StatType::Expression(expr));
        world.entity_mut(e0).insert(stats);
    }
    {
        let mut stats = Stats::new();
        // "Total = parent.TotalLife + 20"
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = parent.TotalLife + 20"
        ).unwrap());
        stats.set("ChildLifeB", StatType::Expression(expr));
        world.entity_mut(e1).insert(stats);
    }
    {
        let mut stats = Stats::new();
        // "Total = parent.ChildLifeB + 20"
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = parent.ChildLifeB + 20"
        ).unwrap());
        stats.set("ChildLifeA", StatType::Expression(expr));
        world.entity_mut(e2).insert(stats);
    }
    {
        let mut stats = Stats::new();
        // "Total = parent.ChildLifeA + 20"
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = parent.ChildLifeA + 20"
        ).unwrap());
        stats.set("ChildLife", StatType::Expression(expr));
        world.entity_mut(e3).insert(stats);
    }

    // (3) Insert StatContext for each
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

    // (4) Build QueryStates to retrieve data
    let mut stats_query = world.query::<&Stats>();
    let ctx_query = world.query::<&StatContext>();

    // Grab E3's stats for repeated evaluation
    let e3_stats = stats_query.get(&world, e3).unwrap();
    // Build ephemeral context once
    let ctx_refs = build(e3, &world, &stats_query, &ctx_query);

    // Evaluate "ChildLife" 10,000 times
    let mut group = c.benchmark_group("deep_hierarchy_eval");
    group.bench_function("evaluate E3.ChildLife x10000", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let val = e3_stats.get("ChildLife", &ctx_refs).unwrap();
                black_box(val);
            }
        });
    });
    group.finish();
}

/// 2) A benchmark that tests the *cost of building* the ephemeral context
/// 10,000 times for the same deep hierarchy.
fn bench_deep_hierarchy_build(c: &mut Criterion) {
    let mut world = World::default();

    // Reuse the same 4-entity chain
    let e0 = world.spawn_empty().id();
    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();
    let e3 = world.spawn_empty().id();

    // Insert stats (same as above)
    {
        let mut stats = Stats::new();
        stats.set("AddedLife", StatType::Literal(100.0));
        stats.set("IncreasedLife", StatType::Literal(50.0));
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = AddedLife * IncreasedLife / 100.0"
        ).unwrap());
        stats.set("TotalLife", StatType::Expression(expr));
        world.entity_mut(e0).insert(stats);
    }
    {
        let mut stats = Stats::new();
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = parent.TotalLife + 20"
        ).unwrap());
        stats.set("ChildLifeB", StatType::Expression(expr));
        world.entity_mut(e1).insert(stats);
    }
    {
        let mut stats = Stats::new();
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = parent.ChildLifeB + 20"
        ).unwrap());
        stats.set("ChildLifeA", StatType::Expression(expr));
        world.entity_mut(e2).insert(stats);
    }
    {
        let mut stats = Stats::new();
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = parent.ChildLifeA + 20"
        ).unwrap());
        stats.set("ChildLife", StatType::Expression(expr));
        world.entity_mut(e3).insert(stats);
    }

    // Insert StatContext (same pattern)
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

    let stats_query = world.query::<&Stats>();
    let ctx_query = world.query::<&StatContext>();

    // Benchmark building ephemeral context 10,000 times
    let mut group = c.benchmark_group("deep_hierarchy_build");
    group.bench_function("build E3 context x10,000", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let ctx_refs = build(e3, &world, &stats_query, &ctx_query);
                black_box(ctx_refs);
            }
        });
    });
    group.finish();
}

/// 3) A simple component that holds a single f32 for ECS iteration test.
#[derive(Component)]
pub struct SimpleValue(pub f32);

/// Benchmarks iterating over 10,000 entities, each with a `SimpleValue`.
fn bench_ecs_value_iteration(c: &mut Criterion) {
    let mut world = World::default();

    // Spawn 10,000 entities
    for i in 0..10_000 {
        world.spawn(SimpleValue(i as f32));
    }

    let mut query = world.query::<&SimpleValue>();

    let mut group = c.benchmark_group("ecs_value_iteration");
    group.bench_function("iterate 10k entities", |b| {
        b.iter(|| {
            for val in query.iter(&world) {
                black_box(val.0);
            }
        });
    });
    group.finish();
}

/// 4) A benchmark that tests a simple context, and **evaluates** `TotalLife` 10,000 times.
fn bench_simple_evaluation(c: &mut Criterion) {
    let mut world = World::default();

    let e0 = world.spawn_empty().id();

    {
        let mut stats = Stats::new();
        stats.set("AddedLife", StatType::Literal(100.0));
        stats.set("IncreasedLife", StatType::Literal(50.0));
        let expr = Expression(evalexpr::build_operator_tree(
            "Total = AddedLife + IncreasedLife"
        ).unwrap());
        stats.set("TotalLife", StatType::Expression(expr));
        world.entity_mut(e0).insert(stats);
    }

    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e0);
        world.entity_mut(e0).insert(ctx);
    }

    let mut stats_query = world.query::<&Stats>();
    let ctx_query = world.query::<&StatContext>();
    let e0_stats = stats_query.get(&world, e0).unwrap();
    let ctx_refs = build(e0, &world, &stats_query, &ctx_query);

    let mut group = c.benchmark_group("simple_eval");
    group.bench_function("evaluate E0.TotalLife x10000", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let val = e0_stats.get("TotalLife", &ctx_refs).unwrap();
                black_box(val);
            }
        });
    });
    group.finish();
}

/// 5) Benchmarks inserting 1,000 key/value pairs into `Stats`.
fn bench_definitions_insertion(c: &mut Criterion) {
    // Pre-build a vector of 1,000 keys
    let keys: Vec<String> = (0..1000)
        .map(|i| format!("Supercalefragilisticexpalidocious{}", i))
        .collect();

    let mut group = c.benchmark_group("stats_insertion");
    group.bench_function("insert 1000 keys", |b| {
        b.iter(|| {
            let mut stats = Stats::new();
            for (i, key) in keys.iter().enumerate() {
                stats.set(key, StatType::Literal(i as f32));
            }
            black_box(stats);
        });
    });
    group.finish();
}

/// 6) Benchmarks removing 1,000 key/value pairs from `Stats`.
fn bench_definitions_removal(c: &mut Criterion) {
    let keys: Vec<String> = (0..1000)
        .map(|i| format!("Supercalefragilisticexpalidocious{}", i))
        .collect();

    let mut group = c.benchmark_group("stats_removal");
    group.bench_function("remove 1000 keys", |b| {
        b.iter(|| {
            // Insert 1,000 key/value pairs
            let mut stats = Stats::new();
            for (i, key) in keys.iter().enumerate() {
                stats.set(key, StatType::Literal(i as f32));
            }
            // Now remove them
            for key in &keys {
                // Here we just call `remove()`. If you want to replicate the old
                // "subtract" logic, you can do it differently.
                let _ = stats.remove(key);
            }
            black_box(stats);
        });
    });
    group.finish();
}

/// 8) Benchmarks a "single-step" calculation with a single formula referencing multiple stats.
fn bench_single_step_calculation(c: &mut Criterion) {
    let mut world = World::default();
    let e0 = world.spawn_empty().id();

    {
        let mut stats = Stats::new();
        stats.set("Step1", StatType::Literal(10.0));
        stats.set("Step2", StatType::Literal(10.0));
        stats.set("Step3", StatType::Literal(10.0));
        stats.set("Step4", StatType::Literal(10.0));
        stats.set("Step5", StatType::Literal(10.0));
        stats.set("Step6", StatType::Literal(10.0));

        // Single-step formula: multiply everything in one shot:
        // "Total = 10 * self.Step1 * self.Step2 * self.Step3 * self.Step4 * self.Step5 * self.Step6"
        let expr_str = "Total = 10 * self.Step1 * self.Step2 * self.Step3 * self.Step4 * self.Step5 * self.Step6";
        let expr = Expression(evalexpr::build_operator_tree(expr_str).unwrap());
        stats.set("TotalVal", StatType::Expression(expr));

        world.entity_mut(e0).insert(stats);
    }

    {
        let mut ctx = StatContext::default();
        ctx.insert("self", e0);
        world.entity_mut(e0).insert(ctx);
    }

    let mut stats_query = world.query::<&Stats>();
    let ctx_query = world.query::<&StatContext>();
    let e0_stats = stats_query.get(&world, e0).unwrap();
    let ctx_refs = build(e0, &world, &stats_query, &ctx_query);

    let mut group = c.benchmark_group("single_step_eval");
    group.bench_function("calculate 10,000 single-step evals", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let val = e0_stats.get("TotalVal", &ctx_refs).unwrap();
                black_box(val);
            }
        });
    });
    group.finish();
}

fn compile_expressions(c: &mut Criterion) {
    let mut group = c.benchmark_group("stat_compilation");
    group.bench_function("Compile 1000 expressions", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let expr_str = "Total = 10 * self.Step1 * self.Step2 * self.Step3 * self.Step4 * self.Step5 * self.Step6";
                let _ = Expression(evalexpr::build_operator_tree(expr_str).unwrap());
            }
        });
    });
    group.finish();
}

// Finally, group all benchmarks.
criterion_group!(
    benches,
    bench_deep_hierarchy_evaluation,
    bench_deep_hierarchy_build,
    bench_ecs_value_iteration,
    bench_simple_evaluation,
    bench_definitions_insertion,
    bench_definitions_removal,
    bench_single_step_calculation,
    compile_expressions,
);
criterion_main!(benches);
