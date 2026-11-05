//! `#[derive(AttributeResolvable)]`: f64 and full-path primitive fields,
//! enums, generics, and the per-prefix path cache.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AttributesPlugin);
    app.update();
    app
}

/// Spawn an entity with `values` set as flat bases, and return its `Attributes`.
fn attrs_with(values: &'static [(&'static str, f32)]) -> (App, Entity) {
    let mut app = test_app();
    let e = app.world_mut().spawn(Attributes::new()).id();
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            for (name, value) in values {
                attributes.set_base(e, name, *value);
            }
        })
        .unwrap();
    (app, e)
}

#[derive(AttributeResolvable, Default, Debug, PartialEq)]
struct Inner {
    radius: f32,
    count: u32,
}

#[derive(AttributeResolvable, Default, Debug, PartialEq)]
struct Outer {
    speed: f64,
    enabled: bool,
    full_path: std::primitive::f32,
    inner: Inner,
    #[skip]
    untouched: f32,
}

#[test]
fn struct_fields_resolve_by_path() {
    let (app, e) = attrs_with(&[
        ("Outer.speed", 2.5),
        ("Outer.enabled", 1.0),
        ("Outer.full_path", 7.0),
        ("Outer.inner.radius", 3.0),
        ("Outer.inner.count", 4.6),
    ]);
    let attrs = app.world().get::<Attributes>(e).unwrap();

    let mut value = Outer { untouched: 9.0, ..default() };
    assert!(value.should_resolve("Outer", attrs));
    value.resolve("Outer", attrs);
    assert_eq!(
        value,
        Outer {
            speed: 2.5,
            enabled: true,
            full_path: 7.0,
            inner: Inner { radius: 3.0, count: 5 },
            untouched: 9.0,
        }
    );
    assert!(!value.should_resolve("Outer", attrs), "resolved value is up to date");
}

#[test]
fn one_type_resolves_under_several_prefixes() {
    // The path cache is keyed by prefix; two prefixes must not share entries.
    let (app, e) = attrs_with(&[
        ("A.radius", 1.0),
        ("A.count", 2.0),
        ("B.radius", 10.0),
        ("B.count", 20.0),
    ]);
    let attrs = app.world().get::<Attributes>(e).unwrap();

    let mut a = Inner::default();
    let mut b = Inner::default();
    for _ in 0..2 {
        a.resolve("A", attrs);
        b.resolve("B", attrs);
    }
    assert_eq!(a, Inner { radius: 1.0, count: 2 });
    assert_eq!(b, Inner { radius: 10.0, count: 20 });
}

#[derive(AttributeResolvable, Debug, PartialEq)]
enum Shape {
    Circle { radius: f32 },
    Rect { width: f32, height: f32, #[skip] label: u8 },
    Scaled(f32),
    Empty,
}

#[test]
fn enum_variants_resolve_their_own_fields() {
    let (app, e) = attrs_with(&[
        ("Circle", 2.0),
        ("Rect.width", 3.0),
        ("Rect.height", 4.0),
        ("Scaled", 1.5),
    ]);
    let attrs = app.world().get::<Attributes>(e).unwrap();

    let mut circle = Shape::Circle { radius: 0.0 };
    circle.resolve("Circle", attrs);
    assert_eq!(circle, Shape::Circle { radius: 2.0 });

    let mut rect = Shape::Rect { width: 0.0, height: 0.0, label: 7 };
    rect.resolve("Rect", attrs);
    assert_eq!(rect, Shape::Rect { width: 3.0, height: 4.0, label: 7 });

    let mut scaled = Shape::Scaled(0.0);
    scaled.resolve("Scaled", attrs);
    assert_eq!(scaled, Shape::Scaled(1.5));

    let mut empty = Shape::Empty;
    assert!(!empty.should_resolve("Empty", attrs));
    empty.resolve("Empty", attrs);
}

#[derive(AttributeResolvable, Default, Debug, PartialEq)]
struct Wrapper<T: AttributeResolvable> {
    value: T,
    scale: f32,
}

#[test]
fn generic_structs_derive() {
    let (app, e) = attrs_with(&[("W.value", 6.0), ("W.scale", 0.5)]);
    let attrs = app.world().get::<Attributes>(e).unwrap();

    let mut wrapped = Wrapper::<u16>::default();
    wrapped.resolve("W", attrs);
    assert_eq!(wrapped, Wrapper { value: 6, scale: 0.5 });
}

