# bevy_gauge

`bevy_gauge` is a flexible stat and modifier system for the [Bevy game engine](https://bevyengine.org/), designed to manage complex character statistics, buffs, debuffs, and equipment effects with ease.

[![crates.io](https://img.shields.io/crates/v/bevy_gauge.svg)](https://crates.io/crates/bevy_gauge)
[![docs.rs](https://docs.rs/bevy_gauge/badge.svg)](https://docs.rs/bevy_gauge)
_(License: MIT OR Apache-2.0)_

## Core Features

- **Dynamic Stats:** Define stats like Life, Mana, Strength, etc.
- **Modifiers:** Add flat bonuses, percentage increases, or complex calculations via expressions.
- **Expression Engine:** Use mathematical expressions to define how stats are calculated (e.g., `base * (1 + increased) * more`).
- **Dependencies:** Stats can depend on other stats, even across different entities (Sources).
- **Tagging:** Apply tags (e.g., "Fire", "Physical", "Sword") to stats and modifiers for fine-grained control over effects.
- **Caching:** Automatic caching of evaluated stats and smart cache invalidation.
- **Change Detection:** `StatsProxy` component provides change detection without ownership conflicts.
- **Derived Components:** Easily create Bevy components whose fields are derived from entity stats, with optional write-back functionality.

## Quick Start

### 1. Add to your `Cargo.toml`

```toml
[dependencies]
bevy_gauge = "0.1" # Replace with the latest version
```

### 2. Add the Plugin and Define Tags

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;
use bevy_gauge::stat_types::ModType;
use bevy_gauge_macros::define_tags;

// Define your game's tag system using the macro
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

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_gauge::plugin) // Essential plugin
        .add_systems(Startup, (setup_game_config, spawn_player).chain())
        .add_systems(Update, (apply_buff_system, display_stats_system))
        .run();
}

// Define your game's stat configuration
fn setup_game_config() {
    // Core attributes (modifiable)
    Konfig::register_stat_type("Strength", "Modifiable");
    Konfig::register_stat_type("Dexterity", "Modifiable");
    Konfig::register_stat_type("Intelligence", "Modifiable");

    // Complex stats (calculated from expressions)
    Konfig::register_stat_type("Life", "Complex");
    Konfig::register_stat_type("Mana", "Complex");
    Konfig::register_stat_type("Accuracy", "Complex");
    Konfig::register_stat_type("Evasion", "Complex");

    // Tagged stats (filterable by tags)
    Konfig::register_stat_type("Damage", "Tagged");

    // Set default formula for complex stats (PoE-style)
    Konfig::set_total_expression_default("added * (1.0 + increased) * more");

    // Configure "more" modifiers as multiplicative
    Konfig::register_relationship_type("more", ModType::Mul);

    // Register tag resolver for damage calculations
    Konfig::register_tag_set("Damage", Box::new(DamageTags));
}

#[derive(Component)]
struct Player;
```

### 3. Spawning an Entity with Stats

```rust
fn spawn_player(mut commands: Commands) {
    commands.spawn((
        Player,
        stats! {
            // Core Attributes
            "Strength" => 25.0,
            "Dexterity" => 18.0,
            "Intelligence" => 33.0,

            // Complex Stats (PoE-style formulas)
            "Life.added" => [100.0, "Strength / 2.0"], // Base + strength bonus
            "Life.more" => 0.4, // 40% more life multiplier
            "Mana.added" => [50.0, "Intelligence / 5.0"], // Base + intelligence bonus
            "Accuracy.added" => 50.0,
            "Accuracy.increased" => "Dexterity / 2.0", // Dexterity increases accuracy

            // Tagged Stats (string-based tags)
            "Damage.added.{MELEE|PHYSICAL}" => 50.0,
            "Damage.increased.{MELEE|PHYSICAL}" => "Strength / 2.0 / 100.0",

            // Percentage bonuses
            "Evasion.increased" => "Dexterity / 2.0 / 100.0",
            "EnergyShield.increased" => "Intelligence / 5.0 / 100.0",
        },
    ));
}
```

### 4. Modifying Stats & Querying

```rust
fn apply_buff_system(
    mut stats_mutator: StatsMutator,
    q_player: Query<Entity, Added<Stats>>,
) {
    if let Ok(player_entity) = q_player.single() {
        // Add modifiers
        stats_mutator.add_modifier(player_entity, "Strength", 5.0);
        stats_mutator.add_modifier(player_entity, "Damage.added.{SWORD|PHYSICAL}", 15.0);
        stats_mutator.add_modifier(player_entity, "Damage.increased.{FIRE|MELEE}", 0.2);
        stats_mutator.add_modifier(player_entity, "Damage.more.{PHYSICAL}", 0.5);
    }
}

fn display_stats_system(
    q_player: Query<&Stats, With<Player>>,
) {
    if let Ok(stats) = q_player.single() {
        // Get stats with string-based tags
        let strength = stats.get("Strength");
        let life = stats.get("Life");
        let axe_physical_damage = stats.get("Damage.{AXE|PHYSICAL}");
        let fire_sword_damage = stats.get("Damage.{FIRE|SWORD}");

        // Cross-entity dependencies
        let weapon_damage = stats.get("Damage.added@weapon");

        println!("Str: {:.1}, Life: {:.1}", strength, life);
        println!("Axe Physical: {:.2}, Fire Sword: {:.2}", axe_physical_damage, fire_sword_damage);
    }
}
```

### 5. Stat Derived Components

Create Bevy components whose fields are automatically updated from stats:

```rust
stat_component!(
    #[derive(Debug)]
    pub struct Health {
        max: f32 <- "Life",           // Derived from Life stat
        current: f32 -> $,            // Writes Health.current value to "$[Health.current]" stat
    }
);
```

```rust
// Write-back support for mutable state
impl WriteBack for Health {
    fn write_back(&self, target_entity: Entity, stats_mutator: &mut StatsMutator) {
        let _ = stats_mutator.set(target_entity, "$[Health.current]", self.current);
    }
}
```

```rust
stat_component!(
    #[derive(Clone, Debug)]
    pub struct PlayerStats {
        damage: f32 <- "Damage.{PHYSICAL}",
        accuracy: f32 <- "Accuracy",
        life: f32 <- "Life",

        // Non-stat fields maintain their values independently
        pub name: String,
        pub level: u32,

        // Optional stats (0.0 becomes None)
        bonus: Option<f32> <- "BonusStat",
    }
);
```

Components update automatically when their underlying stats change.

## Change Detection with StatsProxy

`bevy_gauge` includes a `StatsProxy` component that automatically tracks when an entity's `Stats` have been modified. This provides efficient change detection without the ownership conflicts that would occur if you tried to use `Changed<Stats>` directly in systems that also use `StatsMutator`. This is mostly used internally, but it is there if you need it.

For more examples, see the `examples/` directory in the repository.

## Contributing

Contributions are welcome! Feel free to open an issue or submit a pull request.

## Version Compatibility

| bevy_gauge_macros | bevy_gauge | bevy |
| ----------------- | ---------- | ---- |
| 0.3.0             | 0.3.0      | 0.18 |
| 0.2.0             | 0.2.2      | 0.17 |
| 0.1.1             | 0.1.1      | 0.16 |

_Note: This crate is in active development. APIs may change between versions._

## License

`bevy_gauge` is dual-licensed under either

- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
  at your option.

## TODO

- Implement string interning
- Automatic stat tag string resolution (currently works in `stats!` macro but not in `stats.get()` calls)
