# bevy_gauge

`bevy_gauge` is a flexible stat and modifier system for the [Bevy game engine](https://bevyengine.org/), designed to manage complex character statistics, buffs, debuffs, and equipment effects with ease.

[![crates.io](https://img.shields.io/crates/v/bevy_gauge.svg)](https://crates.io/crates/bevy_gauge)
[![docs.rs](https://docs.rs/bevy_gauge/badge.svg)](https://docs.rs/bevy_gauge)
_(License: MIT OR Apache-2.0)_

## Core Features

*   **Dynamic Stats:** Define stats like Health, Mana, Strength, etc.
*   **Modifiers:** Add flat bonuses, percentage increases, or complex calculations via expressions.
*   **Expression Engine:** Use mathematical expressions to define how stats are calculated (e.g., `base * (1 + increased) * more`).
*   **Dependencies:** Stats can depend on other stats, even across different entities (Sources).
*   **Tagging:** Apply tags (e.g., "Fire", "Physical", "Sword") to stats and modifiers for fine-grained control over effects.
*   **Caching:** Automatic caching of evaluated stats and smart cache invalidation.
*   **Derived Components:** Easily create Bevy components whose fields are derived from entity stats, with optional write-back functionality.

## Quick Start

### 1. Add to your `Cargo.toml`
```toml
[dependencies]
bevy_gauge = "0.1" # Replace with the latest version
```

### 2. Add the Plugin
```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_gauge::plugin) // Essential plugin
        .insert_resource(setup_game_config()) // Your game's stat configuration
        .add_systems(Startup, spawn_player)
        .add_systems(Update, (apply_buff_system, display_health_system))
        .run();
}

// Define your game's stat configuration
fn setup_game_config() -> Config {
    let mut config = Config::default();

    // --- Flat Stats --- 
    // Use Flat stats for values that are typically set directly.
    // They don't need a total_expression registered as their value is their direct numeric content.
    config.register_stat_type("CurrentHealth", "Flat");
    // Example: CurrentHealth is set after taking damage or healing.

    // --- Modifiable Stats --- 
    // Use Modifiable stats for values that have a base amount and can be altered 
    // by a list of modifiers. They also typically don't need a total_expression,
    // as their final value is their internal base after all its modifiers are applied.
    config.register_stat_type("MaxHealth", "Modifiable");
    config.register_stat_type("Strength", "Modifiable");
    config.register_stat_type("AttackPower", "Modifiable");

    // --- Complex Stats --- 
    // Use Complex stats when a stat's total value is calculated from several distinct "parts".
    // These REQUIRE a total_expression to define how parts combine.
    config.register_stat_type("AttackDamage", "Complex");
    config.register_total_expression("AttackDamage", "base * (1 + increased) * more");
    // Modifiers would target parts like "AttackDamage.base", "AttackDamage.increased", etc.

    config
}

#[derive(Component)]
struct Player;
```

### 3. Spawning an Entity with Stats
Use the `stats!` macro to easily initialize stats. `StatsInitializer` (which `stats!` creates) automatically adds the `Stats` component if it's not present.
```rust
fn spawn_player(mut commands: Commands) {
    commands.spawn((
        Player,
        stats! { // Creates a StatsInitializer component
            // For Modifiable stats, initialize the stat name directly.
            // This sets the internal 'base' value of the Modifiable stat.
            "MaxHealth" => [
                100.0,              // Initial base value
                "Strength * 2.0"    // Bonus from Strength (Expression modifying MaxHealth's base)
            ],
            "Strength" => 10.0,
            // Flat stats are initialized with their direct value.
            "CurrentHealth" => 100.0,
            // AttackPower (Modifiable) is initialized with an expression for its base value.
            "AttackPower" => "Strength * 1.5", 

            // For Complex stats, you initialize their parts:
            "AttackDamage.base" => 25.0,
            "AttackDamage.increased" => 0.1 // 10% initial increased damage
        },
    ));
    println!("Player spawned.");
}
```

### 4. Modifying Stats & Adding/Removing Modifiers
Use the `StatAccessor` `SystemParam` for changes.

```rust
fn apply_buff_system(
    mut stat_accessor: StatAccessor,
    player_query: Query<Entity, (With<Player>, Added<Player>)>, 
) {
    if let Ok(player_entity) = player_query.get_single() {
        // Add +5 to Strength (Modifiable stat)
        println!("Applying +5 Strength buff.");
        stat_accessor.add_modifier(player_entity, "Strength", 5.0);

        // Set CurrentHealth (Flat stat) directly
        // let current_max_health = stat_accessor.evaluate(player_entity, "MaxHealth"); // May need evaluate for complex dependencies
        // stat_accessor.set(player_entity, "CurrentHealth", current_max_health);
    }
}

fn display_health_system(
    // For reading stats, query the Stats component directly.
    player_query: Query<(Entity, &Stats), With<Player>>,
) {
    if let Ok((player_entity, stats_component)) = player_query.get_single() {
        // Use stats_component.get() for direct stat values or parts.
        // Use stats_component.evaluate() if the stat is Complex/Tagged or has expression modifiers
        // that need full re-evaluation based on other potentially changed stats.
        // StatAccessor::evaluate() is also an option if you don't have &Stats.

        let strength = stats_component.get("Strength");
        let attack_power = stats_component.evaluate("AttackPower"); // Evaluate as it depends on Strength expression
        let max_health = stats_component.evaluate("MaxHealth"); // Evaluate as it depends on Strength expression
        let current_health = stats_component.get("CurrentHealth");
        let attack_damage = stats_component.evaluate("AttackDamage"); // Complex stat, needs evaluation

        println!(
            "Player Stats -- STR: {}, ATK_PWR: {}, HP: {}/{}, DMG: {}",
            strength, attack_power, current_health, max_health, attack_damage
        );
    }
}
```
**Note on "Adding/Removing Stats":** When a modifier is added for a stat path (e.g., `"NewStat"` or `"NewStat.part"`) that the entity doesn't yet have, `bevy_gauge` will create that stat on the fly for the entity. The new stat will use default configurations (e.g., `Modifiable` type, default total expression `"0"`) unless specific configurations for `"NewStat"` have been registered in the `Config` resource.

TODO Change to allow users to define their own custom defaults for different stat types

### 5. Stat Derived Components
Create Bevy components whose fields are automatically updated from stats.

**Define your component and implement `StatDerived` (and optionally `WriteBack`):**
```rust
TODO
```
The `update_derived_stats` system (included in `bevy_gauge::plugin`) will automatically call `update_from_stats` on your `PlayerHealthDisplay` component if `should_update` returns true. For a more detailed explanation and on how the `update_from_stats` is actually called with proper accessor, please refer to the full User Guide.

## Dive Deeper
For more advanced features like Sources, Tags, Stat Effects, and detailed explanations, please refer to the [User Guide](bevy_gauge.md).

## Contributing
Contributions are welcome! Feel free to open an issue or submit a pull request.

## License
`bevy_gauge` is dual-licensed under either
*   MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
*   Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
at your option.
