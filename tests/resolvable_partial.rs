//! Resolving a composite from attributes only touches fields whose attribute
//! exists; the rest keep their authored values.

use bevy::prelude::*;
use bevy_gauge::prelude::*;

#[derive(Clone, Debug, PartialEq, AttributeResolvable)]
struct Gatherer {
    radius: f32,
    count: usize,
    enabled: bool,
}

#[derive(Resource)]
struct Target(Entity);

#[test]
fn unset_fields_keep_their_values() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AttributesPlugin);
    app.add_systems(Startup, |mut commands: Commands| {
        let e = commands.spawn(attributes! { "Gatherer.count" => 3.0 }).id();
        commands.insert_resource(Target(e));
    });
    app.add_systems(Update, |target: Res<Target>, q: Query<&Attributes>| {
        let attrs = q.get(target.0).unwrap();
        let mut g = Gatherer { radius: 2.5, count: 1, enabled: true };
        assert!(g.should_resolve("Gatherer", attrs));
        g.resolve("Gatherer", attrs);
        assert_eq!(g, Gatherer { radius: 2.5, count: 3, enabled: true });

        // Nothing left to change: no spurious re-resolves from unset fields.
        assert!(!g.should_resolve("Gatherer", attrs));

        // A plain terminal with no attribute is left alone too.
        let mut r = 7.0f32;
        assert!(!r.should_resolve("Missing", attrs));
        r.resolve("Missing", attrs);
        assert_eq!(r, 7.0);
    });
    app.update();
    app.update();
}
