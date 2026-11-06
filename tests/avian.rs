//! The `avian3d` feature's `Mass` sync.
#![cfg(feature = "avian3d")]

use avian3d::prelude::Mass;
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(AttributesPlugin);
    app
}

fn mass(app: &App, e: Entity) -> f32 {
    app.world().get::<Mass>(e).unwrap().0
}

#[test]
fn mass_without_a_mass_attribute_is_left_alone() {
    // Mass arrives before Attributes, so nothing seeds the attribute; the
    // sync must not read the missing attribute as 0 and zero the body.
    let mut app = test_app();
    let e = app.world_mut().spawn(Mass(10.0)).id();
    app.world_mut()
        .entity_mut(e)
        .insert(attributes! { "Strength" => 3.0 });
    app.update();
    app.update();
    assert_eq!(mass(&app, e), 10.0);
}
