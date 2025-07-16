use bevy::prelude::*;

use crate::prelude::Stats;

/// A marker component that tracks when an entity's Stats have been modified.
/// This component is automatically added/updated when stats change, triggering
/// Bevy's change detection system without conflicting with StatsMutator ownership.
#[derive(Component, Default)]
pub struct StatsProxy(bool);

/// System that processes StatsChanged events and updates StatsProxy components.
/// This runs at the end of the StatsMutation schedule, ensuring all stat changes
/// in that schedule are captured before the UpdateStatDerived schedule runs.
pub fn update_stats_proxy_system(
    mut query: Query<&mut StatsProxy, Changed<Stats>>
) {
    for mut proxy in query.iter_mut() {
        proxy.0 = true;
    }
}

/// Plugin function to register the StatsProxy system and event.
/// This should be called during app setup to enable the proxy functionality.
pub fn plugin(app: &mut App) {
    app.add_systems(crate::app_extension::StatsMutation, update_stats_proxy_system);
}
