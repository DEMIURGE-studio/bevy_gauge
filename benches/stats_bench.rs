use bevy::{ecs::system::RunSystemOnce, prelude::*};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bevy_gauge::prelude::*;

// Helper function to set up an app with a default Config, plugins, and a single entity
fn setup_app_for_bench() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy_gauge::plugin);
    let entity = app.world_mut().spawn(Stats::new()).id();
    (app, entity)
}

// Helper function to set up an app with multiple entities
fn setup_app_with_entities_for_bench(count: usize) -> (App, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy_gauge::plugin);
    let entities = (0..count)
        .map(|_| app.world_mut().spawn(Stats::new()).id())
        .collect::<Vec<_>>();
    (app, entities)
}

// Helper to run a system that adds a modifier for setup.
fn setup_stat_modifier_system(app: &mut App, entity: Entity, stat_name: &str, value: f32) {
    let stat_name_owned = stat_name.to_string();
    let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
        stat_accessor.add_modifier(entity, &stat_name_owned, value);
    });
    let _ = app.world_mut().run_system(system_id);
    app.update(); // Ensure modifier is processed if subsequent reads depend on it immediately
}

pub fn bench_stat_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("stat_access");
    
    for value in [10.0, 100.0, 1000.0].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(value), value, |b, &value| {
            let (mut app, entity) = setup_app_for_bench();
            setup_stat_modifier_system(&mut app, entity, "Life.base", value); // Using .base for clarity
            
            b.iter(|| {
                let entity_for_iter = entity; // Copy entity directly into the closure
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(entity_for_iter, "Life.base"));
                });
            });
        });
    }
    group.finish();
}

pub fn bench_dependent_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependent_stats_intra_entity");
    
    for chain_length in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(chain_length), chain_length, |b, &chain_length| {
            let (mut app, entity) = setup_app_for_bench();
            
            let final_stat_name = format!("Level{}", chain_length);
            let el = entity; // Copy for closure
            let cl = chain_length; // No dereference needed, already a value

            // Set up a dependency chain of the specified length
            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                stat_accessor.add_modifier(el, "Base", 10.0);
                for i in 1..=cl {
                    let prev_stat = if i == 1 { "Base".to_string() } else { format!("Level{}", i - 1) };
                    let curr_stat = format!("Level{}", i);
                    stat_accessor.add_modifier(el, &curr_stat, Expression::new(&format!("{} * 1.1", prev_stat)).unwrap());
                }
            });
            app.update();
            
            b.iter(|| {
                let el_for_iter = el; // Copy entity
                let final_stat_name_for_iter = final_stat_name.clone(); // Clone String inside b.iter
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(el_for_iter, &final_stat_name_for_iter));
                });
            });
        });
    }
    group.finish();
}

pub fn bench_entity_dependencies(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependent_stats_inter_entity");
    
    for chain_length in [1, 3, 5, 10].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(chain_length), chain_length, |b, &chain_length| {
            let (mut app, entities) = setup_app_with_entities_for_bench(chain_length + 1);
            let last_entity = entities[chain_length];
            let cl = chain_length; // User fixed: No dereference needed

            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                stat_accessor.add_modifier(entities[0], "Power.base", 100.0);
                for i in 1..=cl {
                    stat_accessor.register_source(entities[i], "Source", entities[i-1]);
                    stat_accessor.add_modifier(entities[i], "Power.base", Expression::new("Source@Power.base * 0.9").unwrap());
                }
            });
            app.update();
            
            b.iter(|| {
                let last_entity_for_iter = last_entity; // Copy entity
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(last_entity_for_iter, "Power.base"));
                });
            });
        });
    }
    group.finish();
}

pub fn bench_tag_based_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("tag_based_stats_evaluation");
    
    for &tag_count in &[1, 3, 5, 10] {
        group.bench_with_input(BenchmarkId::from_parameter(tag_count), &tag_count, |b, &tag_count| {
            let (mut app, entity) = setup_app_for_bench();
            
            // Setup Config for Tagged "Damage" stat
            Konfig::register_stat_type("Damage", "Tagged");
            Konfig::register_total_expression("Damage", "base * (1.0 + increased) + added");

            let el = entity; // Copy for closure
            let tc = tag_count; // Copy for closure
            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                stat_accessor.add_modifier(el, "Damage.base.0", 10.0); // Untagged base damage
                for i in 0..tc {
                    let tag = 1u32 << i; // Simple tag, e.g., 1, 2, 4, 8...
                    stat_accessor.add_modifier(el, &format!("Damage.added.{}", tag), 5.0 + i as f32);
                    stat_accessor.add_modifier(el, &format!("Damage.increased.{}", tag), 0.1 * (i as f32 + 1.0));
                }
            });
            app.update();

            let first_tag_path = format!("Damage.{}", 1u32 << 0); // Evaluate total damage for the first tag
            
            b.iter(|| {
                let el_for_iter = el; // Copy entity
                let first_tag_path_for_iter = first_tag_path.clone(); // Clone String inside b.iter
                 let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(el_for_iter, &first_tag_path_for_iter));
                });
            });
        });
    }
    group.finish();
}

pub fn bench_mixed_dependencies(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_dependencies_evaluation");
    
    for &complexity in &[1, 3, 5, 10] {
        group.bench_with_input(BenchmarkId::from_parameter(complexity), &complexity, |b, &complexity| {
            let (mut app, entities) = setup_app_with_entities_for_bench(complexity + 1);
            let last_entity = entities[complexity];
            let compl = complexity; // copy

            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                stat_accessor.add_modifier(entities[0], "Power.base", 20.0);
                for i in 1..=compl {
                    stat_accessor.register_source(entities[i], "Source", entities[0]);
                    stat_accessor.add_modifier(entities[i], "Multiplier.base", 1.0 + (i as f32 * 0.1));
                    stat_accessor.add_modifier(entities[i], "Damage.base", Expression::new("Source@Power.base * Multiplier.base").unwrap());
                    if i > 1 {
                        stat_accessor.register_source(entities[i], "Prev", entities[i-1]);
                        stat_accessor.add_modifier(
                            entities[i],
                            "ComplexDamage.base",
                            Expression::new("(Source@Power.base * 0.5) + (Prev@Damage.base * 0.3) * Multiplier.base").unwrap()
                        );
                    }
                }
            });
            app.update();
            
            let stat_to_eval = if complexity > 1 { "ComplexDamage.base".to_string() } else { "Damage.base".to_string() };
            b.iter(|| {
                let last_entity_for_iter = last_entity; // Copy entity
                let stat_to_eval_for_iter = stat_to_eval.clone(); // Clone String inside b.iter
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(last_entity_for_iter, &stat_to_eval_for_iter));
                });
            });
        });
    }
    group.finish();
}

pub fn bench_stats_update_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_update_propagation");
    
    for &entity_count in &[1, 10, 50, 100] { // Number of dependent entities
        group.bench_with_input(BenchmarkId::from_parameter(entity_count), &entity_count, |b, &entity_count| {
            let (mut app, entities) = setup_app_with_entities_for_bench(entity_count + 1);
            
            let central = entities[0];
            let dependent_entities_ids = entities[1..].to_vec(); // Clone for setup system

            // Setup: Central entity and dependents that rely on its "Aura.base"
            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                stat_accessor.add_modifier(central, "Aura.base", 10.0);
                for (i, &entity_id) in dependent_entities_ids.iter().enumerate() {
                    stat_accessor.register_source(entity_id, "CentralSource", central);
                    let multiplier = 0.8 + ((i as f32 % 5.0) * 0.1); // Vary multiplier
                    stat_accessor.add_modifier(entity_id, "Buff.value", Expression::new(&format!("CentralSource@Aura.base * {}", multiplier)).unwrap());
                }
            });
            app.update();

            // Pre-register the system that updates the central entity's stat
            let update_system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
                stat_accessor.add_modifier(central, "Aura.base", 1.0); // Increment aura
            });
            app.update(); // process registration

            let dependent_entities_for_eval = entities[1..].to_vec(); // Clone for benchmark loop

            b.iter(|| {
                // Run the system to update the central entity's aura
                let _ = black_box(app.world_mut().run_system(update_system_id));
                // add_modifier itself should propagate, an app.update() after this might not be strictly needed for stats
                // but can be kept if we want to simulate a frame tick for other Bevy systems.
                // For measuring just stat propagation, this app.update() could be omitted. Let's keep it for now.
                app.update(); 

                // Evaluate all dependent entities in one go to measure read-after-write performance
                let deps_clone = dependent_entities_for_eval.clone();
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    for &dep_entity in &deps_clone {
                        black_box(accessor.get(dep_entity, "Buff.value"));
                    }
                });
            });
        });
    }
    group.finish();
}

pub fn bench_complex_expression_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_expression_evaluation");
    
    let expressions = [
        "Base + Added",
        "Base * (1.0 + Increased)", // Use 1.0 for f32 consistency
        "Base * (1.0 + Increased) + Added",
        "min(Base * (1.0 + Increased) + Added, Cap)",
        "(Base * (1.0 + Increased) + Added) * (1.0 + More) - Taken"
    ];
    
    for (i, expr_str_ref) in expressions.iter().enumerate() {
        group.bench_with_input(BenchmarkId::from_parameter(i), expr_str_ref, |b, &expr_to_setup| {
            let (mut app, entity) = setup_app_for_bench();
            let el = entity; // copy
            let expr_owned = expr_to_setup.to_string(); // own for system

            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                stat_accessor.add_modifier(el, "Base", 100.0);
                stat_accessor.add_modifier(el, "Added", 50.0);
                stat_accessor.add_modifier(el, "Increased", 0.3);
                stat_accessor.add_modifier(el, "More", 0.2);
                stat_accessor.add_modifier(el, "Taken", 25.0);
                stat_accessor.add_modifier(el, "Cap", 200.0);
                stat_accessor.add_modifier(el, "Result", Expression::new(&expr_owned).unwrap());
            });
            app.update();
            
            b.iter(|| {
                let el_for_iter = el; // Copy entity
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(el_for_iter, "Result"));
                });
            });
        });
    }
    group.finish();
}

pub fn bench_many_modifiers_on_stat(c: &mut Criterion) {
    let mut group = c.benchmark_group("many_modifiers_on_stat");
    
    for &modifier_count in &[1, 10, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(modifier_count), &modifier_count, |b, &mc| {
            let (mut app, entity) = setup_app_for_bench();
            let el = entity; // copy
            let mc_val = mc; // copy

            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                for _ in 0..mc_val {
                    // Assuming "Power.base" is modifiable or "Power.sum_part" if flat and summed.
                    // If "Power" is default Flat, "Power.base" is just a name.
                    // For Modifiable, add_modifier("Power", val) adds to base.
                    // Let's use a Modifiable stat for this.
                    stat_accessor.add_modifier(el, "Power", 1.0); 
                }
            });
            // Ensure "Power" is Modifiable
            Konfig::register_stat_type("Power", "Modifiable");
            app.update();
            
            b.iter(|| {
                let el_for_iter = el; // Copy entity
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(el_for_iter, "Power")); // Evaluate the Modifiable stat "Power"
                });
            });
        });
    }
    group.finish();
}

pub fn bench_many_distinct_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("many_distinct_stats");
    
    for &stat_count in &[10, 50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(stat_count), &stat_count, |b, &sc| {
            let (mut app, entity) = setup_app_for_bench();
            let el = entity; // copy
            let sc_val = sc; // copy

            let _ = app.world_mut().run_system_once(move |mut stat_accessor: StatAccessor| {
                for i in 0..sc_val {
                    stat_accessor.add_modifier(el, &format!("Stat{}.value", i), i as f32);
                }
            });
            app.update();
            
            let target_stat_name = format!("Stat{}.value", sc / 2);
            b.iter(|| {
                let el_for_iter = el; // Copy entity
                let target_stat_name_for_iter = target_stat_name.clone(); // Clone String inside b.iter
                let _ = app.world_mut().run_system_once(move |accessor: StatAccessor| {
                    black_box(accessor.get(el_for_iter, &target_stat_name_for_iter));
                });
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_stat_access,
    bench_dependent_stats,
    bench_entity_dependencies,
    bench_tag_based_stats,
    bench_mixed_dependencies,
    bench_stats_update_propagation,
    bench_complex_expression_evaluation,
    bench_many_modifiers_on_stat,
    bench_many_distinct_stats
);
criterion_main!(benches);