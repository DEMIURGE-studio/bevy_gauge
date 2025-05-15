use bevy::prelude::*;
use bevy_gauge::prelude::*;

// --- Components --- //

#[derive(Component)]
struct Player;

// --- Setup --- //

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_gauge::plugin)
        .insert_resource(app_config())
        .add_systems(Startup, setup_player)
        .add_systems(Update, get_stats_system)
        .run();
}

fn app_config() -> Config {
    let mut config = Config::default();

    // Core Attributes (Modifiable)
    // No need to register total_expression, defaults to its base value after modifiers.
    config.register_stat_type("Strength", "Modifiable");
    config.register_stat_type("Dexterity", "Modifiable");
    config.register_stat_type("Intelligence", "Modifiable");

    // Derived Stats (Modifiable)
    // These will have their base values modified by expressions derived from core attributes.
    config.register_stat_type("Life", "Complex");
    config.register_stat_type("Damage", "Tagged");
    config.register_stat_type("Accuracy", "Complex");
    config.register_stat_type("Evasion", "Complex");
    config.register_stat_type("Mana", "Complex");
    config.register_stat_type("EnergyShield", "Complex");

    config
}

fn setup_player(mut commands: Commands, mut stat_accessor: StatAccessor) {
    let player_entity = commands
        .spawn((
            Player,
            stats! {
                "Strength" => 25.0,
                "Dexterity" => 18.0,
                "Intelligence" => 33.0,
                "Life.added" => [100.0, "(Strength / 10.0) * 5.0"], // Base life + Str bonus
                "Accuracy.added" => 50.0,     // Base accuracy before Dex bonus modifier
                "Accuracy.increased" => "(Dexterity / 10.0) * 20.0",
                "Mana.added" => [50.0, "(Intelligence / 10.0) * 5.0"],         // Base mana before Int bonus modifier
                "Damage.increased.MELEE|PHYSICAL" => "(Strength / 10.0) * 0.02",
                "Evasion.increased" => "(Dexterity / 10.0) * 0.02",
                "EnergyShield.increased" => "(Intelligence / 10.0) * 0.02",
            },
            Name::new("Player"),
        ))
        .id();

    println!("Player setup complete with attribute conversion modifiers.");
    println!("Initial attributes: Str: 25, Dex: 18, Int: 33");
}

fn add_modifiers_system(
    mut stat_accessor: StatAccessor,
    player_query: Query<Entity, With<Player>>,
) {
    if let Ok(player_entity) = player_query.get_single() {
        stat_accessor.add_modifier(player_entity, "Strength", Expression::new("25.0").unwrap());
    }
}

fn get_stats_system(
    player_query: Query<&Stats, With<Player>>,
) {
    if *ran_once {
        return;
    }

    if let Ok(player_stats) = player_query.get_single() {
        // Access stats directly
        let strength = player_stats.get("Strength");
        let dexterity = player_stats.get("Dexterity");
        let intelligence = player_stats.get("Intelligence");

        println!("Attributes:");
        println!("  Strength: {:.1}", strength);
        println!("  Dexterity: {:.1}", dexterity);
        println!("  Intelligence: {:.1}", intelligence);

        let life = player_stats.get("Life");
        let melee_phys_increase = player_stats.get("Damage.increased.MELEE|PHYSICAL");
        let accuracy = player_stats.get("Accuracy");
        let evasion_increase = player_stats.get("Evasion.increased");
        let mana = player_stats.get("Mana");
        let es_increase = player_stats.get("EnergyShield.increased");

        println!("\nDerived Stats:");
        println!("  Max Life: {:.1}", life);
        println!("    (From Str {}: Base 100 + ({}/10).floor()*5 = {:.1})", 
            strength, strength, 100.0 + (strength/10.0).floor()*5.0);

        println!("  Melee Physical Damage Increase: {:.2}%", melee_phys_increase * 100.0);
        println!("    (From Str {}: ({}/10).floor()*2% = {:.2}%)", 
            strength, strength, (strength/10.0).floor()*2.0);

        println!("  Accuracy Rating: {:.1}", accuracy);
        println!("    (From Dex {}: Base 50 + ({}/10).floor()*20 = {:.1})", 
            dexterity, dexterity, 50.0 + (dexterity/10.0).floor()*20.0);

        println!("  Evasion Increase: {:.2}%", evasion_increase * 100.0);
        println!("    (From Dex {}: ({}/10).floor()*2% = {:.2}%)", 
            dexterity, dexterity, (dexterity/10.0).floor()*2.0);

        println!("  Max Mana: {:.1}", mana);
        println!("    (From Int {}: Base 50 + ({}/10).floor()*5 = {:.1})", 
            intelligence, intelligence, 50.0 + (intelligence/10.0).floor()*5.0);

        println!("  Energy Shield Increase: {:.2}%", es_increase * 100.0);
        println!("    (From Int {}: ({}/10).floor()*2% = {:.2}%)", 
            intelligence, intelligence, (intelligence/10.0).floor()*2.0);
    }
} 