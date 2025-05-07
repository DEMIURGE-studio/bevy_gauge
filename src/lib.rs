#![feature(sync_unsafe_cell)]
#![feature(associated_type_defaults)]

// TODO track dependencies both ways

// TODO Rewrite Tagged stats to support query caching

use prelude::*;
use bevy::prelude::*;

pub mod app_extension;
pub mod dirty;
pub mod expressions;
pub mod macros;
pub mod modifier_set;
pub mod prelude;
pub mod sources;
pub mod stat_accessor;
pub mod stat_addressing;
pub mod stat_config;
pub mod stat_derived;
pub mod stat_effect;
pub mod stat_error;
pub mod stat;
pub mod stat_requirements;
pub mod stat_types;
pub mod stats_component;
pub mod systems;
pub mod tags;

pub fn plugin(app: &mut App) {
    app.add_observer(remove_stats)
    .add_plugins(app_extension::plugin);
}