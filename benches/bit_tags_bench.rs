use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bevy::prelude::*;
use std::hash::{Hash, Hasher};
use bevy_gauge::tags::TagRegistry;

// Reproducing TagRegistry for the benchmark to be self-contained
fn create_populated_registry(primary_types: usize, subtypes_per_type: usize) -> TagRegistry {
    let mut registry = TagRegistry::new();

    for i in 0..primary_types {
        let primary = format!("PRIMARY_{}", i);
        registry.register_primary_type(&primary);

        for j in 0..subtypes_per_type {
            let subtype = format!("SUB_{}_{}", i, j);
            registry.register_subtype(&primary, &subtype);
        }
    }

    registry
}

// Helper to create compound tags (combining subtypes)
fn create_compound_tags(registry: &mut TagRegistry, primary_type: &str, count: usize, max_components: usize) -> Vec<u32> {
    let mut compounds = Vec::with_capacity(count);

    // Ensure max_components is at least 2
    let max_components = std::cmp::max(2, max_components);

    // Get all subtypes for this primary type - FIXED INDEXING
    let mut all_subtypes = Vec::new();

    // First, collect ALL subtypes that exist in the registry for this primary type
    if let Some(subtypes_map) = registry.string_to_id.get(primary_type) {
        for (subtype_name, &id) in subtypes_map.iter() {
            // Only add if it matches our expected format - this helps ensure we get only our test subtypes
            if subtype_name.starts_with("SUB_") {
                all_subtypes.push((subtype_name.clone(), id));
            }
        }
    }

    // If we don't have enough subtypes, add some
    if all_subtypes.len() < max_components {
        // Make sure we have enough subtypes by adding more if needed
        for i in all_subtypes.len()..max_components+1 {
            let subtype = format!("SUB_{}_extra_{}", primary_type, i);
            let id = registry.register_tag(primary_type, &subtype);
            all_subtypes.push((subtype, id));
        }
    }

    // Now create the compound tags
    for i in 0..count {
        // Take a deterministic approach instead of random for benchmarking
        let num_components = std::cmp::min(all_subtypes.len(), max_components);
        let mut compound_id = 0;

        for j in 0..num_components {
            // Use modulo to wrap around if we need more components than available subtypes
            let idx = (i + j) % all_subtypes.len();
            compound_id |= all_subtypes[idx].1;
        }

        // Register the compound tag
        let compound_name = format!("COMPOUND_{}_{}", primary_type, i);
        registry.string_to_id.get_mut(primary_type).unwrap().insert(compound_name.clone(), compound_id);
        registry.id_to_string.get_mut(primary_type).unwrap().insert(compound_id, compound_name);

        compounds.push(compound_id);
    }

    compounds
}

fn bench_registry_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("TagRegistry Creation");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("primary_types", size), size, |b, &size| {
            b.iter(|| {
                let mut registry = TagRegistry::new();
                for i in 0..size {
                    registry.register_primary_type(&format!("PRIMARY_{}", i));
                }
            });
        });
    }

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("subtypes_per_type", size), size, |b, &size| {
            b.iter(|| {
                let mut registry = TagRegistry::new();
                registry.register_primary_type("PRIMARY");
                for i in 0..size {
                    registry.register_subtype("PRIMARY", &format!("SUB_{}", i));
                }
            });
        });
    }

    group.finish();
}

fn bench_tag_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("TagRegistry Lookup");

    // Benchmark tag lookup with differently sized registries
    for &size in &[10, 100, 1000] {
        let registry = create_populated_registry(10, size);

        group.bench_with_input(BenchmarkId::new("lookup_by_name", size), &size, |b, _| {
            b.iter(|| {
                for i in 0..10 {
                    for j in 0..size {
                        let primary = format!("PRIMARY_{}", i);
                        let subtype = format!("SUB_{}_{}", i, j);
                        black_box(registry.get_id(&primary, &subtype));
                    }
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("lookup_by_id", size), &size, |b, _| {
            b.iter(|| {
                for i in 0..10 {
                    for j in 0..size {
                        // We know the IDs are 2^j for each subtype
                        let id = 1 << j;
                        let primary = format!("PRIMARY_{}", i);
                        black_box(registry.get_tag(&primary, id));
                    }
                }
            });
        });
    }

    group.finish();
}

fn bench_tag_qualification(c: &mut Criterion) {
    let mut group = c.benchmark_group("Tag Qualification");

    // Create a registry with sufficient subtypes for compound creation
    let mut registry = create_populated_registry(5, 10);

    // Create different numbers of compound tags to test scaling
    for &compound_count in &[5, 10, 20] {
        let primary_type = "PRIMARY_0";

        // Ensure the primary type exists
        registry.register_primary_type(primary_type);

        // Create compound tags - this will now ensure we have enough subtypes
        let compounds = create_compound_tags(&mut registry, primary_type, compound_count, 4);

        // Benchmark qualification checks - no need to skip as create_compound_tags ensures we have tags
        group.bench_with_input(BenchmarkId::new("qualification_checks", compound_count), &compound_count, |b, _| {
            b.iter(|| {
                for &compound_id in &compounds {
                    // Check qualification against all possible subtypes (up to 10)
                    for j in 0..10 {
                        let subtype_id = 1 << j;
                        black_box(registry.tag_qualifies_for(primary_type, subtype_id, compound_id));
                    }
                }
            });
        });
    }

    group.finish();
}

fn bench_realistic_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("Realistic Usage");

    // Create a realistic sized registry (with smaller numbers to avoid issues)
    let mut registry = create_populated_registry(100, 10);

    // Ensure primary types exist
    let primary_0 = "PRIMARY_0";
    let primary_1 = "PRIMARY_1";
    registry.register_primary_type(primary_0);
    registry.register_primary_type(primary_1);

    // Create compound tags for different categories - will add subtypes if needed
    let damage_compounds = create_compound_tags(&mut registry, primary_0, 10, 5);
    let weapon_compounds = create_compound_tags(&mut registry, primary_1, 10, 5);

    // Benchmark a realistic game scenario: matching modifiers to stats
    group.bench_function("modifier_matching", |b| {
        b.iter(|| {
            // Simulate checking if modifiers apply to stats
            for &modifier_tag in &damage_compounds {
                for j in 0..10 {
                    let stat_tag = 1 << j;
                    // Check if this damage modifier applies to this damage stat
                    black_box(registry.tag_qualifies_for(primary_0, modifier_tag, stat_tag));
                }
            }

            for &modifier_tag in &weapon_compounds {
                for j in 0..10 {
                    let stat_tag = 1 << j;
                    // Check if this weapon modifier applies to this weapon stat
                    black_box(registry.tag_qualifies_for(primary_1, modifier_tag, stat_tag));
                }
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tag_qualification,
    bench_realistic_usage,
    bench_registry_creation,
    bench_tag_lookup
);
criterion_main!(benches);
