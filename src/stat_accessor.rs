use bevy::{ecs::system::SystemParam, prelude::*, utils::HashSet};
use super::prelude::*;

// TODO:  
// 1. Some way to set a stat
// 2. Some way to initialize a Stats component

// SystemParam for accessing stats from systems
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    query: Query<'w, 's, &'static mut Stats>,
    config: Res<'w, Config>,
}

impl StatAccessor<'_, '_> {
    pub fn get(&self, target_entity: Entity, path: &str) -> f32 {
        let Ok(stats) = self.query.get(target_entity) else {
            return 0.0;
        };

        stats.get(path)
    }

    pub fn set(&mut self, target_entity: Entity, path: &str, value: f32) {
        let Ok(mut stats) = self.query.get_mut(target_entity) else {
            return;
        };

        stats.set(path, value);
    }
    
    pub fn get_stats(&self, target_entity: Entity) -> Result<&Stats, ()> {
        let Ok(stats) = self.query.get(target_entity) else {
            return Err(());
        };

        Ok(stats)
    }

    pub fn add_modifier<V: Into<ModifierType>>(&mut self, target_entity: Entity, path: &str, modifier: V) {
        let vt = modifier.into();
        self.add_modifier_value(target_entity, path, vt);
    }

    pub fn add_modifier_value(&mut self, target_entity: Entity, path: &str, modifier: ModifierType) {
        let path = StatPath::parse(path);

        if !self.query.contains(target_entity) {
            return;
        }
        
        if let ModifierType::Expression(ref expression) = modifier {
            // example entry: "Master@Life", master_entity, "Life"
            // i.e., map entities the modified_entity is dependent on to the stat modified_entity is dependent on, 
            // and the final path inside of the cached_stats of modified_entity.
            let mut dependencies_info = Vec::new();

            // example entry: master_entity, "Life", servant_entity
            //                servant_entity, "Life.Added", "Strength"
            let mut dependents_to_add = Vec::new();
            
            // First gather dependency information
            if let Ok(target_stats) = self.query.get(target_entity) {
                for depends_on in expression.compiled.iter_variable_identifiers() {
                    if depends_on.contains('@') {
                        let parts: Vec<&str> = depends_on.split('@').collect();
                        let entity_name = parts[0];
                        let dependency_path = parts[1];
                        
                        if let Some(&depends_on_entity) = target_stats.sources.get(entity_name) {
                            dependencies_info.push((
                                depends_on.to_string(),
                                depends_on_entity,
                                dependency_path.to_string(),
                            ));
                            
                            dependents_to_add.push((
                                depends_on_entity,
                                dependency_path.to_string(),
                                DependentType::EntityStat(target_entity),
                            ));
                        }
                    } else {
                        dependents_to_add.push((
                            target_entity,
                            depends_on.to_string(),
                            DependentType::LocalStat(path.to_string())
                        ));
                    }
                }
            }
            
            // Cache dependency values
            let dependencies_to_cache = dependencies_info
                .iter()
                .filter_map(|(depends_on, depends_on_entity, dependency_path)| {
                    if let Ok(depends_on_stats) = self.query.get(*depends_on_entity) {
                        let value = depends_on_stats.evaluate_by_string(dependency_path);
                        Some((depends_on.clone(), value))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            
            if let Ok(target_stats) = self.query.get(target_entity) {
                for (depends_on, value) in dependencies_to_cache {
                    target_stats.cache_stat(&depends_on, value);
                }
            }
            
            // Register dependents
            for (depends_on_entity, dependency_path, dependent_type) in dependents_to_add {
                if let Ok(mut depends_on_stats) = self.query.get_mut(depends_on_entity) {
                    depends_on_stats.add_dependent(&dependency_path, dependent_type);
                }
            }
        }

        if let Ok(mut target_stats) = self.query.get_mut(target_entity) {
            target_stats.add_modifier_value(&path, modifier, &self.config);
        }

        self.update_stat(target_entity, &path.full_path);
    }

    pub fn remove_modifier<V: Into<ModifierType>>(&mut self, target_entity: Entity, path: &str, modifier: V) {
        let vt = modifier.into();
        self.remove_modifier_value(target_entity, path, &vt);
    }

    pub fn remove_modifier_value(&mut self, target_entity: Entity, path: &str, modifier: &ModifierType) {
        let path = StatPath::parse(path);
        
        // First, collect all the dependencies to remove
        let mut dependencies_to_remove = Vec::new();
        
        {
            let target_stats = match self.query.get(target_entity) {
                Ok(stats) => stats,
                Err(_) => return,
            };
            
            if let ModifierType::Expression(expression) = modifier {
                for depends_on in expression.compiled.iter_variable_identifiers() {
                    let depends_on = StatPath::parse(depends_on);
                    if let Some(head) = depends_on.target {
                        let dependency_path = depends_on.full_path; // "Life_Added"
                        
                        if let Some(&depends_on_entity) = target_stats.sources.get(head) {
                            dependencies_to_remove.push((
                                depends_on_entity,
                                dependency_path.to_string(),
                                DependentType::EntityStat(target_entity)
                            ));
                        }
                    } else {
                        // Remove local stat dependency
                        dependencies_to_remove.push((
                            target_entity,
                            depends_on.to_string(),
                            DependentType::LocalStat(path.to_string())
                        ));
                    }
                }
            }
        }
        
        // Now remove all dependencies
        for (depends_on_entity, dependency_path, dependent_type) in dependencies_to_remove {
            if let Ok(mut depends_on_stats) = self.query.get_mut(depends_on_entity) {
                depends_on_stats.remove_dependent(&dependency_path, dependent_type);
            }
        }

        // Finally remove the modifier itself
        if let Ok(mut target_stats) = self.query.get_mut(target_entity) {
            target_stats.remove_modifier_value(&path, modifier);
        }

        self.update_stat(target_entity, &path.full_path);
    }

    // This is filthy
    pub fn register_source(&mut self, target_entity: Entity, name: &str, source_entity: Entity) {
        // Early return if target doesn't exist
        if !self.query.contains(target_entity) {
            return;
        }

        // Step 1: Collect all updates we need to make
        let (updates, stats_to_update) = self.collect_source_updates(target_entity, name, source_entity);
        if updates.is_empty() {
            return;
        }

        // Step 2: Apply all dependency updates
        self.apply_dependency_updates(&updates);

        // Step 3: Update all affected stats
        for stat in stats_to_update {
            self.update_stat(stat.entity, &stat.path);
        }
    }

    fn collect_source_updates(
        &mut self,
        target_entity: Entity,
        name: &str,
        source_entity: Entity,
    ) -> (Vec<DependencyUpdate>, Vec<StatUpdate>) {
        let Ok(mut stats) = self.query.get_mut(target_entity) else {
            return (Vec::new(), Vec::new());
        };

        let mut dependency_updates = Vec::new();
        let mut stats_to_update = Vec::new();

        // Get old source for cleanup
        let old_source = stats.sources.get(name).cloned();
        
        // Update source mapping
        stats.sources.insert(name.to_string(), source_entity);

        // Collect updates for all dependent stats
        for (stat, dependents) in stats.get_dependents() {
            // Only process stats that reference this source
            if !stat.contains('@') || !stat.starts_with(name) {
                continue;
            }

            let Some(dependency_path) = stat.split('@').nth(1) else {
                continue;
            };

            // Add new dependency
            dependency_updates.push(DependencyUpdate {
                source_entity,
                path: dependency_path.to_string(),
                target_entity,
            });

            // Collect stats that need updating
            for (dependent, _) in dependents {
                if let DependentType::LocalStat(path) = dependent {
                    stats_to_update.push(StatUpdate {
                        entity: target_entity,
                        path: path.clone(),
                    });
                }
            }

            // Handle old source cleanup if needed
            if let Some(old_source_entity) = old_source {
                if old_source_entity != source_entity {
                    dependency_updates.push(DependencyUpdate {
                        source_entity: old_source_entity,
                        path: dependency_path.to_string(),
                        target_entity,
                    });
                }
            }
        }

        (dependency_updates, stats_to_update)
    }

    fn apply_dependency_updates(&mut self, updates: &[DependencyUpdate]) {
        for update in updates {
            if let Ok(mut source_stats) = self.query.get_mut(update.source_entity) {
                source_stats.add_dependent(
                    &update.path,
                    DependentType::EntityStat(update.target_entity)
                );
            }
        }
    }

    pub fn evaluate(&self, target_entity: Entity, path: &str) -> f32 {
        if let Ok(stats) = self.query.get(target_entity) {
            stats.evaluate_by_string(path)
        } else {
            0.0
        }
    }

    pub fn update_stat(&mut self, target_entity: Entity, stat_path: &str) {
        let mut processed = HashSet::new();
        self.update_stat_recursive(target_entity, stat_path, &mut processed);
    }

    fn update_stat_recursive(
        &mut self,
        target_entity: Entity,
        stat_path: &str,
        processed: &mut HashSet<(Entity, String)>
    ) {
        let process_key = (target_entity, stat_path.to_string());
        if !processed.insert(process_key) {
            // Already processed this combination
            return;
        }

        // Step 1: Calculate new value and update cache
        let current_value = {
            let Ok(stats) = self.query.get(target_entity) else {
                return;
            };
            let path = StatPath::parse(stat_path);
            let value = stats.evaluate(&path);
            stats.set_cached(stat_path, value);
            value
        };

        // Step 2: Collect all updates needed
        let updates = self.collect_dependent_updates(target_entity, stat_path);
        if updates.is_empty() {
            return;
        }

        // Step 3: Process all updates
        self.process_dependent_updates(updates, processed);
    }

    fn collect_dependent_updates(
        &self,
        target_entity: Entity,
        stat_path: &str,
    ) -> Vec<StatUpdate> {
        let Ok(stats) = self.query.get(target_entity) else {
            return Vec::new();
        };

        let mut updates = Vec::new();

        // Handle local stat dependencies
        for dependent in stats.get_stat_dependents(stat_path) {
            match dependent {
                DependentType::LocalStat(path) => {
                    updates.push(StatUpdate {
                        entity: target_entity,
                        path,
                    });
                },
                DependentType::EntityStat(dependent_entity) => {
                    // Find all prefixes that reference this entity in the dependent
                    if let Ok(dependent_stats) = self.query.get(dependent_entity) {
                        for (prefix, &source_entity) in dependent_stats.sources.iter() {
                            if source_entity == target_entity {
                                // Create the cache key that needs updating
                                let cache_key = format!("{}@{}", prefix, stat_path);
                                
                                // Find all stats that depend on this cache entry
                                for cache_dependent in dependent_stats.get_stat_dependents(&cache_key) {
                                    if let DependentType::LocalStat(dependent_path) = cache_dependent {
                                        updates.push(StatUpdate {
                                            entity: dependent_entity,
                                            path: dependent_path,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        updates
    }

    fn process_dependent_updates(
        &mut self,
        updates: Vec<StatUpdate>,
        processed: &mut HashSet<(Entity, String)>
    ) {
        for update in updates {
            self.update_stat_recursive(
                update.entity,
                &update.path,
                processed
            );
        }
    }

    pub fn apply_modifier_set(&mut self, target_entity: Entity, modifier_set: &ModifierSet) {
        modifier_set.apply(self, &target_entity);
    }

    pub fn remove_modifier_set(&mut self, target_entity: Entity, modifier_set: &ModifierSet) {
        modifier_set.remove(self, &target_entity);
    }

    // TODO make me more efficient
    pub fn remove_stat_entity(&mut self, target_entity: Entity) {
        let Ok(target_stats) = self.query.get(target_entity) else {
            return;
        };

        let mut dependent_entities = Vec::new();
        let dependents = target_stats.get_dependents();
        for (stat, stat_dependents) in dependents.iter() {
            for (dependent, _) in stat_dependents.iter() {
                let DependentType::EntityStat(dependent_entity) = dependent else {
                    continue;
                };

                dependent_entities.push((stat, dependent_entity));
            }
        }

        let mut stat_dependencies = Vec::new();
        for (stat, &dependent_entity) in dependent_entities {
            let Ok(dependent_stats) = self.query.get(dependent_entity) else {
                return;
            };

            for (source_name, &source_entity) in dependent_stats.sources.iter() {
                if source_entity == target_entity {
                    let cache_key = format!("{}@{}", source_name, stat);
                    dependent_stats.remove_cached(&cache_key);
                    stat_dependencies.push((dependent_entity, cache_key));
                }
            }
        }

        for (dependent_entity, cache_key) in stat_dependencies {
            self.update_stat(dependent_entity, &cache_key);
        }
    }
}

/// Represents a change in dependency relationship between stats of different entities.
/// Used when registering or updating entity sources to track what dependencies need to be added or removed.
#[derive(Debug, Clone)]
struct DependencyUpdate {
    /// The entity whose stat is being depended on
    source_entity: Entity,
    /// The path of the stat within the source entity
    path: String,
    /// The entity that depends on the source
    target_entity: Entity,
}

impl DependencyUpdate {
    /// Creates a new dependency update for adding a dependency
    fn new_add(source_entity: Entity, path: &str, target_entity: Entity) -> Self {
        Self {
            source_entity,
            path: path.to_string(),
            target_entity,
        }
    }

    /// Creates a new dependency update for removing a dependency
    fn new_remove(source_entity: Entity, path: &str, target_entity: Entity) -> Self {
        Self {
            source_entity,
            path: path.to_string(),
            target_entity,
        }
    }
}

/// Represents a stat that needs to be recalculated due to dependency changes.
/// Used to track which stats need to be updated after dependency changes are applied.
#[derive(Debug, Clone)]
struct StatUpdate {
    /// The entity whose stat needs updating
    entity: Entity,
    /// The path of the stat to update
    path: String,
}

impl StatUpdate {
    /// Creates a new stat update
    fn new(entity: Entity, path: &str) -> Self {
        Self {
            entity,
            path: path.to_string(),
        }
    }
}

pub(crate) fn remove_stats(
    trigger: Trigger<OnRemove, Stats>,
    mut stat_accessor: StatAccessor,
) {
    let removed_entity = trigger.entity();
    stat_accessor.remove_stat_entity(removed_entity);
}