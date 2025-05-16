//! `bevy_gauge` is a flexible stat system for the Bevy game engine.
//!
//! It allows for defining complex character or item statistics with features like:
//! - Configurable stat types (e.g., flat values, tagged modifiers, modifiable bases).
//! - Expression-based calculations for total stat values.
//! - Modifiers that can be additive or multiplicative.
//! - Tagging system for fine-grained control over which modifiers apply.
//! - Dependencies between stats, including stats from different entities (sources).
//! - Caching of evaluated stat values for performance.
//! - Automatic cache invalidation when underlying values or dependencies change.
//! - Integration with Bevy's component system, allowing components to derive their fields
//!   from stats or write their values back to stats.
//!
//! # Quick Start
//!
//! 1.  **Add the plugin:**
//!     ```no_run
//!     use bevy::prelude::*;
//!     use bevy_gauge::prelude::*;
//!
//!     fn main() {
//!         App::new()
//!             .add_plugins(DefaultPlugins)
//!             .add_plugins(bevy_gauge::plugin) // Add this line
//!             // ... other app setup ...
//!             .run();
//!     }
//!     ```
//!
//! 2.  **Configure stats:** Create a `Config` resource and register your stat types
//!     and how they are calculated.
//!     ```no_run
//!     use bevy::prelude::*;
//!     use bevy_gauge::prelude::*;
//!
//!     fn setup_stats(mut config: ResMut<Config>) {
//!         config.register_stat_type("Health", "Modifiable"); // Max health
//!         config.register_total_expression("Health", "base"); // Total is just its base
//!
//!         config.register_stat_type("Damage", "Tagged");
//!         config.register_total_expression("Damage", "base * (1.0 + increased) * more");
//!     }
//!
//!     fn main() {
//!         App::new()
//!             .add_plugins(DefaultPlugins)
//!             .add_plugins(bevy_gauge::plugin)
//!             .init_resource::<Config>() // Initialize the Config resource
//!             .add_systems(Startup, setup_stats) // Configure stats at startup
//!             // ...
//!             .run();
//!     }
//!     ```
//!
//! 3.  **Add `Stats` component to entities:**
//!     ```no_run
//!     # use bevy::prelude::*;
//!     # use bevy_gauge::prelude::*;
//!     fn spawn_player(mut commands: Commands) {
//!         commands.spawn((PlayerTag, Stats::new()));
//!     }
//!     #[derive(Component)]
//!     # struct PlayerTag;
//!     ```
//!
//! 4.  **Interact with stats using `StatAccessor` in systems:**
//!     ```no_run
//!     # use bevy::prelude::*;
//!     # use bevy_gauge::prelude::*;
//!     #[derive(Component)]
//!     # struct PlayerTag;
//!     # fn spawn_player(mut commands: Commands) { commands.spawn((PlayerTag, Stats::new())); }
//!     fn apply_damage_buff(mut stat_accessor: StatAccessor, query: Query<Entity, With<PlayerTag>>) {
//!         if let Ok(player_entity) = query.get_single() {
//!             // Add a 20% increased damage modifier with tag 1 (e.g., "Fire")
//!             stat_accessor.add_modifier(player_entity, "Damage.increased.1", 0.20);
//!         }
//!     }
//!
//!     fn print_player_damage(stat_accessor: StatAccessor, query: Query<Entity, With<PlayerTag>>) {
//!         if let Ok(player_entity) = query.get_single() {
//!             // Evaluate total damage (no specific tag, so considers all relevant tags)
//!             let total_damage = stat_accessor.evaluate(player_entity, "Damage");
//!             // Evaluate fire damage (tag 1)
//!             let fire_damage = stat_accessor.evaluate(player_entity, "Damage.1");
//!             println!("Player Total Damage: {}, Fire Damage: {}", total_damage, fire_damage);
//!         }
//!     }
//!     ```
//!
//! Check the `prelude` module for the most commonly used items.
//! The `StatAccessor` is the main entry point for interacting with entity stats from systems.
//! The `Config` resource is used for initial setup.
#![feature(sync_unsafe_cell)]
#![feature(associated_type_defaults)]

// TODO track dependencies both ways

// TODO Rewrite Tagged stats to support query caching

use prelude::*;
use bevy::prelude::*;

pub mod app_extension;
pub mod dirty;
pub mod expressions;
pub mod initializer;
pub mod konfig;
pub mod macros;
pub mod modifier_set;
pub mod prelude;
pub mod sources;
pub mod stat_accessor;
pub mod stat_addressing;
pub mod stat_derived;
pub mod stat_effect;
pub mod stat_error;
pub mod stat;
pub mod stat_requirements;
pub mod stat_types;
pub mod stats_component;
pub mod systems;
pub mod tags;

/// The main Bevy plugin for `bevy_gauge`.
///
/// Adds the necessary systems, resources, and configurations to integrate the stat system
/// into a Bevy application.
/// This includes setting up:
/// - An observer to clean up stats when entities with a `Stats` component are removed.
/// - The `app_extension::plugin` for custom schedules and derived/write-back component helpers.
///
/// This plugin should be added to your Bevy `App` for the stat system to function.
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_gauge::plugin as BevyGaugePlugin;
///
/// fn main() {
///     App::new()
///         .add_plugins(DefaultPlugins)
///         .add_plugins(bevy_gauge::plugin)
///         // ... other app setup ...
///         .run();
/// }
/// ```
pub fn plugin(app: &mut App) {
    app.add_observer(remove_stats)
    .add_observer(apply_stats_initializer)
    .add_plugins(app_extension::plugin);
}