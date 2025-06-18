use bevy::{ecs::system::RunSystemOnce, prelude::*};
use bevy_gauge::prelude::*;

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy_gauge::plugin);
    app
}

fn setup_health_config() {
    Konfig::reset_for_test(); // Ensure clean state
    Konfig::register_stat_type("Life", "Complex");
    Konfig::register_total_expression("Life", "base + bonus - reduction");
}

fn setup_damage_config() {
    Konfig::reset_for_test(); // Ensure clean state
    Konfig::register_stat_type("Damage", "Complex");
    Konfig::register_total_expression("Damage", "base * (1.0 + increased) * more");
}

#[test]
fn test_basic_stats_initialization() {
    let mut app = setup_app();

    let mut initial_mods = ModifierSet::default();
    initial_mods.add("Life.base", 100.0);
    initial_mods.add("Mana.base", 50.0);

    let entity = app.world_mut().spawn((
        Stats::new(),
        StatsInitializer::new(initial_mods.clone()), // Clone since we might want to inspect it later
    )).id();

    app.update(); // This should trigger the OnAdd<StatsInitializer> observer

    // TODO generally access is done via a Stats query, and Stats::get()
    // Check if stats are applied
    let health_val = app.world_mut().run_system_once(
        move |stats_mutator: StatsMutator| stats_mutator.get(entity, "Life.base")
    ).unwrap();
    assert_eq!(health_val, 100.0, "Life.base should be initialized to 100.0");

    let mana_val = app.world_mut().run_system_once(
        move |stats_mutator: StatsMutator| stats_mutator.get(entity, "Mana.base")
    ).unwrap();
    assert_eq!(mana_val, 50.0, "Mana.base should be initialized to 50.0");

    // Check if StatsInitializer component is removed
    assert!(app.world().get::<StatsInitializer>(entity).is_none(), "StatsInitializer component should be removed after application");
}

#[test]
fn test_initialization_with_expressions() {
    let mut app = setup_app();

    // Configure a stat that uses an expression
    Konfig::register_stat_type("Power", "Complex"); // Assuming Flat allows expressions on .base or we define a part
    Konfig::register_total_expression("Power", "base + bonus");
    Konfig::register_stat_type("Stamina", "Flat");

    let mut initial_mods = ModifierSet::default();
    initial_mods.add("Stamina.base", 20.0);
    initial_mods.add("Power.base", 10.0);
    initial_mods.add("Power.bonus", Expression::new("Stamina.base * 2.0").unwrap());

    let entity = app.world_mut().spawn((
        Stats::new(),
        StatsInitializer::new(initial_mods),
    )).id();

    app.update();

    let power_val = app.world_mut().run_system_once(
        move |stats_mutator: StatsMutator| stats_mutator.get(entity, "Power")
    ).unwrap();
    // Expected: Power.base (10) + Power.bonus (Stamina.base (20) * 2.0 = 40) = 50
    assert_eq!(power_val, 50.0, "Power should be 10 (base) + 40 (bonus from Stamina) = 50.0");

    assert!(app.world().get::<StatsInitializer>(entity).is_none(), "StatsInitializer should be removed.");
}

#[test]
fn test_initializer_on_entity_without_stats_component_initially() {
    let mut app = setup_app();

    let mut initial_mods = ModifierSet::default();
    initial_mods.add("Agility.base", 30.0);

    // Spawn with initializer but add Stats component in the same update cycle AFTER commands are processed
    let entity = app.world_mut().spawn(StatsInitializer::new(initial_mods)).id();

    app.update();

    let agility_val = app.world_mut().run_system_once(
        move |stats_mutator: StatsMutator| stats_mutator.get(entity, "Agility.base")
    ).unwrap();
    assert_eq!(agility_val, 30.0, "Agility.base should be initialized to 30.0 even if Stats is added slightly after Initializer");

    assert!(app.world().get::<StatsInitializer>(entity).is_none(), "StatsInitializer should be removed.");
}

#[test]
fn test_initialization_with_source_dependency() {
    let mut app = setup_app();

    Konfig::register_stat_type("Strength", "Flat"); // Source stat
    Konfig::register_stat_type("AttackPower", "Complex"); // Target stat uses Complex
    // Total expression refers to local parts of AttackPower
    Konfig::register_total_expression("AttackPower", "base * bonus");

    // Source Entity
    let mut source_mods = ModifierSet::default();
    source_mods.add("Strength.base", 5.0);
    let source_entity = app.world_mut().spawn((
        Stats::new(),
        StatsInitializer::new(source_mods),
    )).id();

    // Target Entity
    let mut target_mods = ModifierSet::default();
    target_mods.add("AttackPower.base", 2.0); // Set the "base" part of AttackPower
    // Set the "bonus_from_strength" part of AttackPower to be an expression that uses the source
    target_mods.add("AttackPower.bonus", Expression::new("Strength@Source").unwrap());

    let target_entity = app.world_mut().spawn((Stats::new(), StatsInitializer::new(target_mods))).id();

    // Manually register source for target.
    app.world_mut().run_system_once(move |mut sa: StatsMutator| {
        sa.register_source(target_entity, "Source", source_entity);
    }).unwrap();

    app.update(); // Process initializers and source registration
    app.update(); // Ensure all updates and evaluations propagate

    let ap_val = app.world_mut().run_system_once(
        move |stats_mutator: StatsMutator| stats_mutator.get(target_entity, "AttackPower")
    ).unwrap();
    // Expected: AttackPower.base (2.0) * AttackPower.bonus_from_strength (Strength@Source (5.0)) = 10.0
    assert_eq!(ap_val, 10.0, "AttackPower should be 2.0 * 5.0 = 10.0");

    assert!(app.world().get::<StatsInitializer>(source_entity).is_none(), "Source StatsInitializer should be removed.");
    assert!(app.world().get::<StatsInitializer>(target_entity).is_none(), "Target StatsInitializer should be removed.");
}

#[test]
fn test_stats_initializer_applies_modifiers_correctly() {
    // ... existing code ...
}

#[test]
fn test_stats_initializer_complex_stat_application() {
    // ... existing code ...
} 