
use bevy::app::App;

pub mod app_extension;
pub mod stats;
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

pub fn plugin(app: &mut App) {
    app.add_plugins((
        schedule::plugin,
        stats::plugin,
    ))
    .register_type::<prelude::StatContext>();
}