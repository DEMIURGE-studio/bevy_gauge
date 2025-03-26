
use bevy::app::App;

pub mod app_extension;
pub mod attribute;
pub mod dirty;
pub mod eval_context;
pub mod macros;
pub mod prelude;
pub mod requirements;
pub mod schedule;
pub mod serialization;
pub mod stat_effect;
pub mod systems;
pub mod traits;
pub mod tags;
pub mod modifiers;
pub mod effects;
mod value_type;
mod resource;
mod stats;
mod tag_registry;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        schedule::plugin,
    ));
}