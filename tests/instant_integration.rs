//! Integration tests for `evaluate_instant` / `apply_instant` with real ECS
//! entities. Ensures that cross-entity `@role` expressions resolve correctly.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(AttributesPlugin);
    app
}

/// Passive cached read - what `&Attributes` readers see after the system ran.
fn cached_value(app: &App, entity: Entity, name: &str) -> f32 {
    app.world()
        .entity(entity)
        .get::<Attributes>()
        .expect("entity has Attributes")
        .value(name)
}

#[test]
fn evaluate_instant_resolves_cross_entity_roles() {
    let mut app = test_app();

    let archer = app.world_mut().spawn(attributes! { "Agility" => 30.0 }).id();
    let target = app
        .world_mut()
        .spawn(attributes! {
            "Life" => 100.0,
            "Life.current" => 100.0,
        })
        .id();
    let arrow = app.world_mut().spawn(attributes! { "Damage" => 15.0 }).id();

    let (preview_len, damage, life) = app
        .world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            let on_hit = instant! {
                "Life.current" -= "Damage@arrow + Agility@attacker * 0.1",
            };
            let roles: &[(&str, Entity)] = &[("arrow", arrow), ("attacker", archer)];

            let preview = attributes.evaluate_instant(&on_hit, roles, target);
            let damage = preview.first().map(|e| e.value).unwrap_or(f32::NAN);
            attributes.apply_evaluated_instant(&preview, target);
            let life = attributes.evaluate(target, "Life.current");
            (preview.len(), damage, life)
        })
        .unwrap();

    assert_eq!(preview_len, 1);
    // 15 (arrow Damage) + 30 * 0.1 (attacker Agility) = 18.0
    assert!((damage - 18.0).abs() < 0.01, "expected 18.0 damage, got {damage}");
    assert!((life - 82.0).abs() < 0.01, "expected life 82.0, got {life}");
    assert!((cached_value(&app, target, "Life.current") - 82.0).abs() < 0.01);
}

#[test]
fn apply_instant_resolves_cross_entity_roles() {
    let mut app = test_app();

    let attacker = app.world_mut().spawn(attributes! { "Strength" => 20.0 }).id();
    let target = app
        .world_mut()
        .spawn(attributes! {
            "Life" => 100.0,
            "Life.current" => 100.0,
        })
        .id();

    let life = app
        .world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            let on_hit = instant! {
                "Life.current" -= "Strength@attacker",
            };
            let roles: &[(&str, Entity)] = &[("attacker", attacker)];
            attributes.apply_instant(&on_hit, roles, target);
            attributes.evaluate(target, "Life.current")
        })
        .unwrap();

    // 100 - 20 = 80
    assert!((life - 80.0).abs() < 0.01, "expected life 80.0, got {life}");
    assert!((cached_value(&app, target, "Life.current") - 80.0).abs() < 0.01);
}

#[test]
fn evaluate_instant_with_literal_values() {
    let mut app = test_app();

    let target = app
        .world_mut()
        .spawn(attributes! {
            "Life" => 100.0,
            "Life.current" => 100.0,
        })
        .id();

    let (preview_len, preview_value, life) = app
        .world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            let on_hit = instant! {
                "Life.current" -= 25.0,
            };
            let preview = attributes.evaluate_instant(&on_hit, &[], target);
            let value = preview.first().map(|e| e.value).unwrap_or(f32::NAN);
            attributes.apply_evaluated_instant(&preview, target);
            let life = attributes.evaluate(target, "Life.current");
            (preview.len(), value, life)
        })
        .unwrap();

    assert_eq!(preview_len, 1);
    assert!((preview_value - 25.0).abs() < 0.01);
    assert!((life - 75.0).abs() < 0.01, "expected life 75.0, got {life}");
    assert!((cached_value(&app, target, "Life.current") - 75.0).abs() < 0.01);
}

/// Instant `-=` rewrites the flat base. Modifiers added before the instant
/// must still be removable afterwards without wiping the base.
#[test]
fn instant_then_modifier_removal_keeps_base() {
    let mut app = test_app();

    let target = app
        .world_mut()
        .spawn(attributes! { "Life.current" => 100.0 })
        .id();

    let (after_buff, after_hit, after_unbuff) = app
        .world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier(target, "Life.current", 20.0);
            let after_buff = attributes.evaluate(target, "Life.current");

            let on_hit = instant! { "Life.current" -= 30.0 };
            attributes.apply_instant(&on_hit, &[], target);
            let after_hit = attributes.evaluate(target, "Life.current");

            attributes.remove_modifier(target, "Life.current", &Modifier::Flat(20.0));
            let after_unbuff = attributes.evaluate(target, "Life.current");
            (after_buff, after_hit, after_unbuff)
        })
        .unwrap();

    assert_eq!(after_buff, 120.0);
    assert_eq!(after_hit, 90.0);
    // 90 - 20 = 70, not 0.
    assert_eq!(after_unbuff, 70.0);
}
