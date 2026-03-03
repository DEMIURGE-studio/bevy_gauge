//! # RPG Combat Example — PoE-style tagged damage
//!
//! Demonstrates `bevy_attributes` with a Path of Exile-style damage model:
//!
//! - **`tagged_attribute`** — sets up a multi-part attribute (Added × (1 + Increased))
//!   with per-tag-combo expressions in one call.
//! - **Modifier generality** — a modifier tagged `MELEE` applies to *all*
//!   melee damage. One tagged `FIRE | MELEE` only applies to fire melee.
//!   Untagged modifiers are global and apply to everything.
//! - **Query specificity** — when dealing a fire melee hit, you query with
//!   `FIRE | MELEE` to pull in global, FIRE-only, MELEE-only, and
//!   FIRE|MELEE modifiers.
//! - **Expression tag syntax** — `"Damage.Added{FIRE|MELEE}"` inside
//!   expressions to reference tag-filtered values.
//! - **Cross-entity deps** — the sword references its wielder's attributes via
//!   `@Wielder`. Swapping the wielder automatically rewires everything.
//! - **`attributes!` / `mod_set!` macros** — ergonomic batch init and buffs.
//!
//! Run with: `cargo run --example rpg_combat`

use bevy::prelude::*;
use bevy_attributes::prelude::*;

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------
// Damage types
const PHYSICAL: TagMask = TagMask::bit(0);
const FIRE: TagMask = TagMask::bit(1);
// Delivery / context tags
const MELEE: TagMask = TagMask::bit(2);

// ---------------------------------------------------------------------------
// Marker components & handles
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Sword;

#[derive(Resource)]
struct Entities {
    warrior: Entity,
    mage: Entity,
    sword: Entity,
}

// ---------------------------------------------------------------------------
// AttributeDerived — auto-syncs a component from tagged attribute values
// ---------------------------------------------------------------------------

/// Mirrors the sword's per-type damage totals. Updated automatically each
/// frame by the `AttributeDerived` system in PostUpdate.
#[derive(Component, Default, Debug)]
struct SwordDamageDisplay {
    physical_melee: f32,
    fire_melee: f32,
}

impl AttributeDerived for SwordDamageDisplay {
    fn should_update(&self, attrs: &Attributes, interner: &Interner) -> bool {
        // In a real game you'd compare against the cached tagged values.
        // For simplicity we just check the untagged totals here.
        let phys = attrs.get_by_name("Damage", interner);
        let fire = attrs.get_by_name("Damage", interner);
        (self.physical_melee - phys).abs() > f32::EPSILON
            || (self.fire_melee - fire).abs() > f32::EPSILON
    }

    fn update_from_attributes(&mut self, attrs: &Attributes, interner: &Interner) {
        self.physical_melee = attrs.get_tagged_by_name("Damage", PHYSICAL | MELEE, interner);
        self.fire_melee = attrs.get_tagged_by_name("Damage", FIRE | MELEE, interner);
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(AttributesPlugin)
        .register_attribute_derived::<SwordDamageDisplay>()
        .add_systems(
            Startup,
            (
                register_tags,
                spawn_entities.after(register_tags),
                setup_sword_attributes.after(spawn_entities),
                equip_warrior.after(setup_sword_attributes),
                print_warrior.after(equip_warrior),
                swap_to_mage.after(print_warrior),
                print_mage.after(swap_to_mage),
                show_tag_queries.after(print_mage),
                apply_buff_and_show.after(show_tag_queries),
            ),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Step 0: Register tag names so expressions can use {FIRE|MELEE} syntax
// ---------------------------------------------------------------------------

fn register_tags(mut resolver: ResMut<TagResolver>) {
    resolver.register("PHYSICAL", PHYSICAL);
    resolver.register("FIRE", FIRE);
    resolver.register("MELEE", MELEE);
    println!("--- Tags registered ---\n");
}

// ---------------------------------------------------------------------------
// Step 1: Spawn entities — players get flat attributes via the attributes! macro
// ---------------------------------------------------------------------------

fn spawn_entities(mut commands: Commands) {
    let warrior = commands
        .spawn((
            Attributes::new(),
            attributes! {
                "Strength"     => 50.0,
                "Intelligence" => 10.0,
            },
            Name::new("Warrior"),
        ))
        .id();

    let mage = commands
        .spawn((
            Attributes::new(),
            attributes! {
                "Strength"     => 15.0,
                "Intelligence" => 60.0,
            },
            Name::new("Mage"),
        ))
        .id();

    let sword = commands
        .spawn((
            Sword,
            Attributes::new(),
            SwordDamageDisplay::default(),
            Name::new("Flaming Greatsword"),
        ))
        .id();

    commands.insert_resource(Entities {
        warrior,
        mage,
        sword,
    });

    println!("--- Entities spawned ---\n");
}

// ---------------------------------------------------------------------------
// Step 2: Configure sword attributes — tagged_attribute sets up the formula,
//         then we add modifiers to the parts
// ---------------------------------------------------------------------------

fn setup_sword_attributes(mut attributes: AttributesMut, handles: Res<Entities>) {
    let sword = handles.sword;

    // One call sets up the whole damage pipeline:
    //   - Creates "Damage.Added" (Sum) and "Damage.Increased" (Sum) part nodes.
    //   - Stores the expression template "Added * (1 + Increased)".
    //   - When evaluate_tagged is called with e.g. PHYSICAL|MELEE, the system
    //     auto-generates:
    //       "Damage.Added{PHYSICAL|MELEE} * (1 + Damage.Increased{PHYSICAL|MELEE})"
    //     No need to enumerate every possible tag combo up front!
    attributes
        .tagged_attribute(
            sword,
            "Damage",
            &[("Added", ReduceFn::Sum), ("Increased", ReduceFn::Sum)],
            "Added * (1 + Increased)",
        )
        .expect("valid tagged attribute");

    // --- Damage.Added ---
    // Two tagged modifiers on the SAME attribute:
    //   25 physical melee flat damage
    //   10 fire melee flat damage
    attributes.add_modifier_tagged(sword, "Damage.Added", 25.0, PHYSICAL | MELEE);
    attributes.add_modifier_tagged(sword, "Damage.Added", 10.0, FIRE | MELEE);

    // --- Damage.Increased ---
    // Physical scaling from wielder Strength, fire scaling from Intelligence.
    // Tagged by damage type only — delivery method (MELEE) doesn't matter
    // for scaling, so a MELEE query will also pick these up.
    attributes
        .add_expr_modifier_tagged(
            sword,
            "Damage.Increased",
            "Strength@Wielder / 200",
            PHYSICAL,
        )
        .expect("valid expression");

    attributes
        .add_expr_modifier_tagged(
            sword,
            "Damage.Increased",
            "Intelligence@Wielder / 300",
            FIRE,
        )
        .expect("valid expression");

    println!("--- Sword attributes configured ---\n");
}

// ---------------------------------------------------------------------------
// Step 3: Equip on the Warrior
// ---------------------------------------------------------------------------

fn equip_warrior(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Warrior equips the Flaming Greatsword ===\n");
    attributes.register_source(handles.sword, "Wielder", handles.warrior);
}

// ---------------------------------------------------------------------------
// Step 4: Print Warrior damage
// ---------------------------------------------------------------------------

fn print_warrior(mut attributes: AttributesMut, handles: Res<Entities>) {
    let sword = handles.sword;

    let phys_added = attributes.evaluate_tagged(sword, "Damage.Added", PHYSICAL | MELEE);
    let phys_inc = attributes.evaluate_tagged(sword, "Damage.Increased", PHYSICAL | MELEE);
    let phys_total = attributes.evaluate_tagged(sword, "Damage", PHYSICAL | MELEE);
    let fire_added = attributes.evaluate_tagged(sword, "Damage.Added", FIRE | MELEE);
    let fire_inc = attributes.evaluate_tagged(sword, "Damage.Increased", FIRE | MELEE);
    let fire_total = attributes.evaluate_tagged(sword, "Damage", FIRE | MELEE);

    println!("  Physical Melee Damage:");
    println!("    Added:     {:.1}", phys_added);
    println!("    Increased: {:.4} ({:.1}%)", phys_inc, phys_inc * 100.0);
    println!("    Total:     {:.2}  = {:.1} * (1 + {:.4})", phys_total, phys_added, phys_inc);

    println!("  Fire Melee Damage:");
    println!("    Added:     {:.1}", fire_added);
    println!("    Increased: {:.4} ({:.1}%)", fire_inc, fire_inc * 100.0);
    println!("    Total:     {:.2}  = {:.1} * (1 + {:.4})", fire_total, fire_added, fire_inc);

    println!("\n  Expected (Warrior: Str 50, Int 10):");
    println!("    Physical: 25 * (1 + 50/200) = 25 * 1.25  = 31.25");
    println!("    Fire:     10 * (1 + 10/300) = 10 * 1.033 = 10.33");
    println!();
}

// ---------------------------------------------------------------------------
// Step 5: Swap wielder to the Mage
// ---------------------------------------------------------------------------

fn swap_to_mage(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Mage takes the sword from the Warrior ===");
    println!("    (one call to register_source — edges auto-rewire)\n");
    attributes.register_source(handles.sword, "Wielder", handles.mage);
}

// ---------------------------------------------------------------------------
// Step 6: Print Mage damage
// ---------------------------------------------------------------------------

fn print_mage(mut attributes: AttributesMut, handles: Res<Entities>) {
    let sword = handles.sword;

    let phys_added = attributes.evaluate_tagged(sword, "Damage.Added", PHYSICAL | MELEE);
    let phys_inc = attributes.evaluate_tagged(sword, "Damage.Increased", PHYSICAL | MELEE);
    let phys_total = attributes.evaluate_tagged(sword, "Damage", PHYSICAL | MELEE);
    let fire_added = attributes.evaluate_tagged(sword, "Damage.Added", FIRE | MELEE);
    let fire_inc = attributes.evaluate_tagged(sword, "Damage.Increased", FIRE | MELEE);
    let fire_total = attributes.evaluate_tagged(sword, "Damage", FIRE | MELEE);

    println!("  Physical Melee Damage:");
    println!("    Added:     {:.1}", phys_added);
    println!("    Increased: {:.4} ({:.1}%)", phys_inc, phys_inc * 100.0);
    println!("    Total:     {:.2}  = {:.1} * (1 + {:.4})", phys_total, phys_added, phys_inc);

    println!("  Fire Melee Damage:");
    println!("    Added:     {:.1}", fire_added);
    println!("    Increased: {:.4} ({:.1}%)", fire_inc, fire_inc * 100.0);
    println!("    Total:     {:.2}  = {:.1} * (1 + {:.4})", fire_total, fire_added, fire_inc);

    println!("\n  Expected (Mage: Str 15, Int 60):");
    println!("    Physical: 25 * (1 + 15/200) = 25 * 1.075 = 26.88");
    println!("    Fire:     10 * (1 + 60/300) = 10 * 1.200 = 12.00");
    println!();
}

// ---------------------------------------------------------------------------
// Step 7: Show how tag specificity works
// ---------------------------------------------------------------------------

fn show_tag_queries(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Tag Query Specificity (Mage wielding) ===\n");

    let sword = handles.sword;

    // Damage.Added has two modifiers:
    //   25.0 [PHYSICAL|MELEE]  and  10.0 [FIRE|MELEE]
    //
    // Queries must be at least as specific as the modifier's tags.
    // A modifier matches when ALL its tags are present in the query.

    let all = attributes.evaluate(sword, "Damage.Added");
    let melee = attributes.evaluate_tagged(sword, "Damage.Added", MELEE);
    let physical = attributes.evaluate_tagged(sword, "Damage.Added", PHYSICAL);
    let fire = attributes.evaluate_tagged(sword, "Damage.Added", FIRE);
    let phys_melee = attributes.evaluate_tagged(sword, "Damage.Added", PHYSICAL | MELEE);
    let fire_melee = attributes.evaluate_tagged(sword, "Damage.Added", FIRE | MELEE);

    println!("  Damage.Added (unfiltered):       {:.1}  (25 + 10 = 35)", all);
    println!("  Damage.Added [MELEE]:            {:.1}  (neither mod is MELEE-only)", melee);
    println!("  Damage.Added [PHYSICAL]:         {:.1}  (neither mod is PHYSICAL-only)", physical);
    println!("  Damage.Added [FIRE]:             {:.1}  (neither mod is FIRE-only)", fire);
    println!("  Damage.Added [PHYSICAL|MELEE]:   {:.1}  (matches the 25.0 physical melee mod)", phys_melee);
    println!("  Damage.Added [FIRE|MELEE]:       {:.1}  (matches the 10.0 fire melee mod)", fire_melee);

    // Now add a MELEE-only modifier — like a passive "+5 melee flat damage".
    // This is more general: it should appear in EVERY melee query.
    println!("\n  Adding +5 generic melee damage (tagged MELEE only)...\n");
    attributes.add_modifier_tagged(sword, "Damage.Added", 5.0, MELEE);

    let melee2 = attributes.evaluate_tagged(sword, "Damage.Added", MELEE);
    let phys_melee2 = attributes.evaluate_tagged(sword, "Damage.Added", PHYSICAL | MELEE);
    let fire_melee2 = attributes.evaluate_tagged(sword, "Damage.Added", FIRE | MELEE);

    println!("  Damage.Added [MELEE]:            {:.1}  (the +5 mod is MELEE-only → matches)", melee2);
    println!("  Damage.Added [PHYSICAL|MELEE]:   {:.1}  (25 physical melee + 5 generic melee)", phys_melee2);
    println!("  Damage.Added [FIRE|MELEE]:       {:.1}  (10 fire melee + 5 generic melee)", fire_melee2);
    println!();
}

// ---------------------------------------------------------------------------
// Step 8: Apply a buff using mod_set!
// ---------------------------------------------------------------------------

fn apply_buff_and_show(mut attributes: AttributesMut, handles: Res<Entities>) {
    println!("=== Applying Fire Enchantment via mod_set! ===\n");

    let enchantment = mod_set! {
        "Damage.Added" [FIRE | MELEE] => 20.0,
    };
    enchantment.apply(handles.sword, &mut attributes);

    let fire_added = attributes.evaluate_tagged(handles.sword, "Damage.Added", FIRE | MELEE);
    let fire_total = attributes.evaluate_tagged(handles.sword, "Damage", FIRE | MELEE);

    println!("  After +20 fire melee enchantment:");
    println!("    Damage.Added [FIRE|MELEE]: {:.1}  (was 10+5=15, now 10+5+20=35)", fire_added);
    println!("    Damage [FIRE|MELEE]:       {:.2}  = 35 * (1 + 60/300) = 35 * 1.2 = 42.00", fire_total);

    println!("\n--- Done ---");
    std::process::exit(0);
}
