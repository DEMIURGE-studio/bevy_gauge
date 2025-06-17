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
        .add_systems(Startup, (setup_game_config, spawn_player).chain()) // Call config setup at Startup
        .add_systems(Update, (apply_buff_system, display_health_system))
        .run();
}

// Define your game's stat configuration
fn setup_game_config() { // No longer returns Config
    // let mut config = Config::default(); // No longer needed

    // --- Flat Stats --- 
    // Use Flat stats for values that are typically set directly.
    // They don't need a total_expression registered as their value is their direct numeric content.
    Konfig::register_stat_type("CurrentHealth", "Flat");
    // Example: CurrentHealth is set after taking damage or healing.

    // --- Modifiable Stats --- 
    // Use Modifiable stats for values that have a base amount and can be altered 
    // by a list of modifiers. They also typically don't need a total_expression,
    // as their final value is their internal base after all its modifiers are applied.
    Konfig::register_stat_type("MaxHealth", "Modifiable");
    Konfig::register_stat_type("Strength", "Modifiable");
    Konfig::register_stat_type("AttackPower", "Modifiable");

    // --- Complex Stats --- 
    // Use Complex stats when a stat's total value is calculated from several distinct "parts".
    // These REQUIRE a total_expression to define how parts combine.
    Konfig::register_stat_type("AttackDamage", "Complex");
    Konfig::register_total_expression("AttackDamage", "base * (1 + increased) * more");
    // Modifiers would target parts like "AttackDamage.base", "AttackDamage.increased", etc.
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
Use the `StatsMutator` `SystemParam` for changes.

```rust
fn apply_buff_system(
    mut stats_mutator: StatsMutator,
    player_query: Query<Entity, (With<Player>, Added<Player>)>, 
) {
    if let Ok(player_entity) = player_query.single() {
        // Add +5 to Strength (Modifiable stat)
        println!("Applying +5 Strength buff.");
        stats_mutator.add_modifier(player_entity, "Strength", 5.0);
    }
}

fn display_health_system(
    // For reading stats, query the Stats component directly.
    player_query: Query<(Entity, &Stats), With<Player>>,
) {
    if let Ok((player_entity, stats)) = player_query.single() {
        // Use stats.get() for direct stat values or parts.

        // Get total
        let strength = stats.get("Strength");

        // Get added
        let added_strength = stats.get("Strength.added");
        
        // Get increased fire damage with axes. Process the tag since bevy_gauge doesn't 
        // implicitly understand string based tags.
        // 
        let inc_fire_damage_with_axes = stats.get(Konfig::process_path("Damage.increased.{FIRE|AXE}"));
        let total_ice_damage_with_swords = stats.get(Konfig::process_path("Damage.{ICE|SWORD}"));
    }
}
```
**Note on "Adding/Removing Stats":** When a modifier is added for a stat path (e.g., `"NewStat"` or `"NewStat.part"`) that the entity doesn't yet have, `bevy_gauge` will create that stat on the fly for the entity. The new stat will use default configurations (e.g., `Modifiable` type, default total expression `"0"`) unless specific configurations for `"NewStat"` have been registered using `Konfig` static methods.

TODO Change to allow users to define their own custom defaults for different stat types

### 5. Stat Derived Components
Create Bevy components whose fields are automatically updated from stats.

**Define your component and implement `StatDerived` (and optionally `WriteBack`):**
```rust
#[derive(Component, Default, Debug)]
pub struct Life {
    pub max: f32,
    pub current: f32,
}

impl StatDerived for Life {
    fn from_stats(stats: &bevy_gauge::prelude::Stats) -> Self {
        let mut s = Self::default();
        s.update_from_stats(stats);
        s
    }
    fn should_update(&self, stats: &bevy_gauge::prelude::Stats) -> bool {
        self.max != stats.get("Life").unwrap_or(0.0)
            || self.current != stats.get("$[Life.current]").unwrap_or(0.0)
    }
    fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::Stats) {
        self.max = stats.get("Life").unwrap_or(0.0);
        self.current = stats.get("$[Life.current]").unwrap_or(0.0);
    }
    fn is_valid(stats: &bevy_gauge::prelude::Stats) -> bool {
        stats.get("Life").is_ok() && stats.get("$[Life.current]").is_ok()
    }
}

impl WriteBack for Life {
    fn write_back(
        &self,
        target_entity: Entity,
        stats_mutator: &mut bevy_gauge::prelude::StatsMutator,
    ) {
        let _ = stats_mutator.set(target_entity, "$[Life.current]", self.current);
    }
}
```
The `update_derived_stats` system (included in `bevy_gauge::plugin`) will automatically call `update_from_stats` on your `Life` component if `should_update` returns true..

You can also use the `stat_component!` macro to more easily define your stat-derived components!
```rust
stat_component!(pub struct Life {
    max: <- "Life",           // Explicit path - reads from "Life" stat
    current: <- $,            // Auto-generated path - reads from "$[Life.current]"
});

// For more complex examples:
stat_component!(pub struct Damage {
    base: <- $,               // Auto-generated: "$[Damage.base]"
    current: <-> $,           // Auto-generated: "$[Damage.current]" (bidirectional)
    bonus: <- "BonusDamage",  // Explicit path to a different stat
});

// Nested structures work too:
stat_component!(pub struct WeaponStats {
    damage: DamageRange {
        min: <- $,            // Auto-generated: "$[WeaponStats.damage.min]"
        max: <- $,            // Auto-generated: "$[WeaponStats.damage.max]"
    }
});
```

The `$` syntax automatically generates stat paths based on your component's structure:
- `field: <- $` becomes `field: <- "$[StructName.field]"`
- For nested fields: `nested.field: <- $` becomes `"$[StructName.nested.field]"`
- You can mix explicit paths and auto-generated ones as needed

TODO explain why this (2 sources of truth) can be done safely

## Dive Deeper
For more advanced features like Sources, Tags, Stat Effects, and detailed explanations, please refer to the [User Guide](bevy_gauge.md).

## Contributing
Contributions are welcome! Feel free to open an issue or submit a pull request.

## License
`bevy_gauge` is dual-licensed under either
*   MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
*   Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
at your option.

## TODO 
Implement string interning.