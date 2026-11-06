//! All-or-nothing `ModifierSet::try_apply`, and the system state that
//! `commands.entity(..).attrs(..)` reuses across commands.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(AttributesPlugin);
    app
}

fn value(app: &App, entity: Entity, name: &str) -> f32 {
    app.world().get::<Attributes>(entity).unwrap().value(name)
}

/// Run `f` once as a system with `AttributesMut`.
fn with_attrs<R: Send + 'static>(
    app: &mut App,
    f: impl FnOnce(&mut AttributesMut) -> R + Send + Sync + 'static,
) -> R {
    let mut f = Some(f);
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| (f.take().unwrap())(&mut attributes))
        .unwrap()
}

// ---------------------------------------------------------------------------
// ModifierSet::remove / try_remove
// ---------------------------------------------------------------------------

#[test]
fn try_apply_changes_nothing_when_an_expression_fails() {
    let mut app = test_app();
    let e = app.world_mut().spawn(Attributes::new()).id();
    let broken = mod_set! {
        "A" => 1.0,
        "B" => "Strength *",
        "C" => 2.0,
    };

    let result = with_attrs(&mut app, move |a| broken.try_apply(e, a));
    assert!(result.is_err());
    assert_eq!(value(&app, e, "A"), 0.0, "entries before the bad one are not applied");
    assert_eq!(value(&app, e, "C"), 0.0);
}

// ---------------------------------------------------------------------------
// commands.entity(..).attrs(..)
// ---------------------------------------------------------------------------

#[test]
fn attrs_commands_on_several_entities_and_frames() {
    // The cached system state is reused across commands and frames, and has
    // to see entities spawned after it was built.
    let mut app = test_app();
    for round in 0..3 {
        let entities: Vec<Entity> = (0..3)
            .map(|_| app.world_mut().spawn(Attributes::new()).id())
            .collect();
        for (i, &e) in entities.iter().enumerate() {
            let v = (round * 10 + i) as f32;
            app.world_mut()
                .commands()
                .entity(e)
                .attrs(move |attrs| attrs.set_base("X", v));
        }
        app.update();
        for (i, &e) in entities.iter().enumerate() {
            assert_eq!(value(&app, e, "X"), (round * 10 + i) as f32);
        }
    }
}

