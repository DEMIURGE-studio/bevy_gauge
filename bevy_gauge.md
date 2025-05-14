# bevy_gauge User Guide

Welcome to `bevy_gauge`, a flexible stat and modifier system for the Bevy game engine! This guide will walk you through its core features and how to use them effectively in your projects.

## Table of Contents

1.  [Introduction](#introduction)
2.  [Core Concepts](#core-concepts)
    *   [Stats Component](#stats-component)
    *   [StatAccessor SystemParam](#stataccessor-systemparam)
    *   [Configuration](#configuration)
3.  [Stat Entity Initialization](#stat-entity-initialization)
    *   [Using `StatsInitializer`](#using-statsinitializer)
    *   [The `stats!` Macro](#the-stats-macro)
4.  [Stat Manipulation](#stat-manipulation)
    *   [Adding Modifiers](#adding-modifiers)
    *   [Removing Modifiers](#removing-modifiers)
    *   [Evaluating Stats](#evaluating-stats)
    *   [Setting Base Stat Values](#setting-base-stat-values)
5.  [Sources](#sources)
    *   [Concept](#concept)
    *   [Registering Sources](#registering-sources)
    *   [Unregistering Sources](#unregistering-sources)
6.  [Custom Source Registration](#custom-source-registration)
7.  [How to Write Expressions](#how-to-write-expressions)
    *   [Syntax](#syntax)
    *   [Available Variables](#available-variables)
    *   [Using Source Stats in Expressions](#using-source-stats-in-expressions)
8.  [Stat Entity Destruction](#stat-entity-destruction)
9.  [Stat Requirements](#stat-requirements)
    *   [The `StatRequirements` Component](#the-statrequirements-component)
    *   [The `requires!` Macro](#the-requires-macro)
10. [Tags](#tags)
    *   [Concept](#concept-1)
    *   [Usage in Stat Paths](#usage-in-stat-paths)
11. [Stat Types: Flat vs Modifiable vs Complex vs Tagged](#stat-types-flat-vs-modifiable-vs-complex-vs-tagged)
    *   [`Flat`](#flat)
    *   [`Modifiable`](#modifiable)
    *   [`Complex`](#complex)
    *   [`Tagged`](#tagged)
12. [Modifier Sets](#modifier-sets)
    *   [The `ModifierSet` Struct](#the-modifierset-struct)
    *   [The `modifier_set!` Macro](#the-modifier_set-macro)
13. [Stat Effects](#stat-effects)
    *   [The `StatEffect` Trait](#the-stateffect-trait)
    *   [Applying and Removing Effects](#applying-and-removing-effects)
14. [Stat Derived Components](#stat-derived-components)
    *   [The `StatDerived` Trait](#the-statderived-trait)
    *   [The `WriteBack` Trait](#the-writeback-trait)
    *   [The `stat_component!` Macro](#the-stat_component-macro)

## Introduction

`bevy_gauge` provides a powerful way to manage statistics for entities in your Bevy games. It supports:
*   Complex calculations via expressions.
*   Modifiers (buffs, debuffs, equipment bonuses).
*   Dependencies between stats, even across different entities (Sources).
*   Tagging for fine-grained modifier application.
*   Automatic caching and cache invalidation.

## Core Concepts

### Stats Component
The primary component is `bevy_gauge::prelude::Stats`. Entities with this component can have statistics managed by the system.

### StatAccessor SystemParam
The `bevy_gauge::prelude::StatAccessor` is a Bevy `SystemParam` that provides the API for interacting with stats (reading, writing, adding/removing modifiers, etc.).

`Stats` components should not be modified except through the API provided by the `StatAccessor`.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn my_system(mut stat_accessor: StatAccessor, /* ... */) {
    // Use stat_accessor to interact with stats
}
```

### Configuration
The `bevy_gauge::prelude::Config` resource is used to define your game's stat types and how they are calculated. This is typically done at startup.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn setup_game_config() -> Config {
    let mut config = Config::default();

    // Define a "Health" stat that is Modifiable (e.g., can have +MaxHP mods)
    config.register_stat_type("Health", "Modifiable");
    // Its total value is determined by its "base" part.
    config.register_total_expression("Health", "base");

    // Define a "Damage" stat that is Tagged (e.g., can have "Fire" Damage, "Physical" Damage)
    config.register_stat_type("Damage", "Tagged");
    // Its total value is calculated using "base", "increased", and "more" parts.
    config.register_total_expression("Damage", "base * (1.0 + increased) * more");

    config
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_gauge::plugin)
        .insert_resource(setup_game_config()) // Make sure Config is available
        .run();
}
```

## Stat Entity Initialization

`Stats` entities (i.e., entities with a `Stats` component) are initialized using the `StatsInitializer` component. The `StatsInitializer` contains a `ModifierSet` that defines the initial base values and modifiers for the entity's stats.

### Using `StatsInitializer`
The `StatsInitializer` component holds a `ModifierSet` that will be applied once by the `apply_stats_initializer` system.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn spawn_player(mut commands: Commands) {
    let mut initial_mods = ModifierSet::default();
    initial_mods.add("Health.base", 100.0); // Base Health
    initial_mods.add("Mana.base", 50.0);   // Base Mana
    initial_mods.add("Damage.base.Fire", 10.0); // Base Fire Damage

    commands.spawn((
        PlayerTag, // Your custom component
        Stats::new(), // The core stats component
        StatsInitializer::new(initial_mods), // Initial stat values
    ));
}

#[derive(Component)]
struct PlayerTag;
```

### The `stats!` Macro
The `stats!` macro provides a convenient way to create a `StatsInitializer` component.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn spawn_enemy(mut commands: Commands) {
    commands.spawn((
        EnemyTag,
        Stats::new(),
        stats! { // Creates a StatsInitializer
            "Health.base" => 50.0,
            "AttackPower.base" => 5.0
        }
    ));
}

#[derive(Component)]
struct EnemyTag;
```
This macro populates a `ModifierSet` within the `StatsInitializer`.

## Stat Manipulation

The `StatAccessor` system parameter is your primary tool for interacting with stats after initialization.

### Adding Modifiers
You can add modifiers to an entity's stats dynamically. Modifiers can be simple numerical values or expressions.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn apply_strength_buff(
    mut stat_accessor: StatAccessor,
    player_query: Query<Entity, With<PlayerTag>>,
) {
    if let Ok(player_entity) = player_query.get_single() {
        // Add +10 to the "base" part of the "Strength" stat
        stat_accessor.add_modifier(player_entity, "Strength.base", 10.0);

        // Add a 20% increased damage modifier to the "Damage" stat, tagged with '1' (e.g., Fire)
        // This assumes "Damage" is a Tagged or Complex stat with an "increased" part.
        stat_accessor.add_modifier(player_entity, "Damage.increased.1", 0.20);

        // Add a modifier whose value is determined by an expression
        // (e.g., AttackPower gets +50% of Strength)
        stat_accessor.add_modifier(
            player_entity,
            "AttackPower.base",
            Expression::new("Strength.total * 0.5").unwrap()
        );
    }
}
```
The `path` string (e.g., `"Strength.base"`, `"Damage.increased.1"`) determines which stat, which part of the stat, and which tags are affected.

### Removing Modifiers
To remove a modifier, you need to provide the exact same path and `ModifierType` (value or expression) that was used to add it.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn remove_strength_buff(
    mut stat_accessor: StatAccessor,
    player_query: Query<Entity, With<PlayerTag>>,
) {
    if let Ok(player_entity) = player_query.get_single() {
        // Remove the +10 "Strength.base" modifier
        stat_accessor.remove_modifier(player_entity, "Strength.base", 10.0);

        // Remove the expression-based modifier for AttackPower
        stat_accessor.remove_modifier(
            player_entity,
            "AttackPower.base",
            Expression::new("Strength.total * 0.5").unwrap()
        );
    }
}
```

### Evaluating Stats
You can retrieve the calculated value of any stat at any time.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn display_player_health(
    stat_accessor: StatAccessor,
    player_query: Query<Entity, With<PlayerTag>>,
) {
    if let Ok(player_entity) = player_query.get_single() {
        let current_health = stat_accessor.evaluate(player_entity, "Health"); // Evaluating the top level stat gives you the total
        let fire_damage = stat_accessor.evaluate(player_entity, "Damage.total.Fire"); // Assuming "Fire" tag
        println!("Player Health: {}, Fire Damage: {}", current_health, fire_damage);
    }
}
```
The system automatically caches evaluated stats and invalidates the cache when underlying values or dependencies change.

### Setting Base Stat Values
For stats where you want to directly set a value (often for "current" values like current health), you can use `set`. This is often used in conjunction with `StatDerived` components (see later).

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn take_damage(
    mut stat_accessor: StatAccessor,
    player_query: Query<Entity, With<PlayerTag>>,
    damage_amount: f32,
) {
    if let Ok(player_entity) = player_query.get_single() {
        let current_health = stat_accessor.evaluate(player_entity, "Health.current");
        let new_health = current_health - damage_amount;
        // Directly set the "base" of "Health.current"
        // Note: "Health.current" would typically be a "Flat" or "Modifiable" stat
        // configured to use its "base" as its total.
        stat_accessor.set(player_entity, "Health.current", new_health.max(0.0));
    }
}
```

## Sources

### Concept
Sources allow one entity's stats to influence another entity's stats. For example, a player's "Charisma" stat might influence an NPC's "Likability" stat towards the player.

When an entity (the "target") needs to calculate a stat that depends on another entity (the "source"), it can reference the source's stats in its expressions.

### Registering Sources
You need to tell the target entity about its sources using `StatAccessor::register_source`.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

#[derive(Component)] struct LeaderTag;
#[derive(Component)] 
struct Minion {
    leader: Entity,
}

fn link_leader_to_minion(
    mut stat_accessor: StatAccessor,
    leader_query: Query<Entity, With<LeaderTag>>,
    minion_query: Query<(Entity, &Minion), Changed<Minion>>,
) {
    for (minion_entity, minion) in minion_query.iter() {
        let leader_entity = minion.0;
        stat_accessor.register_source(minion_entity, "LeaderAlias", leader_entity);
    }
}

// and later...

// Example: Minion's AttackPower gets a bonus from the Leader's Strength
// This modifier would be on the Minion's Stats component.
// The expression refers to `LeaderAlias@Strength.total`.
stat_accessor.add_modifier(
    minion_entity,
    "AttackPower.base",
    Expression::new(""LeaderAlias@Strength.total" * 0.1").unwrap() // 10% of leader's strength
);
```

### Unregistering Sources
If the relationship ends, you should unregister the source.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn unlink_leader_from_minion(
    mut stat_accessor: StatAccessor,
    minion_query: Query<Entity, With<MinionTag>>,
) {
    if let Ok(minion_entity) = minion_query.get_single() {
        stat_accessor.unregister_source(minion_entity, "LeaderAlias");
    }
}
```

## Custom Source Registration
While `bevy_gauge` handles source registration and stat lookups across entities automatically once registered, if you have highly dynamic or complex ways of determining *which* entity is a source or *how* a source connection is established beyond simple proximity or explicit linking, you would implement that logic in your own Bevy systems. These systems would then use `StatAccessor::register_source` and `StatAccessor::unregister_source` to inform `bevy_gauge` about these relationships. `bevy_gauge` will automatically update relevant stats when a source is registered or unregistered.

## How to Write Expressions

Expressions are Rust strings that are parsed and evaluated by the `evalexpr` crate. They define how stats (or parts of stats) are calculated.

### Syntax
Expressions follow standard mathematical syntax. You can use:
*   Numbers (e.g., `100.0`, `0.5`)
*   Arithmetic operators (`+`, `-`, `*`, `/`)
*   Parentheses for grouping `()`
*   Stat paths as variables (see below)

### Available Variables
Within an expression for a stat, you can typically refer to:
*   **Parts of the current stat**: If a `Complex` or `Tagged` stat is defined with parts like `base`, `increased`, `more`, these can be used directly as variables in its `total_expression`.
    *   Example: `"base * (1.0 + increased) * more"`
*   **Other stats on the same entity**: You can reference the total value of other stats on the same entity by their name, enclosed in double quotes.
    *   Example: `"Strength * 2.0"`
*   **Literal values**

### Using Source Stats in Expressions
To use a stat from a registered source entity, you prefix the stat path with the `SourceAlias` followed by an `@` symbol, all enclosed in double quotes.

*   Example: If a source was registered with alias `"Leader"`, an expression on the target entity could be:
    `"Strength.total@Leader * 0.5"`

This would fetch the `Strength.total` value from the entity registered as `"Leader"`.

## Stat Entity Destruction
When an entity with a `Stats` component is despawned, Bevy handles the removal of the `Stats` component itself. `bevy_gauge` includes an observer system (`remove_stats`) that cleans up any dependencies or source registrations related to the despawned entity, preventing dangling references or incorrect calculations.

## Stat Requirements

TODO This is NOT what StatRequirements is for.
### The `StatRequirements` Component
The `bevy_gauge::prelude::StatRequirements` component can be added to an entity to declare that it needs certain stats to be present and valid for some of its systems or logic to function correctly. This is more of a convention for your systems to check rather than something `bevy_gauge` strictly enforces on its own calculations.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn spawn_warrior(mut commands: Commands) {
    commands.spawn((
        Stats::new(),
        stats! { /* ... initial stats ... */ },
        // This warrior requires "Stamina" and "Rage" stats to be usable.
        requires!["Stamina", "Rage"]
    ));
}
```

### The `requires!` Macro
The `requires!` macro is a convenient way to create a `StatRequirements` component.

```rust
requires!["Health", "Mana", "AttackPower.base"]
```
Your game systems can then query for entities `With<StatRequirements>` and check `StatAccessor::is_stat_valid` or `StatAccessor::evaluate` for these required stats before performing actions.

## Tags

### Concept
Tags allow for fine-grained control over how modifiers apply and how stats are queried. They are useed with `Tagged` stat types. Tags are represented internally as `u32` bitmasks, allowing for combinations of tags.

For example, you might have tags for:
*   Damage types (Fire = 1, Cold = 2, Physical = 4)
*   Weapon types (Sword = 8, Axe = 16)
*   Skills or effects

A modifier might grant "+10% damage with Fire Swords". This would apply if the query or context has both the Fire tag and the Sword tag.

TODO Does not explain permissive vs strict. Does not explain tag categories (i.e., "fire" is "elemental")
### Usage in Stat Paths
Tags are appended to stat paths.
*   `"Damage.total"`: Evaluates total damage considering all relevant tags or untagged modifiers.
*   `"Damage.total.1"`: Evaluates total damage specifically for tag `1` (e.g., Fire).
*   `"Damage.increased.3"`: Adds/queries an "increased" modifier for `Damage` that has both tag `1` (Fire) AND tag `2` (Cold) (since 1 | 2 = 3).
*   `"Damage.base.Sword"`: If you have a string-to-tag mapping setup in your `Config` (not shown in current examples but a potential extension), you could use named tags. Otherwise, you'd use their numeric `u32` representation. The examples primarily use numeric tags.

When adding a modifier:
`stat_accessor.add_modifier(entity, "Damage.increased.1", 0.20); // 20% increased damage with tag 1 (Fire)`

When evaluating:
`let fire_damage = stat_accessor.evaluate(entity, "Damage.total.1");`

The `Tagged` stat type internally manages how these tagged modifiers combine.

## Stat Types: Flat vs Modifiable vs Complex vs Tagged

When you call `config.register_stat_type("MyStat", "TypeName")`, `TypeName` determines the underlying structure and behavior of "MyStat".

TODO specify that this is the only value that can be set, and is useful for values like 
"current life" that cannot typically be modified.
### `Flat`
*   **Concept**: The simplest type. Represents a single numerical value.
*   **Behavior**: Direct modifications (literals only) add to or subtract from its value.
*   **Use Case**: Current health, current mana (where the "max" is a separate stat), resource counts, level.
*   **Default `total_expression`**: Usually `"base"` (if it even needs one, often it's just its direct value).

### `Modifiable`
*   **Concept**: A base value that can be altered by a list of modifiers.
*   **Behavior**: Modifiers can be additive or multiplicative, determined by the `Config` for that stat part. It has a `base` value and a list of `mods` (expressions or literals).
    *   Additive: `final = base + sum_of_mods`
    *   Multiplicative: `final = base * product_of_mods`
*   **Use Case**: Max Health (base + flat bonuses + % bonuses), armor, resistance.
*   **Default `total_expression`**: Often `"base"` where `base` itself is calculated by applying modifiers. Or, the `Modifiable` struct is a *part* of a `Complex` stat.

### `Complex`
*   **Concept**: A stat whose total value is calculated by an arbitrary expression combining several named "parts". Each part is typically a `Modifiable` stat.
*   **Behavior**: You define a `total_expression` (e.g., `"base * (1 + base) * multiplier"`). Each variable in this expression (e.g., `base`, `base`) is a "part" that can have its own modifiers.
*   **Use Case**: Final damage calculations, complex skill effects, stats that depend on multiple sub-components.
*   **`total_expression` Example**: `"base * (1.0 + increased) * more"`

### `Tagged`
*   **Concept**: Similar to `Complex` in that it has parts combined by an expression, but crucially, its modifiers and parts can be associated with tags.
*   **Behavior**: Allows for querying the stat's value considering specific tags. For example, "total Fire damage" or "increased damage with Axes". Modifiers are added with specific tags, and evaluations can request values for specific tags or combinations.
*   **Use Case**: Elemental damages, damage types vs. weapon types, conditional bonuses.
*   **`total_expression` Example**: Similar to `Complex`, like `"base * (1.0 + increased) * more"`, but the values of `base`, `increased`, and `more` are themselves resolved considering the requested tags.

## Modifier Sets

### The `ModifierSet` Struct
A `bevy_gauge::prelude::ModifierSet` is a collection of stat modifications (base values, modifiers) that can be applied together. It's primarily used by `StatsInitializer` to set up an entity's initial stats. It can also be used with `StatEffect`.

TODO We do not currently support ".Fire" for tags. i.e., we do not support string -> u32 conversion
We probably should though.
A user defined tag could look like "Damage.increased.FIRE|AXE" for example
```rust
use bevy_gauge::prelude::*;

let mut modifiers = ModifierSet::default();
modifiers.add("Health.base", 100.0);
modifiers.add("Strength.base", Expression::new("10.0 + "Level" * 2.0").unwrap());
modifiers.add("Damage.increased.Fire", 0.25); // 25% increased Fire damage
```

### The `modifier_set!` Macro
The `modifier_set!` macro provides a more concise way to define a `ModifierSet`.

```rust
use bevy_gauge::prelude::*;

let modifiers = modifier_set! {
    "Health.base" => 100.0,
    "Mana.base" => 50.0,
    "AttackPower.base" => [
        10.0, // Add a flat 10
        "Strength.total * 0.5", // And add 50% of Strength
    ],
    "Damage.increased.Fire" => 0.15 // 15% increased Fire damage (tag is numeric)
};

// Usage with StatsInitializer:
// commands.spawn((Stats::new(), StatsInitializer::new(modifiers)));
```
This is very useful for defining reusable sets of modifiers, like for character classes or item templates.

## Stat Effects

### The `StatEffect` Trait
The `bevy_gauge::prelude::StatEffect` trait allows you to encapsulate a set of stat changes (and potentially other game logic) into a reusable unit, like a buff, debuff, or skill effect.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;
use std::fmt::Debug; // Required for deriving Debug

// Example: A simple Might buff
#[derive(Debug)] // StatEffect requires Debug
pub struct MightBuff {
    strength_bonus: f32,
    duration: f32, // You'd handle duration logic outside StatEffect typically
}

impl StatEffect for MightBuff {
    type Context = Entity; // This effect applies to a single entity

    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = *context;
        stat_accessor.add_modifier(target_entity, "Strength.base", self.strength_bonus);
        // You could also add modifiers to other stats, e.g., "AttackPower"
    }

    fn remove(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = *context;
        stat_accessor.remove_modifier(target_entity, "Strength.base", self.strength_bonus);
    }
}
```
The `Context` associated type allows effects to require more complex information (e.g., source entity, random number generators) if needed, by defining a custom context struct that implements `StatEffectContext`.

### Applying and Removing Effects
You typically apply and remove effects through your own game systems, often in response to events or conditions.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

// Assume MightBuff from previous example
fn cast_might_buff_system(
    mut commands: Commands,
    mut stat_accessor: StatAccessor,
    player_query: Query<Entity, With<PlayerTag>>,
    // In a real game, you'd likely have an event or input trigger this
) {
    if let Ok(player_entity) = player_query.get_single() {
        let buff = MightBuff { strength_bonus: 5.0, duration: 10.0 };

        // Apply the effect
        buff.apply(&mut stat_accessor, &player_entity);

        // In a real system, you'd store the active buff and its expiry time
        // on the player entity to remove it later.
        // For example:
        // commands.entity(player_entity).insert(ActiveBuff { effect: buff, expires_at: ... });
    }
}

// System to remove expired buffs (simplified)
fn buff_expiry_system(
    mut commands: Commands,
    mut stat_accessor: StatAccessor,
    // Query for entities with active buffs and their expiry times
    // active_buffs_query: Query<(Entity, &ActiveBuff)>,
) {
    // ... logic to find expired buffs ...
    // if buff.is_expired() {
    //     buff.effect.remove(&mut stat_accessor, &entity);
    //     commands.entity(entity).remove::<ActiveBuff>();
    // }
}
```
`ModifierSet` also implements `StatEffect`, so you can directly apply a `ModifierSet` to an entity.

## Stat Derived Components

Sometimes, you want Bevy components whose fields are directly derived from an entity's stats, and in some cases, you might want changes to those component fields to write back to the stats. `bevy_gauge` provides traits and a macro for this.

### The `StatDerived` Trait
This trait is for components whose fields should be populated from an entity's `Stats`. It defines methods to:
*   Create an instance from `Stats` (`from_stats`).
*   Check if the component needs updating based on current `Stats` (`should_update`).
*   Update its fields from `Stats` (`update_from_stats`).
*   Check if the required stats are valid (`is_valid`).

### The `WriteBack` Trait
This trait is for `StatDerived` components that also need to write some of their field values *back* to the entity's `Stats`.
*   `write_back`: Takes the component's current state and uses `StatAccessor` to update the underlying stats (typically using `set`).

### The `stat_component!` Macro
This macro simplifies the creation of components that implement `StatDerived` and optionally `WriteBack`.

**Example:**

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

stat_component!(
    #[derive(Default, Debug)] // You can add other derives here
    pub struct Life {
        max: <- "Life",             // Read-only from "Life" stat (likely "Life.total")
        current: <-> "CurrentLife", // Read-write to "CurrentLife" stat
    }
);
```

**What it generates (approximately):**
TODO Change stat_component! to use "get" instead of "evaluate"
TODO Change stat_component! to allow the user to define custom derives as per the above example
```rust
// This is what the stat_component! macro would generate for the Life struct above:

#[derive(::bevy::prelude::Component, ::std::default::Default, ::std::fmt::Debug)]
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
        // Note: The macro usually uses .get() which returns a Result,
        // or .evaluate() if it's about the final value.
        // The example uses .get(), implying it might be fetching a specific part or base value.
        // For a "total" value, you'd typically use stats.evaluate("StatName.total")
        self.max != stats.evaluate(stats.entity, "Life") // Assuming "Life" resolves to "Life.total"
            || self.current != stats.evaluate(stats.entity, "CurrentLife") // Assuming "CurrentLife" resolves to "CurrentLife.total"
    }

    fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::Stats) {
        self.max = stats.evaluate(stats.entity, "Life");
        self.current = stats.evaluate(stats.entity, "CurrentLife");
    }

    // is_valid would check if the underlying stat paths are defined/configured
    fn is_valid(stats: &bevy_gauge::prelude::Stats) -> bool {
        // This check is more about whether the stat *definitions* exist
        // and can be evaluated, rather than just being non-zero.
        // A more robust check might involve trying to evaluate and seeing if it errors,
        // or checking against the Config.
        stats.can_evaluate(stats.entity, "Life") && stats.can_evaluate(stats.entity, "CurrentLife")
    }
}

impl WriteBack for Life {
    fn write_back(&self, target_entity: Entity, stat_accessor: &mut bevy_gauge::prelude::StatAccessor) {
        // Write back self.current to the "CurrentLife" stat's base.
        let _ = stat_accessor.set(target_entity, "CurrentLife", self.current);
        // self.max is read-only (<-), so it's not written back.
    }
}
```

**Explanation of `<-` and `<->`:**
*   `fieldName: <- "StatPath"`: The `fieldName` will be populated from `StatPath`. It's read-only; changes to this field in the component won't affect the underlying stat. With that in mind, `<-` fields should only be changed via stat changes.
*   `fieldName: <-> "StatPath"`: The `fieldName` is populated from `StatPath`, AND if the `WriteBack` trait is implemented (which `stat_component!` does for `<->` fields), changes to this field in the component will be written back to the `StatPath` (typically its base value) when the `write_back_stats` system runs.

`bevy_gauge` includes systems (`update_derived_stats` and `write_back_stats`) that run at different stages in the Bevy schedule to keep these components synchronized with the `Stats` component. You'll need to ensure the `bevy_gauge::app_extension::plugin` is added to your app, which `bevy_gauge::plugin` does by default.
```

This guide should cover the main aspects of `bevy_gauge`. Let me know if you'd like any section expanded or clarified! 