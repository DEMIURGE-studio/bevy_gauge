use bevy::ecs::relationship::Relationship;
use bevy::prelude::*;
use crate::prelude::*;
use crate::schedule::StatsMutation;
use bevy::platform::collections::HashMap as BevyHashMap;
use std::any::TypeId;

#[derive(Resource, Default)]
pub(crate) struct StatRelationshipAliases(BevyHashMap<TypeId, &'static str>);

fn derive_alias<R: 'static>() -> &'static str {
    let full = std::any::type_name::<R>();
    let last = full.rsplit("::").next().unwrap_or(full);
    match last.find('<') { Some(pos) => &last[..pos], None => last }
}

fn get_alias_for<R: 'static>(aliases: &StatRelationshipAliases) -> &'static str {
    aliases.0.get(&TypeId::of::<R>()).copied().unwrap_or_else(|| derive_alias::<R>())
}

pub(crate) fn on_edge_changed_system<R>(
    q: Query<(Entity, &R), (With<Stats>, Changed<R>)>,
    mut stats_mutator: StatsMutator,
    aliases: Res<StatRelationshipAliases>,
) where
    R: Relationship + Component + Send + Sync + 'static,
{
    let alias = get_alias_for::<R>(&aliases);
    for (entity, edge) in &q {
        if let Ok(stats_ro) = stats_mutator.get_stats(entity) {
            if let Some(&old_source) = stats_ro.sources.get(alias) {
                let src = edge.get();
                if old_source != src {
                    stats_mutator.unregister_source(entity, alias);
                }
            }
        }
        stats_mutator.register_source(entity, alias, edge.get());
    }
}

pub(crate) fn on_edge_removed_system<R>(
    mut removed: RemovedComponents<R>,
    mut stats_mutator: StatsMutator,
    aliases: Res<StatRelationshipAliases>,
) where
    R: Component + Send + Sync + 'static,
{
    let alias = get_alias_for::<R>(&aliases);
    for entity in removed.read() { stats_mutator.unregister_source(entity, alias); }
}

pub trait StatsAppSourcesExt {
    fn register_stat_relationship<R>(&mut self) -> &mut Self
    where R: Relationship + Component + Send + Sync + 'static;

    fn register_stat_relationship_as<R>(&mut self, alias: &'static str) -> &mut Self
    where R: Relationship + Component + Send + Sync + 'static;

    fn register_stat_relationship_with<R>(&mut self, extractor: fn(&R) -> Entity) -> &mut Self
    where R: Component + Send + Sync + 'static;

    fn register_stat_relationship_as_with<R>(&mut self, alias: &'static str, extractor: fn(&R) -> Entity) -> &mut Self
    where R: Component + Send + Sync + 'static;
}

impl StatsAppSourcesExt for App {
    fn register_stat_relationship<R>(&mut self) -> &mut Self
    where R: Relationship + Component + Send + Sync + 'static {
        let alias = derive_alias::<R>();
        StatsAppSourcesExt::register_stat_relationship_as::<R>(self, alias)
    }

    fn register_stat_relationship_as<R>(&mut self, alias: &'static str) -> &mut Self
    where R: Relationship + Component + Send + Sync + 'static {
        self.init_resource::<StatRelationshipAliases>();
        {
            let world = self.world_mut();
            let mut aliases_map = world.resource_mut::<StatRelationshipAliases>();
            aliases_map.0.insert(TypeId::of::<R>(), alias);
        }
        self.add_systems(StatsMutation, on_edge_changed_system::<R>);
        self.add_systems(StatsMutation, on_edge_removed_system::<R>);
        self
    }

    fn register_stat_relationship_with<R>(&mut self, extractor: fn(&R) -> Entity) -> &mut Self
    where R: Component + Send + Sync + 'static {
        let alias = derive_alias::<R>();
        StatsAppSourcesExt::register_stat_relationship_as_with::<R>(self, alias, extractor)
    }

    fn register_stat_relationship_as_with<R>(&mut self, alias: &'static str, extractor: fn(&R) -> Entity) -> &mut Self
    where R: Component + Send + Sync + 'static {
        self.init_resource::<StatRelationshipAliases>();
        {
            let world = self.world_mut();
            let mut aliases_map = world.resource_mut::<StatRelationshipAliases>();
            aliases_map.0.insert(TypeId::of::<R>(), alias);
        }
        self.add_systems(StatsMutation, move |q: Query<(Entity, &R), (With<Stats>, Changed<R>)>, mut stats_mutator: StatsMutator, aliases: Res<StatRelationshipAliases>| {
            let alias = get_alias_for::<R>(&aliases);
            for (entity, edge) in &q {
                let src = extractor(edge);
                if let Ok(stats_ro) = stats_mutator.get_stats(entity) {
                    if let Some(&old_source) = stats_ro.sources.get(alias) {
                        if old_source != src { stats_mutator.unregister_source(entity, alias); }
                    }
                }
                stats_mutator.register_source(entity, alias, src);
            }
        });

        self.add_systems(StatsMutation, move |mut removed: RemovedComponents<R>, mut stats_mutator: StatsMutator, aliases: Res<StatRelationshipAliases>| {
            let alias = get_alias_for::<R>(&aliases);
            for entity in removed.read() { stats_mutator.unregister_source(entity, alias); }
        });

        self
    }
}