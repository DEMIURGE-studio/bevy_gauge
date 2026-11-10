//! `TaggedAttribute` lets a `ModifierSet` declare a tag-filtered attribute, so
//! an entity can be fully initialised from data (e.g. via `AttributeInitializer`).

use bevy::prelude::*;
use bevy_gauge::prelude::*;

define_tags! {
    TestTags,
    fire,
    spell,
    attack,
}

const FIRE: TagMask = TestTags::FIRE;
const SPELL: TagMask = TestTags::SPELL;
const ATTACK: TagMask = TestTags::ATTACK;

#[derive(Resource)]
struct Mage(Entity);

#[test]
fn tagged_attribute_builder_declares_tag_filtered_attribute() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AttributesPlugin);

    app.add_systems(Startup, |mut commands: Commands| {
        let mut set = ModifierSet::new();
        set.add_builder(TaggedAttribute::new(
            "Damage",
            &[("added", ReduceFn::Sum), ("increased", ReduceFn::Sum), ("more", ReduceFn::Product)],
            "added * (1 + increased) * more",
        ));
        set.add("Damage.added", 10.0);
        set.add_tagged("Damage.increased", 0.5, FIRE);
        set.add_tagged("Damage.increased", 0.25, SPELL);
        set.add_tagged("Damage.more", 0.2, FIRE | SPELL);
        let mage = commands.spawn(AttributeInitializer::new(set)).id();
        commands.insert_resource(Mage(mage));
    });

    app.add_systems(Update, |mage: Res<Mage>, mut attributes: AttributesMut| {
        // Fire spell: 10 * (1 + 0.5 + 0.25) * 1.2 = 21
        let fire_spell = attributes.evaluate_tagged(mage.0, "Damage", FIRE | SPELL);
        assert!((fire_spell - 21.0).abs() < 1e-3, "fire spell damage {fire_spell}");

        // Fire attack: 10 * (1 + 0.5) * 1.0 = 15 (the spell-only mods don't apply)
        let fire_attack = attributes.evaluate_tagged(mage.0, "Damage", FIRE | ATTACK);
        assert!((fire_attack - 15.0).abs() < 1e-3, "fire attack damage {fire_attack}");

        // An empty "more" product is 1, not 0.
        let more = attributes.evaluate_tagged(mage.0, "Damage.more", ATTACK);
        assert!((more - 1.0).abs() < 1e-6, "empty more product {more}");
    });

    app.update();
    app.update();
}
