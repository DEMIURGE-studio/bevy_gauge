use bevy::prelude::*;
use bevy_utils::HashMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bevy_guage::prelude::{
    StatDefinitions, StatContext, StatContextRefs, Stats,
    // plus any other items you need
};

use std::fmt::Debug;

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

/// A benchmark that tests a deep hierarchy of parent contexts.
/// E0 is the root, E1 points to E0, E2 -> E1, E3 -> E2. 
/// Each entity has a stat referencing its parent's stat,
/// which itself may reference the *grandparent's* stat, etc.
fn bench_deep_hierarchy(c: &mut Criterion) {
    let mut world = World::default();

    // === 1) Spawn four entities to form a deep hierarchy ===
    let e0 = world.spawn_empty().id();
    let e1 = world.spawn_empty().id();
    let e2 = world.spawn_empty().id();
    let e3 = world.spawn_empty().id();

    // === 2) Insert StatDefinitions for each entity ===

    // E0: root-level stats
    {
        let mut defs = StatDefinitions::new();
        // As an example, let’s define two stats: "AddedLife" and "IncreasedLife",
        // plus a "TotalLife" expression referencing them.
        defs.add("AddedLife", 100).unwrap();
        defs.add("IncreasedLife", 50).unwrap();
        defs.set("TotalLife", bevy_guage::prelude::ExpressionPart::new(
            1, 
            "+= AddedLife * IncreasedLife / 100.0"
        ));
        world.entity_mut(e0).insert(defs);
    }

    // E1: references its parent's "TotalLife"
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLifeB", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.TotalLife + 20",
        ));
        world.entity_mut(e1).insert(defs);
    }

    // E2: references E1's "ChildLife"
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLifeA", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.ChildLifeB + 20",
        ));
        world.entity_mut(e2).insert(defs);
    }

    // E3: references E2's "ChildLife"
    {
        let mut defs = StatDefinitions::new();
        defs.set("ChildLife", bevy_guage::prelude::ExpressionPart::new(
            1,
            "+= parent.ChildLifeA + 20",
        ));
        world.entity_mut(e3).insert(defs);
    }

    // === 3) Insert StatContext for each entity, pointing "parent" to the correct parent. ===
    // Also insert a "self" context reference if you want to allow self lookups.
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

    // === 4) Insert a Stats component if needed (some code depends on it) ===
    world.entity_mut(e0).insert(Stats::default());
    world.entity_mut(e1).insert(Stats::default());
    world.entity_mut(e2).insert(Stats::default());
    world.entity_mut(e3).insert(Stats::default());

    // === 5) Build QueryStates for manual lookups ===
    let mut defs_query = world.query::<&StatDefinitions>();
    let mut ctx_query  = world.query::<&StatContext>();

    // We'll retrieve E3's definitions (where we want to evaluate "ChildLife").
    let e3_defs = defs_query.get(&world, e3).unwrap();

    // Build the ephemeral context for E3 once (the “cold” part).
    let ctx_refs = build(e3, &world, &defs_query, &ctx_query);

    // === 6) Run the benchmark, evaluating E3's "ChildLife" 10k times. ===
    let mut group = c.benchmark_group("deep_hierarchy");
    group.bench_function("evaluate E3.ChildLife x10000", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let val = e3_defs.get("ChildLife", &ctx_refs).unwrap();
                // Use black_box to avoid the compiler optimizing away the read
                black_box(val);
            }
        });
    });
    group.finish();
}

// Group all benchmarks together
criterion_group!(
    benches,
    bench_deep_hierarchy
);
criterion_main!(benches);
