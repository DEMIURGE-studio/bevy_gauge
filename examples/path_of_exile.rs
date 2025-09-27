use bevy::prelude::*;
use bevy_gauge::prelude::*;
use bevy_gauge::stat_types::ModType;
use bevy_gauge_macros::define_tags;

define_tags!{
    DamageTags,
    damage_type {
        elemental { fire, cold, lightning },
        physical,
        chaos,
    },
    weapon_type {
        melee { sword, axe },
        ranged { bow, wand },
    },
}

// --- Components --- //

#[derive(Component)]
struct Player;

// --- Setup --- //

fn main() {
    app_config();
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_gauge::plugin)
        .add_systems(Startup, setup_player)
        .add_systems(Startup, get_stats_system.after(setup_player))
        .run();
}

fn app_config() {
    // Core Attributes (Modifiable)
    // No need to register total_expression, defaults to its base value after modifiers.
    Konfig::register_stat_type("Strength", "Modifiable");
    Konfig::register_stat_type("Dexterity", "Modifiable");
    Konfig::register_stat_type("Intelligence", "Modifiable");

    // Derived Stats (Modifiable)
    // These will have their base values modified by expressions derived from core attributes.
    Konfig::register_stat_type("Life", "Complex");
    Konfig::register_stat_type("Damage", "Tagged");
    Konfig::register_stat_type("Accuracy", "Complex");
    Konfig::register_stat_type("Evasion", "Complex");
    Konfig::register_stat_type("Mana", "Complex");
    Konfig::register_stat_type("EnergyShield", "Complex");
    Konfig::set_total_expression_default("added * (1.0 + increased) * more");
    
    // Configure "more" as multiplicative so percentages work correctly
    Konfig::register_relationship_type("more", ModType::Mul);
    
    // Register tag set for Damage to resolve {MELEE|PHYSICAL} tags
    Konfig::register_tag_set("Damage", Box::new(DamageTags));
}

fn setup_player(mut commands: Commands) {
    commands
        .spawn((
            Player,
            stats! {
                "Strength" => 25.0, // Some arbitrary base values
                "Dexterity" => 18.0,
                "Intelligence" => 33.0,
                "Life.added" => [100.0, "Strength / 2.0"], // base + dynamic modifier
                "Life.more" => 0.4, // "40% more life" multiplier. Multipliers are multiplicative
                "Accuracy.added" => 50.0,
                "Accuracy.increased" => "Dexterity / 2.0",
                "Mana.added" => [50.0, "Intelligence / 5.0"], // base + dynamic modifier
                "Damage.added.{MELEE|PHYSICAL}" => 50.0, // base melee physical damage
                "Damage.increased.{MELEE|PHYSICAL}" => "Strength / 2.0 / 100.0", // bonus melee physical damage from strength
                "Evasion.increased" => "Dexterity / 2.0 / 100.0",
                "EnergyShield.increased" => "Intelligence / 5.0 / 100.0",
            },
            Name::new("Player"),
        ));

    println!("Player setup complete with attribute conversion modifiers.");
    println!("Initial attributes: Str: 25, Dex: 18, Int: 33");
}

fn get_stats_system(
    q_player: Query<&Stats, With<Player>>,
) {
    if let Ok(player_stats) = q_player.single() {
        // Access stats directly
        let strength = player_stats.get("Strength");
        let dexterity = player_stats.get("Dexterity");
        let intelligence = player_stats.get("Intelligence");

        println!("Attributes:");
        println!("  Strength: {:.1}", strength);
        println!("  Dexterity: {:.1}", dexterity);
        println!("  Intelligence: {:.1}", intelligence);

        let life = player_stats.get("Life");
        let axe_physical = player_stats.get(Konfig::process_path("Damage.{AXE|PHYSICAL}").as_str());
        let accuracy = player_stats.get("Accuracy");
        let evasion_increase = player_stats.get("Evasion.increased");
        let mana = player_stats.get("Mana");
        let es_increase = player_stats.get("EnergyShield.increased");

        println!("\nDerived Stats:");
        println!("  Max Life: {:.1}", life);
        println!("    (From Str {}: (Base 100 + {} / 2) * (1 + 0.4) = {:.1})", 
            strength, strength, (100.0 + (strength / 2.0)) * 1.4);

        println!("  Axe Physical Damage: {:.2}", axe_physical);
        println!("    (From Str {}: 50 * (1 + {} / 2 / 100) = {:.2})", 
            strength, strength, 50.0 * (1.0 + (strength / 2.0 / 100.0)));

        println!("  Accuracy Rating: {:.1}", accuracy);
        println!("    (From Dex {}: 50 * (1 + {} / 2) = {:.1})", 
            dexterity, dexterity, 50.0 * (1.0 + (dexterity / 2.0)));

        println!("  Evasion Increase: {:.2}%", evasion_increase * 100.0);
        println!("    (From Dex {}: {} / 2 / 100 = {:.2}%)", 
            dexterity, dexterity, (dexterity / 2.0 / 100.0) * 100.0);

        println!("  Max Mana: {:.1}", mana);
        println!("    (From Int {}: 50 + {} / 5 = {:.1})", 
            intelligence, intelligence, 50.0 + (intelligence / 5.0));

        println!("  Energy Shield Increase: {:.2}%", es_increase * 100.0);
        println!("    (From Int {}: {} / 5 / 100 = {:.2}%)", 
            intelligence, intelligence, (intelligence / 5.0 / 100.0) * 100.0);
    }
} 