use bevy_gauge::stat_modifiers::Stats;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use bevy::prelude::*;
use bevy::ecs::component::Component;
use std::time::Duration;
use rand::prelude::*;

pub mod damage {
    pub const FIRE: u32 = 1 << 0u32; 
    pub const COLD: u32 = 1 << 1u32;
    pub const LIGHTNING: u32 = 1 << 2u32;
    pub const ELEMENTAL: u32 = 1 << 0u32 | 1 << 1u32 | 1 << 2u32;
    pub const PHYSICAL: u32 = 1 << 3u32;
    pub const CHAOS: u32 = 1 << 4u32;
    pub const DAMAGE_TYPE: u32 = 1 << 0u32 | 1 << 1u32 | 1 << 2u32 | 1 << 3u32 | 1 << 4u32;
    pub const SWORD: u32 = 1 << 5u32;
    pub const AXE: u32 = 1 << 6u32;
    pub const MELEE: u32 = 1 << 5u32 | 1 << 6u32;
    pub const BOW: u32 = 1 << 7u32;
    pub const WAND: u32 = 1 << 8u32;
    pub const RANGED: u32 = 1 << 7u32 | 1 << 8u32;
    pub const WEAPON_TYPE: u32 = 1 << 5u32 | 1 << 6u32 | 1 << 7u32 | 1 << 8u32;
    pub const DAMAGE: u32 = 1 << 0u32
        | 1 << 1u32
        | 1 << 2u32
        | 1 << 3u32
        | 1 << 4u32
        | 1 << 5u32
        | 1 << 6u32
        | 1 << 7u32
        | 1 << 8u32;
    pub fn match_tag(tag: &str) -> u32 {
        match tag {
            "fire" => FIRE,
            "cold" => COLD,
            "lightning" => LIGHTNING,
            "elemental" => ELEMENTAL,
            "physical" => PHYSICAL,
            "chaos" => CHAOS,
            "damage_type" => DAMAGE_TYPE,
            "sword" => SWORD,
            "axe" => AXE,
            "melee" => MELEE,
            "bow" => BOW,
            "wand" => WAND,
            "ranged" => RANGED,
            "weapon_type" => WEAPON_TYPE,
            "damage" => DAMAGE,
            _ => 0,
        }
    }
}

// Simple Bevy component for comparison
#[derive(Component)]
struct SimpleStat(f32);

fn benchmark_group(c: &mut Criterion) {
    let mut group = c.benchmark_group("Stat System");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.warm_up_time(Duration::from_secs(1));

    // Test insertion speeds
    group.bench_function("insert_simple_stat", |b| {
        b.iter(|| {
            let mut stats = Stats::default();
            stats.add_modifier("Speed", black_box(10.0));
        })
    });

    group.bench_function("insert_modifiable_stat", |b| {
        b.iter(|| {
            let mut stats = Stats::default();
            stats.add_modifier("Damage_Added", black_box(10.0));
            stats.add_modifier("Damage_Increased", black_box(1.1));
        })
    });

    group.bench_function("insert_complex_stat", |b| {
        b.iter(|| {
            let mut stats = Stats::default();
            stats.add_modifier(
                &format!("Damage_Added_{}", damage::FIRE | damage::SWORD),
                black_box(5.0)
            );
        })
    });

    group.bench_function("insert_expression_stat", |b| {
        b.iter(|| {
            let mut stats = Stats::default();
            stats.add_modifier("Damage_More", black_box("Damage_Added * 0.1"));
        })
    });

    // Test removal speeds
    group.bench_function("remove_simple_stat", |b| {
        let mut stats = Stats::default();
        stats.add_modifier("Speed", 10.0);
        b.iter(|| {
            stats.remove_modifier("Speed", black_box(5.0));
        })
    });

    group.bench_function("remove_modifiable_stat", |b| {
        let mut stats = Stats::default();
        stats.add_modifier("Damage_Added", 10.0);
        stats.add_modifier("Damage_Increased", 1.1);
        b.iter(|| {
            stats.remove_modifier("Damage_Added", black_box(5.0));
        })
    });

    group.bench_function("remove_complex_stat", |b| {
        let mut stats = Stats::default();
        stats.add_modifier(
            &format!("Damage_Added_{}", damage::FIRE | damage::SWORD),
            5.0
        );
        b.iter(|| {
            stats.remove_modifier(
                &format!("Damage_Added_{}", damage::FIRE | damage::SWORD),
                black_box(2.0)
            );
        })
    });

    // Test evaluation speeds
    group.bench_function("evaluate_simple_stat", |b| {
        let mut stats = Stats::default();
        stats.add_modifier("Speed", 10.0);
        b.iter(|| {
            black_box(stats.evaluate("Speed"));
        })
    });

    group.bench_function("evaluate_modifiable_stat", |b| {
        let mut stats = Stats::default();
        stats.add_modifier("Damage_Added", 10.0);
        stats.add_modifier("Damage_Increased", 1.1);
        stats.add_modifier("Damage_More", 1.05);
        b.iter(|| {
            black_box(stats.evaluate("Damage"));
        })
    });

    group.bench_function("evaluate_complex_stat", |b| {
        let mut stats = Stats::default();
        stats.add_modifier(
            &format!("Damage_Added_{}", damage::FIRE | damage::SWORD),
            5.0
        );
        stats.add_modifier(
            &format!("Damage_Increased_{}", damage::FIRE | damage::SWORD),
            1.2
        );
        b.iter(|| {
            black_box(stats.evaluate(&format!("Damage_{}", damage::FIRE | damage::SWORD)));
        })
    });

    group.bench_function("evaluate_expression_stat", |b| {
        let mut stats = Stats::default();
        stats.add_modifier("BaseDamage", 10.0);
        stats.add_modifier("Damage_Added", "BaseDamage * 0.1 + 1.0");
        b.iter(|| {
            black_box(stats.evaluate("Damage"));
        })
    });

    // Test with varying numbers of modifiers
    for count in [10, 100, 1000].iter() {
        group.bench_with_input(
            &format!("evaluate_with_{}_modifiers", count),
            count,
            |b, &count| {
                let mut stats = Stats::default();
                stats.add_modifier("Base", 10.0);
                
                let mut rng = rand::rng();
                for i in 0..count {
                    let modifier_type = if rng.random_bool(0.5) { "Added" } else { "Increased" };
                    let value: f32 = rng.random_range(0.5..2.0);
                    stats.add_modifier(format!("Damage_{}_{}", modifier_type, i), value);
                }
                
                b.iter(|| {
                    black_box(stats.evaluate("Damage"));
                })
            },
        );
    }

    // Compare with simple Bevy component access
    group.bench_function("bevy_component_access", |b| {
        let mut world = World::new();
        world.spawn(SimpleStat(10.0));
        
        let mut query = world.query::<&SimpleStat>();
        b.iter(|| {
            for stat in query.iter(&world) {
                black_box(stat.0);
            }
        })
    });

    group.bench_function("evaluate_expression_dependencies", |b| {
        let mut stats = Stats::default();
        
        // Setup base stats
        stats.add_modifier("Strength", 50.0); // Simple stat
        stats.add_modifier("Intelligence", 30.0); // Simple stat
        
        // Life with expression-based modifiers
        stats.add_modifier("Life_Added", "Strength / 5"); // 50/5 = 10
        stats.add_modifier("Life_Increased", 1); // 30/30 = 1.0 (100% increase)
        stats.add_modifier("Life_Increased", "Intelligence / 30"); // 30/30 = 1.0 (100% increase)
        stats.add_modifier("Life_More", 1); // 1.0 + 0.5 = 1.5
        stats.add_modifier("Life_More", "Strength / 100"); // 1.0 + 0.5 = 1.5
        
        // Lightning Damage that depends on Life
        stats.add_modifier("Damage_Added", "Life * 0.2"); // Depends on full Life calculation
        stats.add_modifier("Damage_Increased", 1); // 1.0 + 0.5 = 1.5
        stats.add_modifier("Damage_Increased", "Intelligence / 60"); // 1.0 + 0.5 = 1.5
        
        // Expected calculations:
        // Life = (Strength/5) * (1 + Intelligence/30) * (1.0 + Strength/100)
        //       = 10 * 2.0 * 1.5 = 30
        // LightningDamage = (Life * 0.2) * (1.0 + Intelligence/60)
        //                 = 6 * 1.5 = 9
        
        b.iter_batched(
            || &stats, // Clone/reuse the prepared stats
            |s| {
                for _ in 0..100 {
                    black_box(s.evaluate("Damage"));
                }
            },
            BatchSize::PerIteration // Treat the whole 100 as one "iteration"
        );
    }); 

    group.bench_function("evaluate_complex_expression_dependencies", |b| {
        let mut stats = Stats::default();
        
        // Setup base stats
        stats.add_modifier("Strength", 50.0); // Simple stat
        stats.add_modifier("Intelligence", 30.0); // Simple stat
        
        // Life with expression-based modifiers
        stats.add_modifier("Life_Added", "Strength / 5"); // 50/5 = 10
        stats.add_modifier("Life_Increased", 1); // 30/30 = 1.0 (100% increase)
        stats.add_modifier("Life_Increased", "Intelligence / 30"); // 30/30 = 1.0 (100% increase)
        stats.add_modifier("Life_More", 1); // 1.0 + 0.5 = 1.5
        stats.add_modifier("Life_More", "Strength / 100"); // 1.0 + 0.5 = 1.5
        
        // Lightning Damage that depends on Life
        stats.add_modifier("Damage_Added_1", "Life * 0.2"); // Depends on full Life calculation
        stats.add_modifier("Damage_Increased_1", 1); // 1.0 + 0.5 = 1.5
        stats.add_modifier("Damage_Increased_1", "Intelligence / 60"); // 1.0 + 0.5 = 1.5
        
        // Expected calculations:
        // Life = (Strength/5) * (1 + Intelligence/30) * (1.0 + Strength/100)
        //       = 10 * 2.0 * 1.5 = 30
        // LightningDamage = (Life * 0.2) * (1.0 + Intelligence/60)
        //                 = 6 * 1.5 = 9
        
        b.iter_batched(
            || &stats, // Clone/reuse the prepared stats
            |s| {
                for _ in 0..100 {
                    black_box(s.evaluate("Damage_1"));
                }
            },
            BatchSize::PerIteration // Treat the whole 100 as one "iteration"
        );
    });

    // const ITERS: u32 = 10000;

    // #[derive(Event)]
    // struct AddTrigger(f32);

    // #[derive(Component)]
    // struct Accumulator(f32);

    // fn trigger_system(mut commands: Commands, query: Query<Entity, With<Accumulator>>) {
    //     let target = query.get_single().unwrap();
    //     for _ in 0..ITERS {
    //         commands.trigger_targets(AddTrigger(5.0), target);
    //     }
    // }

    // fn listen_system(trigger: Trigger<AddTrigger>, mut query: Query<&mut Accumulator>) {
    //     let entity = trigger.entity();
    //     let Ok(mut accumulator) = query.get_mut(entity) else {
    //         return;
    //     };
    //     accumulator.0 += trigger.0;
    // }

    // group.bench_function("observer_speed", |b| {
    //     b.iter_batched(
    //         || {
    //             let mut binding = App::new();
    //             let app = binding
    //                 .add_systems(Update, trigger_system)
    //                 .add_observer(listen_system);

    //             app
    //                 .world_mut()
    //                 .spawn(Accumulator(0.0));

    //             binding
    //         }, // Clone/reuse the prepared stats
    //         |mut app| {
    //             for _ in 0..100 {
    //                 app.update();
    //             }
    //         },
    //         BatchSize::PerIteration // Treat the whole 100 as one "iteration"
    //     );
    // });

    // group.bench_function("observe_entity_speed", |b| {
    //     b.iter_batched(
    //         || {
    //             let mut binding = App::new();
    //             let app = binding
    //                 .add_systems(Update, trigger_system);

    //             app
    //                 .world_mut()
    //                 .spawn(Accumulator(0.0))
    //                 .observe(listen_system);

    //             binding
    //         }, // Clone/reuse the prepared stats
    //         |mut app| {
    //             for _ in 0..100 {
    //                 app.update();
    //             }
    //         },
    //         BatchSize::PerIteration // Treat the whole 100 as one "iteration"
    //     );
    // });

    // fn direct_update(mut query: Query<&mut Accumulator>) {
    //     let Ok(mut accumulator) = query.get_single_mut() else {
    //         return;
    //     };
    //     for _ in 0..ITERS {
    //         accumulator.0 += 5.0;
    //     }
    // }

    // group.bench_function("direct_access_speed", |b| {
    //     b.iter_batched(
    //         || {
    //             let mut binding = App::new();
    //             let app = binding
    //                 .add_systems(Update, direct_update);

    //             app
    //                 .world_mut()
    //                 .spawn(Accumulator(0.0));

    //             binding
    //         }, // Clone/reuse the prepared stats
    //         |mut app| {
    //             for _ in 0..100 {
    //                 app.update();
    //             }
    //         },
    //         BatchSize::PerIteration // Treat the whole 100 as one "iteration"
    //     );
    // });

    group.finish();
}

// ```
// Test                     | ITER  | time 
// observer_speed           | 10000 | 46.8ms
//                          | 1000  | 6.2ms
//                          | 100   | 2.0ms
//                          | 10    | 1.5ms
//                          | 1     | 1.5ms
// observer_entity_speed    | 10000 | 52.2ms
//                          | 1000  | 6.9ms
//                          | 100   | 2.2ms
//                          | 10    | 1.6ms
//                          | 1     | 1.5ms
// direct_access            | 10000 | 3.7ms
//                          | 1000  | 1.9ms
//                          | 100   | 1.5ms
//                          | 10    | 1.5ms
//                          | 1     | 1.5ms
// ```
criterion_group!(benches, benchmark_group);
criterion_main!(benches);