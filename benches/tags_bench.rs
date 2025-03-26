use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;
use bevy_gauge::prelude::*;
use bevy_gauge::tags::{TagGroup, ValueTag};
use bevy_gauge::tags::{BitTagVector, TagRegistry, BitPolicy};

// Benchmark comparing ValueTag vs BitTagVector vs String as HashMap keys
pub fn bench_hashmap_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("HashMap Key Lookup");

    // Define sample sizes
    let sizes = [10, 100, 1000, 10000];

    for &size in sizes.iter() {
        // ValueTag lookup benchmark
        {
            // Setup ValueTag HashMap
            let tag_map = setup_value_tag_hashmap(size);

            // Generate lookup keys
            let (tag_keys, _) = generate_lookup_keys(size);

            group.bench_function(
                BenchmarkId::new("ValueTag", size),
                |b| {
                    b.iter(|| {
                        for key in &tag_keys {
                            black_box(tag_map.get(key));
                        }
                    })
                },
            );
        }

        // String lookup benchmark
        {
            // Setup String HashMap
            let string_map = setup_string_hashmap(size);

            // Generate lookup keys
            let (_, string_keys) = generate_lookup_keys(size);

            group.bench_function(
                BenchmarkId::new("String", size),
                |b| {
                    b.iter(|| {
                        for key in &string_keys {
                            black_box(string_map.get(key));
                        }
                    })
                },
            );
        }

        // BitTagVector lookup benchmark
        {
            // Setup BitTagVector HashMap with registry
            let registry = TagRegistry::new();
            let (bit_tag_map, registry) = setup_bittag_hashmap(size, registry);

            // Generate lookup keys
            let (tag_keys, _) = generate_lookup_keys(size);

            // Convert to BitTagVectors
            let bit_tag_keys: Vec<_> = tag_keys
                .iter()
                .map(|tag| registry.convert_tag(tag))
                .collect();

            group.bench_function(
                BenchmarkId::new("BitTagVector", size),
                |b| {
                    b.iter(|| {
                        for key in &bit_tag_keys {
                            black_box(bit_tag_map.get(key));
                        }
                    })
                },
            );
        }
    }

    group.finish();
}

// Benchmark qualification operations
pub fn bench_tag_qualification(c: &mut Criterion) {
    let mut group = c.benchmark_group("Tag Qualification");

    // Define sample sizes for the number of tags to qualify against
    let sizes = [10, 100, 1000];

    for &size in sizes.iter() {
        // ValueTag benchmark
        {
            let (target_tags, modifier_tags) = generate_tag_pairs(size);

            group.bench_function(
                BenchmarkId::new("ValueTag", size),
                |b| {
                    b.iter(|| {
                        let mut count = 0;
                        for i in 0..size {
                            if modifier_tags[i].qualifies_for(&target_tags[i]) {
                                count += 1;
                            }
                        }
                        black_box(count)
                    })
                },
            );
        }

        // BitTagVector benchmark
        {
            let (target_tags, modifier_tags) = generate_tag_pairs(size);
            let registry = TagRegistry::new();

            // Convert to BitTagVectors
            let bit_target_tags: Vec<_> = target_tags
                .iter()
                .map(|tag| registry.convert_tag(tag))
                .collect();

            let bit_modifier_tags: Vec<_> = modifier_tags
                .iter()
                .map(|tag| registry.convert_tag(tag))
                .collect();

            group.bench_function(
                BenchmarkId::new("BitTagVector", size),
                |b| {
                    b.iter(|| {
                        let mut count = 0;
                        for i in 0..size {
                            if bit_modifier_tags[i].qualifies_for(&bit_target_tags[i]) {
                                count += 1;
                            }
                        }
                        black_box(count)
                    })
                },
            );
        }

        // Registry-based policy benchmark
        {
            let (target_tags, modifier_tags) = generate_tag_pairs(size);
            let registry = TagRegistry::new();
            let registry_ref = &registry;

            group.bench_function(
                BenchmarkId::new("RegistryPolicy", size),
                |b| {
                    b.iter(|| {
                        let mut count = 0;
                        for i in 0..size {
                            if registry_ref.qualifies_with_policy(&modifier_tags[i], &target_tags[i]) {
                                count += 1;
                            }
                        }
                        black_box(count)
                    })
                },
            );
        }
    }

    group.finish();
}

// Benchmark conversion from ValueTag to BitTagVector
pub fn bench_tag_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("Tag Conversion");

    // Define sample sizes
    let sizes = [10, 100, 1000];

    for &size in sizes.iter() {
        // Cold conversion benchmark (no cache)
        {
            // Generate a set of tags with varying complexity
            let tags = generate_complex_tags(size);

            group.bench_function(
                BenchmarkId::new("Cold Conversion", size),
                |b| {
                    b.iter_batched(
                        || {
                            // Create a fresh registry for each batch
                            (TagRegistry::new(), tags.clone())
                        },
                        |(fresh_registry, tags_to_convert)| {
                            for tag in &tags_to_convert {
                                black_box(fresh_registry.convert_tag(tag));
                            }
                        },
                        criterion::BatchSize::SmallInput,
                    )
                },
            );
        }

        // Warm conversion benchmark (with cache)
        {
            // Generate a set of tags with varying complexity
            let tags = generate_complex_tags(size);

            // Create a registry for conversion with cache
            let registry = TagRegistry::new();

            // Pre-warm the cache
            for tag in &tags {
                registry.convert_tag(tag);
            }

            // Clear the registry's internal cache to avoid impacting the benchmark
            registry.clear_cache();

            // Now benchmark with a pre-populated registry
            let registry_ref = &registry;

            group.bench_function(
                BenchmarkId::new("Warm Conversion", size),
                |b| {
                    b.iter(|| {
                        for tag in &tags {
                            black_box(registry_ref.convert_tag(tag));
                        }
                    })
                },
            );
        }
    }

    group.finish();
}

// Benchmark finding qualifying tags in collections
pub fn bench_find_qualifying(c: &mut Criterion) {
    let mut group = c.benchmark_group("Find Qualifying Tags");

    // Define collection sizes
    let sizes = [10, 100, 1000];

    for &size in sizes.iter() {
        // Manual iteration benchmark
        {
            // Create a registry and collection for this benchmark
            let registry = TagRegistry::new();
            let collection_tags = generate_complex_tags(size);

            // Associate them with entities
            let mut tag_entities = Vec::with_capacity(size);
            for (i, tag) in collection_tags.iter().enumerate() {
                let entity = Entity::from_raw(i as u32);
                registry.register_collection_tag(entity, tag);
                tag_entities.push((tag.clone(), entity));
            }

            // Create a handful of query tags
            let query_tags = generate_complex_tags(10);

            group.bench_function(
                BenchmarkId::new("Manual Iteration", size),
                |b| {
                    b.iter(|| {
                        let mut total_matches = 0;
                        for query in &query_tags {
                            for (tag, _) in &tag_entities {
                                if query.qualifies_for(tag) {
                                    total_matches += 1;
                                }
                            }
                        }
                        black_box(total_matches)
                    })
                },
            );
        }

        // Registry-based approach
        {
            // Create a fresh registry for this benchmark
            let registry = TagRegistry::new();
            let collection_tags = generate_complex_tags(size);

            // Associate them with entities
            for (i, tag) in collection_tags.iter().enumerate() {
                let entity = Entity::from_raw(i as u32);
                registry.register_collection_tag(entity, tag);
            }

            // Create a handful of query tags
            let query_tags = generate_complex_tags(10);
            let registry_ref = &registry;

            group.bench_function(
                BenchmarkId::new("Registry", size),
                |b| {
                    b.iter(|| {
                        let mut total_matches = 0;
                        for query in &query_tags {
                            let matches = registry_ref.find_qualifying_entities(query);
                            total_matches += matches.len();
                        }
                        black_box(total_matches)
                    })
                },
            );
        }

        // Registry "any value" approach (OR semantics)
        {
            let registry = TagRegistry::new();
            let collection_tags = generate_complex_tags(size);

            // Associate them with entities
            for (i, tag) in collection_tags.iter().enumerate() {
                let entity = Entity::from_raw(i as u32);
                registry.register_collection_tag(entity, tag);
            }

            // Create a handful of query tags
            let query_tags = generate_complex_tags(10);
            let registry_ref = &registry;

            group.bench_function(
                BenchmarkId::new("Registry-AnyValue", size),
                |b| {
                    b.iter(|| {
                        let mut total_matches = 0;
                        for query in &query_tags {
                            let matches = registry_ref.find_entities_with_any_value(query);
                            total_matches += matches.len();
                        }
                        black_box(total_matches)
                    })
                },
            );
        }
    }

    group.finish();
}

// Benchmark policy-based qualification with different complexities
pub fn bench_policy_qualification(c: &mut Criterion) {
    let mut group = c.benchmark_group("Policy Qualification");

    // Define sample sizes
    let sizes = [10, 50, 100];

    for &size in sizes.iter() {
        // Create a registry with policies
        let mut registry = TagRegistry::new();
        registry.set_default_policy("weapon_type", BitPolicy::Permissive);
        registry.set_default_policy("elemental", BitPolicy::Default(false));
        registry.set_default_policy("target", BitPolicy::Strict);
        registry.set_default_policy("range", BitPolicy::Permissive);

        // Generate complex test cases
        let test_cases = generate_policy_test_cases(size);
        let registry_ref = &registry;

        group.bench_function(
            BenchmarkId::new("PolicyQualification", size),
            |b| {
                b.iter(|| {
                    let mut count = 0;
                    for (modifier, target) in &test_cases {
                        if registry_ref.qualifies_with_policy(modifier, target) {
                            count += 1;
                        }
                    }
                    black_box(count)
                })
            },
        );
    }

    group.finish();
}

// Helper functions

// Setup a HashMap with ValueTag keys
fn setup_value_tag_hashmap(size: usize) -> HashMap<ValueTag, i32> {
    let mut map = HashMap::with_capacity(size);

    for i in 0..size {
        let tag = create_value_tag(i);
        map.insert(tag, i as i32);
    }

    map
}

// Setup a HashMap with String keys
fn setup_string_hashmap(size: usize) -> HashMap<String, i32> {
    let mut map = HashMap::with_capacity(size);

    for i in 0..size {
        let tag = create_value_tag(i);
        let key = tag.stringify();
        map.insert(key, i as i32);
    }

    map
}

// Setup a HashMap with BitTagVector keys
fn setup_bittag_hashmap(size: usize, registry: TagRegistry) -> (HashMap<BitTagVector, i32>, TagRegistry) {
    let mut map = HashMap::with_capacity(size);

    for i in 0..size {
        let tag = create_value_tag(i);
        let bit_tag = registry.convert_tag(&tag);
        map.insert(bit_tag, i as i32);
    }

    (map, registry)
}

// Generate lookup keys for benchmarking
fn generate_lookup_keys(size: usize) -> (Vec<ValueTag>, Vec<String>) {
    let mut tag_keys = Vec::with_capacity(size);
    let mut string_keys = Vec::with_capacity(size);

    // Create a mix of keys that exist in the map and some that don't
    for i in 0..size {
        // Use every other key for lookup to simulate realistic access patterns
        if i % 2 == 0 {
            let tag = create_value_tag(i);
            string_keys.push(tag.stringify());
            tag_keys.push(tag);
        } else {
            let tag = create_value_tag(size + i); // These won't be in the map
            string_keys.push(tag.stringify());
            tag_keys.push(tag);
        }
    }

    (tag_keys, string_keys)
}

// Generate pairs of related tags for qualification testing
fn generate_tag_pairs(size: usize) -> (Vec<ValueTag>, Vec<ValueTag>) {
    let mut target_tags = Vec::with_capacity(size);
    let mut modifier_tags = Vec::with_capacity(size);

    for i in 0..size {
        // Create a target tag with some specificity
        let mut target = create_value_tag(i);

        // Create a related modifier tag that should qualify for the target
        // We'll make it slightly more specific or general to test qualification edge cases
        let mut modifier = target.clone();

        if i % 3 == 0 {
            // Make the modifier more general - remove a group
            if let Some(groups) = &mut modifier.groups {
                if !groups.is_empty() {
                    let first_key = groups.keys().next().unwrap().clone();
                    groups.remove(&first_key);
                }
            }
        } else if i % 3 == 1 {
            // Make the modifier match the target exactly
            // (already done by cloning)
        } else {
            // Make the modifier use an "All" group where the target has specific values
            if let Some(groups) = &target.groups {
                if !groups.is_empty() {
                    let first_key = groups.keys().next().unwrap().clone();
                    if let Some(TagGroup::AnyOf(_)) = groups.get(&first_key) {
                        modifier.add_all_group(first_key);
                    }
                }
            }
        }

        target_tags.push(target);
        modifier_tags.push(modifier);
    }

    (target_tags, modifier_tags)
}

// Generate a collection of tags with varying complexity
fn generate_complex_tags(size: usize) -> Vec<ValueTag> {
    let mut tags = Vec::with_capacity(size);

    for i in 0..size {
        tags.push(create_value_tag(i));
    }

    tags
}

// Generate test cases for policy qualification
fn generate_policy_test_cases(size: usize) -> Vec<(ValueTag, ValueTag)> {
    let mut test_cases = Vec::with_capacity(size);

    // Define base tag types
    let base_tags = [
        ("Damage", "damage"),
        ("Spell", "spell"),
        ("Attack", "attack"),
        ("Entity", "entity"),
        ("Effect", "effect"),
    ];

    // Define group names
    let group_names = [
        "elemental",
        "weapon_type",
        "target",
        "range",
        "skill_type",
        "creature_type",
        "resist",
        "effect",
    ];

    // Define values for each group
    let values = [
        // elemental
        &["fire", "ice", "lightning", "earth", "wind", "water", "physical"][..],
        // weapon_type
        &["sword", "axe", "mace", "bow", "staff", "dual_wield"][..],
        // target
        &["enemy", "ally", "area", "self", "boss"][..],
        // range
        &["melee", "ranged", "short", "long"][..],
        // skill_type
        &["magic", "physical", "hybrid", "special"][..],
        // creature_type
        &["boss", "minion", "humanoid", "undead", "beast"][..],
        // resist
        &["physical", "magical", "elemental", "fire", "ice"][..],
        // effect
        &["bleed", "stun", "slow", "poison", "burn"][..],
    ];

    for i in 0..size {
        // Create target tag with 2-3 groups
        let (base_name, _) = base_tags[i % base_tags.len()];
        let mut target = ValueTag::new(base_name.to_string(), None);

        let num_target_groups = 2 + (i % 2);
        for j in 0..num_target_groups {
            let group_idx = (i + j) % group_names.len();
            let group_name = group_names[group_idx];

            if j % 2 == 0 {
                // Add specific value
                let value_set = values[group_idx];
                let value = value_set[i % value_set.len()];

                let mut value_set = HashSet::new();
                value_set.insert(value.to_string());
                target.add_any_of_group(group_name.to_string(), value_set);
            } else {
                // Add All group
                target.add_all_group(group_name.to_string());
            }
        }

        // Create modifier tag - sometimes matching, sometimes not
        let (mod_base_name, _) = base_tags[i % base_tags.len()];
        let mut modifier = ValueTag::new(mod_base_name.to_string(), None);

        // Decide if this should match or not (for interesting benchmark results)
        let should_match = i % 3 != 0;

        let num_mod_groups = 1 + (i % 3);
        for j in 0..num_mod_groups {
            let group_idx = (i + j) % group_names.len();
            let group_name = group_names[group_idx];

            if should_match {
                // Create a modifier that should match via policy
                if j < num_target_groups {
                    // Use same group as target
                    if let Some(groups) = &target.groups {
                        if let Some(group) = groups.get(group_name) {
                            match group {
                                TagGroup::All => {
                                    modifier.add_all_group(group_name.to_string());
                                },
                                TagGroup::AnyOf(values) => {
                                    let first_value = values.iter().next().unwrap();
                                    let mut value_set = HashSet::new();
                                    value_set.insert(first_value.clone());
                                    modifier.add_any_of_group(group_name.to_string(), value_set);
                                }
                            }
                        }
                    }
                } else {
                    // Add a group using policy rules
                    match group_name {
                        "weapon_type" => {
                            // For permissive policy, add any value
                            let value_set = values[1]; // weapon_type values
                            let value = value_set[i % value_set.len()];

                            let mut value_set = HashSet::new();
                            value_set.insert(value.to_string());
                            modifier.add_any_of_group(group_name.to_string(), value_set);
                        },
                        "elemental" => {
                            // For default policy, use default value
                            let mut value_set = HashSet::new();
                            value_set.insert("physical".to_string());
                            modifier.add_any_of_group(group_name.to_string(), value_set);
                        },
                        _ => {
                            // For other groups, use an All group
                            modifier.add_all_group(group_name.to_string());
                        }
                    }
                }
            } else {
                // Create a modifier that shouldn't match
                let value_set = values[group_idx];
                let value = value_set[(i + 10) % value_set.len()]; // Ensure different value

                let mut value_set = HashSet::new();
                value_set.insert(value.to_string());
                modifier.add_any_of_group(group_name.to_string(), value_set);
            }
        }

        test_cases.push((modifier, target));
    }

    test_cases
}

// Create a ValueTag with varying complexity based on index
fn create_value_tag(index: usize) -> ValueTag {
    let base_names = ["damage", "resist", "bonus", "attack", "defense", "critical", "evade", "accuracy"];
    let group_names = ["physical", "magical", "elemental", "melee", "ranged", "weapon", "armor", "accessory"];
    let value_options = ["fire", "ice", "lightning", "earth", "wind", "water", "light", "dark",
        "sword", "axe", "mace", "dagger", "staff", "bow", "wand", "shield"];

    let base_index = index % base_names.len();
    let primary_value = base_names[base_index].to_string();

    let mut tag = ValueTag::new(primary_value, None);

    // Add 1-3 groups based on the index
    let num_groups = 1 + (index % 3);

    for i in 0..num_groups {
        let group_index = (index + i) % group_names.len();
        let group_name = group_names[group_index].to_string();

        if i % 2 == 0 {
            // Add an All group
            tag.add_all_group(group_name);
        } else {
            // Add an AnyOf group with 1-3 values
            let num_values = 1 + (index % 3);
            let mut values = HashSet::new();

            for j in 0..num_values {
                let value_index = (index + i + j) % value_options.len();
                values.insert(value_options[value_index].to_string());
            }

            tag.add_any_of_group(group_name, values);
        }
    }

    tag
}

// Benchmark specific configurations
criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .sample_size(50);
    targets = bench_hashmap_lookup, bench_tag_qualification, bench_tag_conversion, 
             bench_find_qualifying, bench_policy_qualification
}

criterion_main!(benches);
