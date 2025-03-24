use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use bevy_gauge::tags::ValueTag;
// Import your ValueTag implementation

// Benchmark comparing ValueTag vs String as HashMap keys
pub fn bench_hashmap_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("HashMap Key Lookup");

    // Define sample sizes
    let sizes = [10, 100, 1000, 10000];

    for size in sizes.iter() {
        // 1. Setup ValueTag HashMap
        let tag_map = setup_value_tag_hashmap(*size);

        // 2. Setup String HashMap
        let string_map = setup_string_hashmap(*size);

        // Generate lookup keys
        let (tag_keys, string_keys) = generate_lookup_keys(*size);

        // Benchmark ValueTag lookup
        group.bench_with_input(
            BenchmarkId::new("ValueTag", size),
            &tag_keys,
            |b, keys| {
                b.iter(|| {
                    for key in keys {
                        black_box(tag_map.get(key));
                    }
                })
            },
        );

        // Benchmark String lookup
        group.bench_with_input(
            BenchmarkId::new("String", size),
            &string_keys,
            |b, keys| {
                b.iter(|| {
                    for key in keys {
                        black_box(string_map.get(key));
                    }
                })
            },
        );
    }

    group.finish();
}

// Benchmark the hashing operation itself
pub fn bench_hash_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Hash Operation");

    // Create a complex ValueTag
    let mut tag = ValueTag::new("damage".to_string(), None);
    tag.add_all_group("physical".to_string());

    let mut values = HashSet::new();
    values.insert("sword".to_string());
    values.insert("axe".to_string());
    values.insert("mace".to_string());
    tag.add_any_of_group("weapon".to_string(), values);

    // Create an equivalent string
    let tag_string = tag.stringify();

    // Benchmark ValueTag hash operation
    group.bench_function("ValueTag hash", |b| {
        b.iter(|| {
            use std::hash::{Hash, Hasher};
            use std::collections::hash_map::DefaultHasher;

            let mut hasher = DefaultHasher::new();
            black_box(&tag).hash(&mut hasher);
            black_box(hasher.finish())
        })
    });

    // Benchmark String hash operation
    group.bench_function("String hash", |b| {
        b.iter(|| {
            use std::hash::{Hash, Hasher};
            use std::collections::hash_map::DefaultHasher;

            let mut hasher = DefaultHasher::new();
            black_box(&tag_string).hash(&mut hasher);
            black_box(hasher.finish())
        })
    });

    group.finish();
}

// Benchmark insertion operation
pub fn bench_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("HashMap Insertion");

    // Define sample sizes
    let sizes = [10, 100, 1000];

    for size in sizes.iter() {
        // Generate keys for insertion
        let (tag_keys, string_keys) = generate_insertion_keys(*size);

        // Benchmark ValueTag insertion
        group.bench_with_input(
            BenchmarkId::new("ValueTag", size),
            &tag_keys,
            |b, keys| {
                b.iter(|| {
                    let mut map: HashMap<ValueTag, i32> = HashMap::with_capacity(*size);
                    for (i, key) in keys.iter().enumerate() {
                        map.insert(key.clone(), i as i32);
                    }
                    black_box(map)
                })
            },
        );

        // Benchmark String insertion
        group.bench_with_input(
            BenchmarkId::new("String", size),
            &string_keys,
            |b, keys| {
                b.iter(|| {
                    let mut map: HashMap<String, i32> = HashMap::with_capacity(*size);
                    for (i, key) in keys.iter().enumerate() {
                        map.insert(key.clone(), i as i32);
                    }
                    black_box(map)
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

// Generate keys for insertion benchmarking
fn generate_insertion_keys(size: usize) -> (Vec<ValueTag>, Vec<String>) {
    let mut tag_keys = Vec::with_capacity(size);
    let mut string_keys = Vec::with_capacity(size);

    for i in 0..size {
        let tag = create_value_tag(i);
        let key = tag.stringify();

        tag_keys.push(tag);
        string_keys.push(key);
    }

    (tag_keys, string_keys)
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
    targets = bench_hashmap_lookup, bench_hash_operation, bench_insertion
}

criterion_main!(benches);