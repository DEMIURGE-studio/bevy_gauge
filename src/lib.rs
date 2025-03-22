
use bevy::app::App;

pub mod app_extension;
pub mod dirty;
pub mod error;
pub mod eval_context;
pub mod macros;
pub mod prelude;
pub mod requirements;
pub mod schedule;
pub mod stat_definitions;
pub mod stat_effect;
pub mod stat_type;
pub mod systems;
pub mod traits;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        schedule::plugin,
        eval_context::plugin,
    ))
    .register_type::<prelude::StatContext>();
}