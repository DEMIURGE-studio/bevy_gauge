use bevy::app::App;

pub mod app_extension;
pub mod attribute;
pub mod effects;
pub mod macros;
pub mod modifiers;
pub mod prelude;
pub mod requirements;
mod resource;
pub mod schedule;
mod stats;
pub mod systems;
pub mod tags;
pub mod traits;
mod value_type;
pub fn plugin(app: &mut App) {
    app.add_plugins((schedule::plugin,));
}
