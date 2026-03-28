//! # Custom Extensions Example - Typed API + Custom AttributeBuilder
//!
//! Demonstrates how to build game-specific APIs on top of bevy_gauge:
//!
//! - **`DamagePipeline`** - a custom [`AttributeBuilder`] that sets up a tagged
//!   damage attribute with `added`, `increased`, `more` parts. Can be used in
//!   `attributes!` via `@build` or added programmatically.
//! - **`DamageExt`** on `Attributes` - `.damage(tags)` reads evaluated damage
//! - **`DamageMutExt`** on `AttributesMut` - `.add_damage()`, `.evaluate_damage()`
//! - The typed layer composes cleanly with the underlying string-key system
//!
//! Run with: `cargo run --example custom_extensions`

use bevy::prelude::*;
use bevy_gauge::prelude::*;

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

define_tags! {
    Tags,
    damage_type {
        elemental { fire, cold },
        physical,
    },
    weapon_type {
        melee { sword, axe },
        ranged { bow },
    },
}

// ---------------------------------------------------------------------------
// DamagePipeline - custom AttributeBuilder
// ---------------------------------------------------------------------------

/// Sets up a tagged damage attribute with `added * (1 + increased) * more`.
///
/// This is a custom [`AttributeBuilder`] that can be used in `attributes!`
/// via `@build` or added programmatically via `ModifierSet::add_builder()`.
#[derive(Clone, Debug)]
struct DamagePipeline;

impl AttributeBuilder for DamagePipeline {
    fn apply(&self, entity: Entity, attributes: &mut AttributesMut) {
        let _ = attributes.tagged_attribute(
            entity,
            "Damage",
            &[
                ("added", ReduceFn::Sum),
                ("increased", ReduceFn::Sum),
                ("more", ReduceFn::Product),
            ],
            "added * (1 + increased) * more",
        );
    }

    fn clone_box(&self) -> Box<dyn AttributeBuilder> {
        Box::new(self.clone())
    }

    fn fmt_debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DamagePipeline")
    }
}

// ---------------------------------------------------------------------------
// Extension trait: reading damage
// ---------------------------------------------------------------------------

trait DamageExt {
    fn damage(&self, tags: TagMask) -> f32;
    fn damage_added(&self, tags: TagMask) -> f32;
    fn damage_increased(&self, tags: TagMask) -> f32;
    fn damage_more(&self, tags: TagMask) -> f32;
}

impl DamageExt for Attributes {
    fn damage(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage", tags)
    }

    fn damage_added(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage.added", tags)
    }

    fn damage_increased(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage.increased", tags)
    }

    fn damage_more(&self, tags: TagMask) -> f32 {
        self.value_tagged("Damage.more", tags)
    }
}

// ---------------------------------------------------------------------------
// Extension trait: mutating damage
// ---------------------------------------------------------------------------

trait DamageMutExt {
    fn evaluate_damage(&mut self, entity: Entity, tags: TagMask) -> f32;
}

impl<F: bevy::ecs::query::QueryFilter> DamageMutExt for AttributesMut<'_, '_, F> {
    fn evaluate_damage(&mut self, entity: Entity, tags: TagMask) -> f32 {
        self.evaluate_tagged(entity, "Damage", tags)
    }
}

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Entities {
    sword: Entity,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(AttributesPlugin)
        .add_systems(
            Startup,
            (
                register_tags,
                spawn_and_configure.after(register_tags),
                demo.after(spawn_and_configure),
            ),
        )
        .run();
}

fn register_tags(mut resolver: ResMut<TagResolver>) {
    Tags::register(&mut resolver);
    println!("--- Tags registered ---\n");
}

// ---------------------------------------------------------------------------
// Setup: create a sword with the damage pipeline wired up at spawn
// ---------------------------------------------------------------------------

fn spawn_and_configure(mut commands: Commands) {
    let sword = commands
        .spawn((
            Name::new("Flaming Greatsword"),
            // DamagePipeline builder sets up the tagged attribute structure
            attributes! {
                @build DamagePipeline,
            },
        ))
        .id();

    // Add damage modifiers via the typed API (after commands flush,
    // the builder will have run - but we can also add modifiers now
    // since the AttributeInitializer observer fires immediately)
    commands.entity(sword).attrs(|attrs| {
        attrs.add_modifier_tagged("Damage.added", Modifier::Flat(100.0), Tags::PHYSICAL);
        attrs.add_modifier_tagged("Damage.added", Modifier::Flat(30.0), Tags::FIRE);
        attrs.add_modifier_tagged("Damage.added", Modifier::Flat(5.0), Tags::SWORD);
        attrs.add_modifier_tagged("Damage.increased", Modifier::Flat(0.50), Tags::PHYSICAL);
        attrs.add_modifier_tagged("Damage.increased", Modifier::Flat(0.25), Tags::FIRE);
        attrs.add_modifier_tagged("Damage.more", Modifier::Flat(0.20), Tags::PHYSICAL);
    });

    commands.insert_resource(Entities { sword });
    println!("--- Sword spawned with DamagePipeline builder ---\n");
}

// ---------------------------------------------------------------------------
// Demo: read damage using both typed and string-key APIs
// ---------------------------------------------------------------------------

fn demo(
    handles: Res<Entities>,
    mut attributes: AttributesMut,
) {
    let sword = handles.sword;

    // --- Read via typed API on AttributesMut ---
    let phys_sword = attributes.evaluate_damage(sword, Tags::PHYSICAL | Tags::SWORD);
    let fire_sword = attributes.evaluate_damage(sword, Tags::FIRE | Tags::SWORD);

    println!("=== Damage via typed AttributesMut API ===\n");
    println!("  Damage [PHYSICAL|SWORD]: {phys_sword:.2}");
    println!("  Damage [FIRE|SWORD]:     {fire_sword:.2}");

    // --- Read via typed API on &Attributes (after evaluation has cached values) ---
    if let Some(attrs) = attributes.get_attributes(sword) {
        println!("\n=== Damage via typed Attributes API (read-only) ===\n");
        let phys = attrs.damage(Tags::PHYSICAL | Tags::SWORD);
        let fire = attrs.damage(Tags::FIRE | Tags::SWORD);
        let phys_added = attrs.damage_added(Tags::PHYSICAL | Tags::SWORD);
        let phys_inc = attrs.damage_increased(Tags::PHYSICAL | Tags::SWORD);
        let phys_more = attrs.damage_more(Tags::PHYSICAL | Tags::SWORD);

        println!("  Damage [PHYSICAL|SWORD]: {phys:.2}");
        println!("    added:     {phys_added:.1}");
        println!("    increased: {phys_inc:.2}");
        println!("    more:      {phys_more:.2}");
        println!("  Damage [FIRE|SWORD]:     {fire:.2}");
    }

    // --- String keys still work alongside typed accessors ---
    println!("\n=== String-key API still works ===\n");
    let raw_added = attributes.evaluate_tagged(sword, "Damage.added", Tags::PHYSICAL | Tags::SWORD);
    println!("  attrs.evaluate_tagged(\"Damage.added\", PHYSICAL|SWORD): {raw_added:.1}");

    println!("\n  Expected:");
    println!("    Physical+Sword added:     100 (physical) + 5 (sword) = 105");
    println!("    Physical+Sword increased: 0.50");
    println!("    Physical+Sword more:      1.20 (Product: 1 + 0.20)");
    println!("    Physical+Sword total:     105 * (1 + 0.50) * 1.20 = 189.00");
    println!("    Fire+Sword added:         30 (fire) + 5 (sword) = 35");
    println!("    Fire+Sword increased:     0.25");
    println!("    Fire+Sword more:          1.00 (no fire-tagged more)");
    println!("    Fire+Sword total:         35 * (1 + 0.25) * 1.00 = 43.75");

    println!("\n--- Done ---");
    std::process::exit(0);
}
