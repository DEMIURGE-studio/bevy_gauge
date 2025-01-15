use bevy::prelude::*;
use bevy_utils::HashMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bevy_guage::prelude::{
    StatDefinitions, StatContext, StatContextRefs, Stats, // Adjust as needed
    // plus anything else you need
};

/// Build a mock scenario with a single entity that has some stats and context,
/// then evaluate a stat repeatedly to measure performance.
fn bench_single_stat_eval(c: &mut Criterion) {
    // 1) Create a Bevy World (or minimal data) to hold your queries if needed.
    let mut world = World::default();
    
    // 2) Spawn an entity with StatDefinitions, StatContext, and Stats.
    let entity = world.spawn_empty().id();
    
    // Insert some example definitions
    {
        let mut defs = StatDefinitions::new();
        // Suppose these are the same stats you mentioned in your snippet:
        defs.set("Life.max", 1000);
        defs.set("Life.current", 500);
        defs.set("Juice.max", 200);
        defs.set("Juice.current", 100);
        
        // Add the component
        world.entity_mut(entity).insert(defs);
    }
    
    // Insert an empty StatContext referencing itself for "self"
    {
        let mut ctx = StatContext::default();
        ctx.insert("self", entity);
        world.entity_mut(entity).insert(ctx);
    }
    
    // Insert a Stats component (flattened values)
    {
        let stats = Stats::default();
        world.entity_mut(entity).insert(stats);
    }
    
    // 3) Now we can create a system param or manual queries to fetch the data
    //    we need for building the `StatContextRefs` each iteration.
    
    let mut query_defs = world.query::<&StatDefinitions>();
    let mut query_ctx  = world.query::<&StatContext>();
    
    // 4) Build the context. In a real scenario, you might do so each iteration,
    //    but if your context doesn't change, you can do it once outside the loop
    //    to measure only the evaluation cost. We'll do both ways in the example.
    
    let defs_ref = query_defs.get(&world, entity).unwrap();
    let _ctx_ref = query_ctx.get(&world, entity).unwrap();

    // 5) Make a closure that does the actual evaluation
    let mut group = c.benchmark_group("single_stat_eval");
    
    group.bench_function("evaluate Life.max once", |b| {
        b.iter(|| {
            // Re‑build the ephemeral StatContextRefs
            let ctx_refs = build(entity, &world, &query_defs, &query_ctx);
            // Evaluate "Life.max"
            let val = defs_ref.get("Life.max", &ctx_refs).unwrap();
            // Use black_box to avoid compiler optimizing away the result
            black_box(val)
        });
    });
    
    group.finish();
}

/// Example of a more complex scenario with multiple stats, subcontexts, etc.
fn bench_complex_eval(c: &mut Criterion) {
    // Similar setup as above, but with nested parent/child, etc.
    // Then measure evaluating a more complex expression that references parents.
    
    // For brevity, omitted here — structure is the same: create world, spawn entities,
    // insert definitions/context, and then in the benchmark closure, do the iteration.
    
    let mut group = c.benchmark_group("complex_eval");
    group.bench_function("evaluate complex scenario", |b| {
        b.iter(|| {
            // ...
        });
    });
    group.finish();
}


pub fn build<'a>(
    entity: Entity,
    world: &'a World,
    defs_query: &QueryState<&StatDefinitions>,
    ctx_query: &QueryState<&StatContext>,
) -> StatContextRefs<'a> {
    // We'll create a map for all sub-entries of this entity.
    let mut map = HashMap::default();

    // If the entity itself has definitions, we store them under “self”.
    if let Ok(defs) = defs_query.get_manual(world, entity) {
        map.insert("self", StatContextRefs::Definitions(defs));
    }

    // If the entity has a StatContext, build subcontext entries for each.
    if let Ok(stat_context) = ctx_query.get_manual(world, entity) {
        for (key, child_entity) in &stat_context.sources {
            // Avoid cycles, etc.
            if *child_entity == entity {
                continue;
            }
            let child_src =
                build(*child_entity, world, defs_query, ctx_query);
            map.insert(key.as_str(), child_src);
        }
    }

    StatContextRefs::SubContext(map)
}

// Criterion macro to group benchmarks together
criterion_group!(benches, bench_single_stat_eval, bench_complex_eval);
criterion_main!(benches);
