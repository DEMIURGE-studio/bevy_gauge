use bevy::prelude::*;
use bevy_gauge::prelude::*;
use serial_test::serial;

// State-specific hierarchy relationships
#[derive(Component, Default, Debug, PartialEq, Eq, Reflect)]
#[relationship_target(relationship = TestEdge, linked_spawn)]
#[reflect(Component, FromWorld, Default)]
pub struct TestEdgeTarget(Vec<Entity>);

impl<'a> IntoIterator for &'a TestEdgeTarget {
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = std::slice::Iter<'a, Entity>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl TestEdgeTarget {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

#[derive(Component, Clone, PartialEq, Eq, Debug, Reflect)]
#[relationship(relationship_target = TestEdgeTarget)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone)]
pub struct TestEdge(#[entities] pub Entity);

impl FromWorld for TestEdge {
    #[inline(always)]
    fn from_world(_world: &mut World) -> Self {
        TestEdge(Entity::PLACEHOLDER)
    }
}

#[test]
#[serial]
fn test_register_and_evaluate_via_relationship() {
    Konfig::reset_for_test();
    Konfig::set_stat_type_default("Modifiable");
    Konfig::register_stat_type("Strength", "Modifiable");
    Konfig::register_stat_type("ChildBonus", "Modifiable");

    let mut app = App::new();
    app.add_plugins(bevy_gauge::plugin);
    // Bring extension methods into scope
    use bevy_gauge::prelude::StatsAppSourcesExt as _;
    app.register_stat_relationship::<TestEdge>();

    // Spawn source with Strength = 50
    let source = app.world_mut().spawn((
        Stats::new(),
        stats! { "Strength" => 50.0 },
        Name::new("Source"),
    )).id();

    // Spawn target with ChildBonus expression referencing Strength@TestEdge
    let target = app.world_mut().spawn((
        Stats::new(),
        stats! { "ChildBonus" => "Strength@TestEdge * 0.1" },
        Name::new("Target"),
    )).id();

    app.update();
    // Attach relationship edge
    app.world_mut().entity_mut(target).insert(TestEdge(source));
    app.update();

    // Verify evaluation uses source
    let stats = app.world().get::<Stats>(target).unwrap();
    assert_eq!(stats.get("ChildBonus"), 5.0);

    // Remove edge and verify it unregisters
    app.world_mut().entity_mut(target).remove::<TestEdge>();
    app.update();
    let stats = app.world().get::<Stats>(target).unwrap();
    assert_eq!(stats.get("ChildBonus"), 0.0);
}

#[test]
#[serial]
fn test_register_with_custom_alias_extractor() {
    Konfig::reset_for_test();
    Konfig::set_stat_type_default("Modifiable");
    Konfig::register_stat_type("Strength", "Modifiable");
    Konfig::register_stat_type("Buff", "Modifiable");

    let mut app = App::new();
    app.add_plugins(bevy_gauge::plugin);
    use bevy_gauge::prelude::StatsAppSourcesExt as _;
    app.register_stat_relationship_as_with::<TestEdge>("Rel", |e: &TestEdge| e.0);

    let src = app.world_mut().spawn((
        Stats::new(),
        stats! { "Strength" => 100.0 },
        Name::new("Src"),
    )).id();

    let dst = app.world_mut().spawn((
        Stats::new(),
        stats! { "Buff" => "Strength@Rel * 0.2" },
        Name::new("Dst"),
    )).id();

    app.update();
    app.world_mut().entity_mut(dst).insert(TestEdge(src));
    app.update();

    let stats = app.world().get::<Stats>(dst).unwrap();
    assert_eq!(stats.get("Buff"), 20.0);
}

