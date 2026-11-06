//! The derived-component bridge: `#[derive(AttributeComponent)]` with
//! `#[read]`, `#[write]`, `#[init_from]` and `#[init_to]`, run through the
//! `PreUpdate`/`PostUpdate` sync, spawn-time init ordering, and
//! `add_gauge_sync_to_schedule`.

use bevy::ecs::schedule::ScheduleLabel;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gauge::prelude::*;

/// `max` follows `MaxHealth`; `current` starts at `MaxHealth` and is written
/// back to `Health.current`.
#[derive(Component, Default, Debug, AttributeComponent)]
struct Health {
    #[read("MaxHealth")]
    max: f32,
    #[write]
    #[init_from("MaxHealth")]
    current: f32,
}

/// Seeds `Speed` from the component at spawn, then follows it. `drag` is an
/// `f64` going both ways.
#[derive(Component, Default, Debug, AttributeComponent)]
struct Mover {
    #[read("Speed")]
    #[init_to]
    speed: f32,
    #[read("Drag")]
    #[init_to]
    drag: f64,
    #[write("Heading")]
    heading: f64,
}

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(AttributesPlugin);
    app
}

fn value(app: &App, entity: Entity, name: &str) -> f32 {
    app.world().get::<Attributes>(entity).unwrap().value(name)
}

fn set_base(app: &mut App, entity: Entity, name: &'static str, v: f32) {
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set_base(entity, name, v);
        })
        .unwrap();
}

#[test]
fn f64_fields_read_write_and_seed() {
    let mut app = test_app();
    let e = app
        .world_mut()
        .spawn((Mover { drag: 0.25, ..default() }, Attributes::new()))
        .id();
    assert_eq!(value(&app, e, "Drag"), 0.25);

    set_base(&mut app, e, "Drag", 0.5);
    app.world_mut().get_mut::<Mover>(e).unwrap().heading = 90.0;
    app.update();
    assert_eq!(app.world().get::<Mover>(e).unwrap().drag, 0.5);
    assert_eq!(value(&app, e, "Heading"), 90.0);
}

#[test]
fn init_to_wins_over_an_initializer_in_the_same_bundle() {
    // Whichever observer Bevy runs first, the initializer is applied before
    // the component seeds, exactly once: Speed is the component's 7 (not 3,
    // and not 3 + 7), and Other is 5 (not 10).
    let mut app = test_app();
    let a = app
        .world_mut()
        .spawn((Mover { speed: 7.0, ..default() }, attributes! { "Speed" => 3.0, "Other" => 5.0 }))
        .id();
    let b = app
        .world_mut()
        .spawn((attributes! { "Speed" => 3.0, "Other" => 5.0 }, Mover { speed: 7.0, ..default() }))
        .id();
    app.update();
    app.update();
    for e in [a, b] {
        assert_eq!(value(&app, e, "Speed"), 7.0);
        assert_eq!(value(&app, e, "Other"), 5.0);
        assert_eq!(app.world().get::<Mover>(e).unwrap().speed, 7.0);
        assert!(app.world().get::<AttributeInitializer>(e).is_none());
    }
}

#[test]
fn init_from_waits_for_attributes_added_later() {
    let mut app = test_app();
    let e = app.world_mut().spawn(Health::default()).id();
    app.update();
    assert_eq!(app.world().get::<Health>(e).unwrap().current, 0.0);

    app.world_mut()
        .entity_mut(e)
        .insert(attributes! { "MaxHealth" => 80.0 });
    app.update();
    let health = app.world().get::<Health>(e).unwrap();
    assert_eq!((health.max, health.current), (80.0, 80.0));
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct SimTick;

#[test]
fn gauge_sync_runs_in_a_custom_schedule() {
    let mut app = test_app();
    app.init_schedule(SimTick);
    add_gauge_sync_to_schedule(&mut app, SimTick);
    let e = app
        .world_mut()
        .spawn((Health::default(), attributes! { "MaxHealth" => 100.0 }))
        .id();
    app.update();

    set_base(&mut app, e, "MaxHealth", 120.0);
    app.world_mut().get_mut::<Health>(e).unwrap().current = 30.0;
    // Only the custom schedule runs: both directions sync inside it.
    app.world_mut().run_schedule(SimTick);
    assert_eq!(app.world().get::<Health>(e).unwrap().max, 120.0);
    assert_eq!(value(&app, e, "Health.current"), 30.0);
}
