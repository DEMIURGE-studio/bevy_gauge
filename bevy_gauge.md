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

**Important Note on Initialization Timing**:
When you spawn an entity with a `Stats` component (either directly or via `StatsInitializer`), and then intend to immediately modify or read its stats using `StatAccessor` *within the same system invocation*, be aware of Bevy's command queue. The `Stats` component only becomes visible to `StatAccessor`'s internal queries after Bevy's commands have been applied (flushed). 
This typically happens automatically between system stages or by explicitly calling `apply_deferred` after your spawning commands. `StatsInitializer` works seamlessly because its `OnAdd` trigger fires after the component is fully added and visible. For manual `StatAccessor` use on newly spawned entities within the same system, ensure proper ordering or use `apply_deferred`.

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
            Expression::new("Strength * 0.5").unwrap()
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
            Expression::new("Strength * 0.5").unwrap()
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
        let fire_damage = stat_accessor.evaluate(player_entity, "Damage.Fire"); // Assuming "Fire" tag
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
// The expression refers to `LeaderAlias@Strength`.
stat_accessor.add_modifier(
    minion_entity,
    "AttackPower.base",
    Expression::new(""LeaderAlias@Strength" * 0.1").unwrap() // 10% of leader's strength
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
    `"Strength@Leader * 0.5"`

This would fetch the `Strength` value from the entity registered as `"Leader"`.

## Stat Entity Destruction
When an entity with a `Stats` component is despawned, Bevy handles the removal of the `Stats` component itself. `bevy_gauge` includes an observer system (`remove_stats`) that cleans up any dependencies or source registrations related to the despawned entity, preventing dangling references or incorrect calculations.

## Stat Requirements

The `bevy_gauge::prelude::StatRequirements` component is a utility for game logic, not something that directly interacts with or is enforced by the stat calculation system itself. It allows you to define a list of conditions in the form of stat expressions (i.e., `"Strength >= 20"`) that your game systems can check to determine if an entity meets certain criteria. 

Think of it as a checklist an entity must pass for certain actions or states to be valid. Each requirement is usually a string that can be evaluated as a boolean expression.

### Using `StatRequirements`
You add the component to an entity, populating it with expressions that should evaluate to true for the requirements to be met.

```rust
use bevy::prelude::*;
use bevy_gauge::prelude::*;

fn spawn_mage(mut commands: Commands) {
    commands.spawn((
        Stats::new(),
        stats! {
            "Intelligence.base" => 15.0,
            "HasLearnedSpell_Fireball" => 1.0, // Using 1.0 for true
            "Mana.current" => 50.0
        },
        // This mage requires Intelligence of at least 20 and the Fireball spell.
        // It also needs at least 30 mana to cast a hypothetical powerful spell.
        requires![
            "Intelligence >= 20.0",
            "HasLearnedSpell_Fireball == 1.0",
            "Mana.current >= 30.0"
        ]
    ));
}
```

### The `requires!` Macro
The `requires!` macro is a convenient way to create a `StatRequirements` component with a list of requirement strings.

```rust
requires!["Strength >= 15", "Agility >= 10", "Level > 5"]
```
## Tags

### Concept
Tags allow for fine-grained control over how modifiers apply and how stats are queried, particularly with `Tagged` stat types. Tags are represented internally as `u32` bitmasks, allowing for combinations of tags by using bitwise operations (e.g., `FIRE | SWORD`). This system is powerful enough to create interactions similar to those found in games like Path of Exile, where modifiers can be general (e.g., "increased fire damage") or very specific (e.g., "increased fire damage with axes").

### Defining Tags
While you can define tags manually as `u32` constants (e.g., `const FIRE: u32 = 1; const AXE: u32 = 2;`), `bevy_gauge` provides a sophisticated macro, `define_tags!`, for this purpose, showcased in `src/tags.rs`. This macro allows you to define tags in hierarchical categories, automatically assigning unique bit values and generating constants for both individual tags and categories.

A conceptual example based on the structure in `src/tags.rs`:
```rust
// In your project's tag definition file (e.g., my_game_tags.rs)
stat_macros::define_tags! { // Assuming define_tags! is accessible
    damage_type {      // Defines DAMAGE_TYPE category
        elemental {    // Defines ELEMENTAL sub-category (part of DAMAGE_TYPE)
            fire,      // Defines FIRE tag (part of ELEMENTAL)
            cold,      // Defines COLD tag
            lightning  // Defines LIGHTNING tag
        },
        physical,      // Defines PHYSICAL tag (part of DAMAGE_TYPE)
        chaos,         // Defines CHAOS tag
    },
    weapon_type {      // Defines WEAPON_TYPE category
        melee {        // Defines MELEE sub-category (part of WEAPON_TYPE)
            sword,     // Defines SWORD tag
            axe        // Defines AXE tag
        },
        ranged {       // Defines RANGED sub-category
            bow,       // Defines BOW tag
            wand       // Defines WAND tag
        },
    },
}
// This would generate u32 constants like FIRE, AXE, ELEMENTAL, WEAPON_TYPE, etc.
```
Using `define_tags!` is highly recommended for managing complex tag relationships. `src/tags.rs` also includes constants like `TAG_CATEGORIES` which lists the top-level categories generated by the macro.

### Using Tags in Stat Paths
When defining stat paths for `Tagged` stats, you use the numeric `u32` values of your defined tags or their bitwise OR'd combinations.

*   **Path Structure**: `StatName.PartName.TagValue`
    *   Example: `"Damage.increased.FIRE"` (if `FIRE` is a `u32` constant).
    *   Example: `"Damage.more.FIRE|AXE"` (using bitwise OR for combined tags).

Currently, `StatPath::parse` expects numeric tag values. If you use string-based tags in your game logic (e.g., from data files), you'll need to convert them to their `u32` mask equivalents before constructing the stat path, potentially using helper functions or a map populated from your `define_tags!` output. The `src/tags.rs` file contains examples like `permissive_tag_from_str` and `build_permissive_mask` that can assist with such conversions.

### Tag Matching Logic
The core rule for determining if a tagged modifier applies during stat evaluation is:
**A modifier applies if all of the modifier's own tags are present in the tags used for the query.**

Let `modifier_tag` be the tag(s) the modifier was applied with, and `query_tag` be the tag(s) used when evaluating the stat. The condition is:
`(query_tag & modifier_tag) == modifier_tag`

**Implications & Examples:**

*   **General Modifiers, Specific Queries (Permissive Application)**: A broadly tagged modifier can apply to a more specific situation.
    *   Modifier: "+10% Fire Damage" (applied with tag `FIRE`).
    *   Query Context: Evaluating damage for an attack that is "Fire" and "Axe" (querying with `FIRE | AXE`).
    *   Logic: `((FIRE | AXE) & FIRE) == FIRE` which is `FIRE == FIRE` (True).
    *   Result: The "+10% Fire Damage" modifier applies.

*   **Specific Modifiers, General Queries (Strict Application)**: A very specific modifier will *not* apply to a general query.
    *   Modifier: "+15% Fire Damage with Axes" (applied with `FIRE | AXE`).
    *   Query Context: Evaluating generic "Fire Damage" (querying with `FIRE`).
    *   Logic: `(FIRE & (FIRE | AXE)) == (FIRE | AXE)` which is `FIRE == (FIRE | AXE)` (False, unless `AXE` is 0).
    *   Result: The "+15% Fire Damage with Axes" modifier does *not* apply.

*   **Specific Modifiers, Matching or More Specific Queries**:
    *   Modifier: "+15% Fire Damage with Axes" (applied with `FIRE | AXE`).
    *   Query Context: Evaluating "Fire Damage with Axes" (querying with `FIRE | AXE`).
    *   Result: Applies.
    *   Query Context: Evaluating "Fire Damage with Magic Axes" (querying with `FIRE | AXE | MAGIC_TAG`).
    *   Result: Applies.

### Permissive Tags and Categories
The `src/tags.rs` file also introduces concepts like `TAG_CATEGORIES` and helper functions (e.g., `build_permissive_tag`). These are designed to facilitate more complex "Path of Exile" style interactions where modifiers might apply based on broader categories. For instance, a modifier for "+X% Elemental Damage" (tagged with `ELEMENTAL`) should apply if the query involves `FIRE` (which is part of the `ELEMENTAL` category).

The function `build_permissive_tag(tag_from_query)` aims to construct an effective `query_tag` that considers these categories. When such a permissive query tag is used in the fundamental matching logic `(permissive_query_tag & modifier_tag) == modifier_tag`, it allows for these category-based applications. Understanding the implementation of `build_permissive_tag` in `src/tags.rs` is key to leveraging this advanced functionality.

For example, if `FIRE` is part of the `ELEMENTAL` category:
*   A modifier for "+Y% Elemental Damage" is applied with `ELEMENTAL`.
*   You perform an attack that is `FIRE | SWORD`.
*   You might generate a `permissive_query_tag` using `build_permissive_tag(FIRE | SWORD_TAG)`. This function would ensure that the resulting `permissive_query_tag` correctly includes bits that allow the `ELEMENTAL` modifier to match.

This system allows for a rich hierarchy of how modifiers apply, from very general (any elemental damage) to very specific (fire damage with an axe).

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
modifiers.add("Strength.base", Expression::new("10.0 + Level * 2.0").unwrap());
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
        "Strength * 0.5", // And add 50% of Strength
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
        max: <- "Life",             // Read-only from "Life" stat (likely "Life")
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
        // For a "total" value, you'd typically use stats.evaluate("StatName")
        self.max != stats.evaluate(stats.entity, "Life") // Assuming "Life" resolves to "Life"
            || self.current != stats.evaluate(stats.entity, "CurrentLife") // Assuming "CurrentLife" resolves to "CurrentLife"
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