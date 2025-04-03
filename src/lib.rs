use bevy::app::App;

pub mod app_extension;
pub mod attribute;
pub mod effects;
mod expressions;
pub mod macros;
mod modifier_events;
pub mod modifiers;
pub mod prelude;
pub mod requirements;
mod resource;
pub mod schedule;
mod stat_events;
mod stat_value;
mod stats;
pub mod systems;
pub mod tags;
pub mod traits;
mod value_type;

pub fn plugin(app: &mut App) {
    app.add_plugins((schedule::plugin,));
}
