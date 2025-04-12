// use bevy::prelude::*;
// use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
// use bevy_gauge::prelude::*;

// // Define constants for tag-based benchmarks
// const FIRE: u32 = 0x01;
// const COLD: u32 = 0x02;
// const LIGHTNING: u32 = 0x04;
// const SWORD: u32 = 0x0100;
// const BOW: u32 = 0x0200;
// const DAMAGE_TYPE: u32 = 0xFF;
// const WEAPON_TYPE: u32 = 0xFF00;

// // Helper function to set up a simple app with an entity
// fn setup_app() -> (App, Entity) {
//     let mut app = App::new();
//     let entity = app.world_mut().spawn(Stats::new()).id();
//     (app, entity)
// }

// // Helper function to set up an app with multiple entities
// fn setup_app_with_entities(count: usize) -> (App, Vec<Entity>) {
//     let mut app = App::new();
//     let entities = (0..count)
//         .map(|_| app.world_mut().spawn(Stats::new()).id())
//         .collect::<Vec<_>>();
//     (app, entities)
// }

// // Helper function to add a basic stat to an entity
// fn add_basic_stat(app: &mut App, entity: Entity, stat_name: &str, value: f32) {
//     // Create an owned copy of the stat name string
//     let stat_name_owned = stat_name.to_string();
    
//     let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//         stat_accessor.add_modifier(entity, &stat_name_owned, value);
//     });
//     let _ = app.world_mut().run_system(system_id);
// }

// pub fn bench_simple_stat_access(c: &mut Criterion) {
//     let mut group = c.benchmark_group("simple_stat_access");
    
//     for value in [10.0, 100.0, 1000.0].iter() {
//         group.bench_with_input(BenchmarkId::from_parameter(value), value, |b, &value| {
//             let (mut app, entity) = setup_app();
//             add_basic_stat(&mut app, entity, "Life.Added", value);
            
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                 black_box(stats.get("Life.Added").unwrap_or(0.0));
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_stat_evaluation(c: &mut Criterion) {
//     let mut group = c.benchmark_group("stat_evaluation");
    
//     for value in [10.0, 100.0, 1000.0].iter() {
//         group.bench_with_input(BenchmarkId::from_parameter(value), value, |b, &value| {
//             let (mut app, entity) = setup_app();
//             add_basic_stat(&mut app, entity, "Life.Added", value);
            
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                 black_box(stats.evaluate_by_string("Life.Added"));
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_dependent_stats(c: &mut Criterion) {
//     let mut group = c.benchmark_group("dependent_stats");
    
//     // Benchmark with different dependency chain lengths
//     for chain_length in [1, 5, 10, 20].iter() {
//         group.bench_with_input(BenchmarkId::from_parameter(chain_length), chain_length, |b, &chain_length| {
//             let (mut app, entity) = setup_app();
            
//             // Set up a dependency chain of the specified length
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 stat_accessor.add_modifier(entity, "Base", 10.0);
                
//                 // Create a chain of dependencies where each level depends on the previous
//                 for i in 1..=chain_length {
//                     let prev_stat = if i == 1 { "Base".to_string() } else { format!("Level{}", i-1) };
//                     let curr_stat = format!("Level{}", i);
                    
//                     // Each level multiplies the previous by 1.1
//                     stat_accessor.add_modifier(entity, &curr_stat, format!("{} * 1.1", prev_stat));
//                 }
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Benchmark accessing the final stat in the chain
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                 black_box(stats.evaluate_by_string(&format!("Level{}", chain_length)));
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_entity_dependencies(c: &mut Criterion) {
//     let mut group = c.benchmark_group("entity_dependencies");
    
//     // Benchmark with different dependency chain lengths between entities
//     for chain_length in [1, 3, 5, 10].iter() {
//         group.bench_with_input(BenchmarkId::from_parameter(chain_length), chain_length, |b, &chain_length| {
//             let (mut app, entities) = setup_app_with_entities(chain_length + 1);
            
//             // Clone the entities vector for use in the system
//             let entities_clone = entities.clone();
            
//             // Set up a dependency chain of entities where each depends on the previous
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 // Set base value for the first entity
//                 stat_accessor.add_modifier(entities_clone[0], "Power.Added", 100.0);
                
//                 // Create a chain of entity dependencies
//                 for i in 1..=chain_length {
//                     // Register dependency on the previous entity
//                     stat_accessor.register_dependency(entities_clone[i], "Source", entities_clone[i-1]);
                    
//                     // Add a stat that depends on the previous entity
//                     stat_accessor.add_modifier(entities_clone[i], "Power.Added", "Source@Power.Added * 0.9");
//                 }
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Benchmark accessing the power stat of the last entity in the chain
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entities[chain_length]).unwrap();
//                 black_box(stats.evaluate_by_string("Power.Added"));
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_tag_based_stats(c: &mut Criterion) {
//     let mut group = c.benchmark_group("tag_based_stats");
    
//     // Benchmark with different numbers of tag combinations
//     for &tag_count in &[1, 3, 5, 10] {
//         group.bench_with_input(BenchmarkId::from_parameter(tag_count), &tag_count, |b, &tag_count| {
//             let (mut app, entity) = setup_app();
            
//             // Set up tag-based stats
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 // Add base damage
//                 stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", u32::MAX), 10.0);
                
//                 // Add elemental damage for each tag
//                 for i in 0..tag_count {
//                     let tag = 1u32 << i;
//                     stat_accessor.add_modifier(entity, &format!("Damage.Added.{}", (u32::MAX & !DAMAGE_TYPE) | tag), 5.0 + i as f32);
//                     stat_accessor.add_modifier(entity, &format!("Damage.Increased.{}", (u32::MAX & !DAMAGE_TYPE) | tag), 0.1 * (i as f32 + 1.0));
//                 }
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Benchmark evaluating a specific tag combination
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                 // Use the first tag for benchmarking
//                 black_box(stats.evaluate_by_string(&format!("Damage.{}", 0x01)));
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_mixed_dependencies(c: &mut Criterion) {
//     let mut group = c.benchmark_group("mixed_dependencies");
    
//     // Benchmark with different complexity levels of mixed dependencies
//     for &complexity in &[1, 3, 5, 10] {
//         group.bench_with_input(BenchmarkId::from_parameter(complexity), &complexity, |b, &complexity| {
//             let (mut app, entities) = setup_app_with_entities(complexity + 1);
            
//             // Clone entities to pass an owned copy to the system
//             let entities_clone = entities.clone();
            
//             // Set up complex mixed dependencies
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 // Base entity with power
//                 stat_accessor.add_modifier(entities_clone[0], "Power.Added", 20.0);
                
//                 // For each level of complexity, add more interconnected dependencies
//                 for i in 1..=complexity {
//                     // Register dependencies on the base entity
//                     stat_accessor.register_dependency(entities_clone[i], "Source", entities_clone[0]);
                    
//                     // Add local multiplier
//                     stat_accessor.add_modifier(entities_clone[i], "Multiplier.Added", 1.0 + (i as f32 * 0.1));
                    
//                     // Add calculation that mixes entity and local dependencies
//                     stat_accessor.add_modifier(entities_clone[i], "Damage.Added", "Source@Power.Added * Multiplier.Added");
                    
//                     // For extra complexity, add dependencies between non-base entities
//                     if i > 1 {
//                         // Register dependency on the previous entity
//                         stat_accessor.register_dependency(entities_clone[i], "Prev", entities_clone[i-1]);
                        
//                         // Add a more complex calculation with multiple entity references
//                         stat_accessor.add_modifier(
//                             entities_clone[i],
//                             "ComplexDamage.Added",
//                             "(Source@Power.Added * 0.5) + (Prev@Damage.Added * 0.3) * Multiplier.Added"
//                         );
//                     }
//                 }
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Benchmark evaluating the complex damage of the last entity
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entities[complexity]).unwrap();
//                 if complexity > 1 {
//                     black_box(stats.evaluate_by_string("ComplexDamage.Added"));
//                 } else {
//                     black_box(stats.evaluate_by_string("Damage.Added"));
//                 }
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_stats_update(c: &mut Criterion) {
//     let mut group = c.benchmark_group("stats_update");
    
//     // Benchmark with different numbers of connected entities
//     for &entity_count in &[1, 10, 50, 100] {
//         group.bench_with_input(BenchmarkId::from_parameter(entity_count), &entity_count, |b, &entity_count| {
//             let (mut app, entities) = setup_app_with_entities(entity_count + 1);
            
//             // Set up a star pattern: one central entity that all others depend on
//             let central = entities[0];
//             // Create an owned copy of the dependent entities
//             let dependent_entities = entities[1..].to_vec();
            
//             // Set up dependencies - we need to clone the dependent_entities for the closure
//             let dependent_for_system = dependent_entities.clone();
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 // Set base aura value
//                 stat_accessor.add_modifier(central, "Aura.Added", 10.0);
                
//                 // All entities depend on the central one
//                 for (i, &entity) in dependent_for_system.iter().enumerate() {
//                     stat_accessor.register_dependency(entity, "Central", central);
//                     // Different multiplier for each entity to make cache effects less predictable
//                     let multiplier = 0.8 + ((i as f32 % 5.0) * 0.1);
//                     stat_accessor.add_modifier(entity, "Buff.Added", format!("Central@Aura.Added * {}", multiplier));
//                 }
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Benchmark updating the central entity and seeing how it affects performance
//             b.iter(|| {
//                 // Update the central entity's aura
//                 let update_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                     stat_accessor.add_modifier(central, "Aura.Added", 1.0);
//                 });
//                 black_box(app.world_mut().run_system(update_id));
                
//                 // Get values from all dependent entities to ensure updates propagate
//                 for &entity in &dependent_entities {
//                     let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                     black_box(stats.evaluate_by_string("Buff.Added"));
//                 }
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_complex_expression_evaluation(c: &mut Criterion) {
//     let mut group = c.benchmark_group("complex_expression_evaluation");
    
//     // Benchmark with expressions of different complexity
//     let expressions = [
//         "Base + Added",
//         "Base * (1 + Increased)",
//         "Base * (1 + Increased) + Added",
//         "min(Base * (1 + Increased) + Added, Cap)",
//         "(Base * (1 + Increased) + Added) * (1 + More) - Taken"
//     ];
    
//     for (i, expr) in expressions.iter().enumerate() {
//         group.bench_with_input(BenchmarkId::from_parameter(i), expr, |b, _| {
//             let (mut app, entity) = setup_app();
            
//             // Clone the expression string here (inside the bench_with_input closure)
//             let expr_string = expr.to_string();
            
//             // Set up all the relevant stats
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 stat_accessor.add_modifier(entity, "Base", 100.0);
//                 stat_accessor.add_modifier(entity, "Added", 50.0);
//                 stat_accessor.add_modifier(entity, "Increased", 0.3);
//                 stat_accessor.add_modifier(entity, "More", 0.2);
//                 stat_accessor.add_modifier(entity, "Taken", 25.0);
//                 stat_accessor.add_modifier(entity, "Cap", 200.0);
                
//                 // Use a reference to the expression string to avoid moving it
//                 stat_accessor.add_modifier(entity, "Result", expr_string.as_str());
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Benchmark evaluating the complex expression
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                 black_box(stats.evaluate_by_string("Result"));
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_many_modifiers(c: &mut Criterion) {
//     let mut group = c.benchmark_group("many_modifiers");
    
//     // Benchmark with different numbers of modifiers on the same stat
//     for &modifier_count in &[1, 10, 50, 100] {
//         group.bench_with_input(BenchmarkId::from_parameter(modifier_count), &modifier_count, |b, &modifier_count| {
//             let (mut app, entity) = setup_app();
            
//             // Add many modifiers to the same stat
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 for i in 0..modifier_count {
//                     stat_accessor.add_modifier(entity, "Power.Added", 1.0);
//                 }
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Benchmark evaluating a stat with many modifiers
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                 black_box(stats.evaluate_by_string("Power.Added"));
//             });
//         });
//     }
    
//     group.finish();
// }

// pub fn bench_many_stats(c: &mut Criterion) {
//     let mut group = c.benchmark_group("many_stats");
    
//     // Benchmark with different numbers of distinct stats
//     for &stat_count in &[10, 50, 100, 500] {
//         group.bench_with_input(BenchmarkId::from_parameter(stat_count), &stat_count, |b, &stat_count| {
//             let (mut app, entity) = setup_app();
            
//             // Add many distinct stats
//             let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut| {
//                 for i in 0..stat_count {
//                     stat_accessor.add_modifier(entity, &format!("Stat{}.Added", i), i as f32);
//                 }
//             });
//             let _ = app.world_mut().run_system(system_id);
            
//             // Pick a random stat to evaluate (middle of the range)
//             let target_stat = format!("Stat{}.Added", stat_count / 2);
            
//             // Benchmark evaluating one of many stats
//             b.iter(|| {
//                 let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
//                 black_box(stats.evaluate_by_string(&target_stat));
//             });
//         });
//     }
    
//     group.finish();
// }

// criterion_group!(
//     benches,
//     bench_simple_stat_access,
//     bench_stat_evaluation,
//     bench_dependent_stats,
//     bench_entity_dependencies,
//     bench_tag_based_stats,
//     bench_mixed_dependencies,
//     bench_stats_update,
//     bench_complex_expression_evaluation,
//     bench_many_modifiers,
//     bench_many_stats
// );
// criterion_main!(benches);