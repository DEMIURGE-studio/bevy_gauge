
use bevy::app::App;

pub mod app_extension;
pub mod components;
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

pub fn plugin(app: &mut App) {
    app.add_plugins((
        schedule::plugin,
        components::plugin,
    ));
}