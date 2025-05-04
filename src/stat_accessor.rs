use bevy::{ecs::system::SystemParam, prelude::*, utils::HashSet};
use super::prelude::*;

// TODO:  
// 1. Some way to set a stat
// 2. Some way to initialize a Stats component

// SystemParam for accessing stats from systems
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    stats_query: Query<'w, 's, &'static mut Stats>,
}

impl StatAccessor<'_, '_> {
    pub fn get(&self, target_entity: Entity, stat_path: &str) -> f32 {
        let Ok(stats) = self.stats_query.get(target_entity) else {
            return 0.0;
        };

        stats.get(stat_path).unwrap_or(0.0)
    }

    pub fn set_base(&mut self, target_entity: Entity, stat_path: &str, value: f32) {
        let Ok(mut stats) = self.stats_query.get_mut(target_entity) else {
            return;
        };

        stats.set_base(stat_path, value);
    }
    
    pub fn get_stats(&self, target_entity: Entity) -> Result<&Stats, ()> {
        let Ok(stats) = self.stats_query.get(target_entity) else {
            return Err(());
        };

        Ok(stats)
    }

    pub fn add_modifier<V: Into<ValueType>>(&mut self, target_entity: Entity, stat_path: &str, modifier: V) {
        let vt = modifier.into();
        self.add_modifier_value(target_entity, stat_path, vt);
    }

    pub fn add_modifier_value(&mut self, target_entity: Entity, stat_path: &str, modifier: ValueType) {
        let stat_path = StatPath::parse(stat_path);

        if !self.stats_query.contains(target_entity) {
            return;
        }
        
        if let ValueType::Expression(ref expression) = modifier {
            // example entry: "Master@Life", master_entity, "Life"
            // i.e., map entities the modified_entity is dependent on to the stat modified_entity is dependent on, 
            // and the final path inside of the cached_stats of modified_entity.
            let mut dependencies_info = Vec::new();

            // example entry: master_entity, "Life", servant_entity
            //                servant_entity, "Life.Added", "Strength"
            let mut dependents_to_add = Vec::new();
            
            // First gather dependency information
            if let Ok(target_stats) = self.stats_query.get(target_entity) {
                for depends_on in expression.value.iter_variable_identifiers() {
                    if depends_on.contains('@') {
                        let parts: Vec<&str> = depends_on.split('@').collect();
                        let entity_name = parts[0];
                        let dependency_stat_path = parts[1];
                        
                        if let Some(&depends_on_entity) = target_stats.sources.get(entity_name) {
                            dependencies_info.push((
                                depends_on.to_string(),
                                depends_on_entity,
                                dependency_stat_path.to_string(),
                            ));
                            
                            dependents_to_add.push((
                                depends_on_entity,
                                dependency_stat_path.to_string(),
                                DependentType::EntityStat(target_entity),
                            ));
                        }
                    } else {
                        dependents_to_add.push((
                            target_entity,
                            depends_on.to_string(),
                            DependentType::LocalStat(stat_path.to_string())
                        ));
                    }
                }
            }
            
            // Cache dependency values
            let dependencies_to_cache = dependencies_info
                .iter()
                .filter_map(|(depends_on, depends_on_entity, dependency_stat_path)| {
                    if let Ok(depends_on_stats) = self.stats_query.get(*depends_on_entity) {
                        let value = depends_on_stats.evaluate_by_string(dependency_stat_path);
                        Some((depends_on.clone(), value))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            
            if let Ok(target_stats) = self.stats_query.get(target_entity) {
                for (depends_on, value) in dependencies_to_cache {
                    target_stats.cache_stat(&depends_on, value);
                }
            }
            
            // Register dependents
            for (depends_on_entity, dependency_stat_path, dependent_type) in dependents_to_add {
                if let Ok(depends_on_stats) = self.stats_query.get(depends_on_entity) {
                    depends_on_stats.add_dependent(&dependency_stat_path, dependent_type);
                }
            }
        }

        if let Ok(mut target_stats) = self.stats_query.get_mut(target_entity) {
            target_stats.add_modifier_value(&stat_path, modifier);
        }

        self.update_stat(target_entity, &stat_path);
    }

    pub fn remove_modifier<V: Into<ValueType>>(&mut self, target_entity: Entity, stat_path: &str, modifier: V) {
        let vt = modifier.into();
        self.remove_modifier_value(target_entity, stat_path, &vt);
    }

    pub fn remove_modifier_value(&mut self, target_entity: Entity, stat_path: &str, modifier: &ValueType) {
        let stat_path = StatPath::parse(stat_path);
        
        // First, collect all the dependencies to remove
        let mut dependencies_to_remove = Vec::new();
        
        {
            let target_stats = match self.stats_query.get(target_entity) {
                Ok(stats) => stats,
                Err(_) => return,
            };
            
            if let ValueType::Expression(expression) = modifier {
                for depends_on in expression.value.iter_variable_identifiers() {
                    let depends_on = StatPath::parse(depends_on);
                    if let Some(head) = &depends_on.owner {
                        let dependency_stat_path = &depends_on.path; // "Life_Added"
                        
                        if let Some(&depends_on_entity) = target_stats.sources.get(head) {
                            dependencies_to_remove.push((
                                depends_on_entity,
                                dependency_stat_path.to_string(),
                                DependentType::EntityStat(target_entity)
                            ));
                        }
                    } else {
                        // Remove local stat dependency
                        dependencies_to_remove.push((
                            target_entity,
                            depends_on.to_string(),
                            DependentType::LocalStat(stat_path.to_string())
                        ));
                    }
                }
            }
        }
        
        // Now remove all dependencies
        for (depends_on_entity, dependency_stat_path, dependent_type) in dependencies_to_remove {
            if let Ok(depends_on_stats) = self.stats_query.get(depends_on_entity) {
                depends_on_stats.remove_dependent(&dependency_stat_path, dependent_type);
            }
        }

        // Finally remove the modifier itself
        if let Ok(mut target_stats) = self.stats_query.get_mut(target_entity) {
            target_stats.remove_modifier_value(&stat_path, modifier);
        }

        self.update_stat(target_entity, &stat_path);
    }

    // This is filthy
    pub fn register_source(&mut self, target_entity: Entity, name: &str, source_entity: Entity) {
        // Check if entity exists before proceeding
        if !self.stats_query.contains(target_entity) {
            return;
        }
    
        // First, collect all the necessary information from the target_entity
        let mut stats_to_update = Vec::new();
        let mut deps_to_register = Vec::new();
        
        // Collect dependent stats and prepare source registration
        {
            if let Ok(mut stats) = self.stats_query.get_mut(target_entity) {
                // Store the old source if it exists (for potential cleanup later)
                let old_source = stats.sources.get(name).cloned();
                
                // Update the source
                stats.sources.insert(name.to_string(), source_entity);
                
                // Collect stats that need updating and dependencies to register
                for (stat, dependents) in stats.get_dependents() {
                    if stat.contains('@') && stat.starts_with(name) {
                        let parts: Vec<&str> = stat.split('@').collect();
                        if parts.len() > 1 {
                            // Add to list of dependencies to register with source entity
                            deps_to_register.push((source_entity, parts[1].to_string(), target_entity));
                            
                            // Collect stats that need to be updated
                            for (dependent, _) in dependents.iter() {
                                if let DependentType::LocalStat(dependent_stat) = dependent {
                                    stats_to_update.push(dependent_stat.clone());
                                }
                            }
                        }
                    }
                }
                
                // Remove dependencies from old source if it exists and is different
                if let Some(old_source_entity) = old_source {
                    if old_source_entity != source_entity {
                        for (stat, _) in stats.get_dependents() {
                            if stat.contains('@') && stat.starts_with(name) {
                                let parts: Vec<&str> = stat.split('@').collect();
                                if parts.len() > 1 {
                                    // Schedule removal of dependency from old source
                                    if let Ok(old_source_stats) = self.stats_query.get(old_source_entity) {
                                        old_source_stats.remove_dependent(
                                            &parts[1].to_string(), 
                                            DependentType::EntityStat(target_entity)
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Register dependencies with the source entity
        for (source_entity, stat_path, target_entity) in deps_to_register {
            if let Ok(source_stats) = self.stats_query.get(source_entity) {
                source_stats.add_dependent(
                    &stat_path,
                    DependentType::EntityStat(target_entity)
                );
            }
        }
        
        // Now update the cached values for stats that depend on the new source
        {
            if let Ok(stats) = self.stats_query.get(target_entity) {
                // Update cached values
                for stat in stats.get_dependents().keys() {
                    if stat.contains('@') && stat.starts_with(name) {
                        let parts: Vec<&str> = stat.split('@').collect();
                        if parts.len() > 1 {
                            // Get the value from the new source
                            let value = self.get(source_entity, parts[1]);
                            stats.cache_stat(stat, value);
                        }
                    }
                }
            }
        }
        
        // Finally, update all dependent stats
        for dependent_stat in stats_to_update {
            self.update_stat(target_entity, &StatPath::parse(&dependent_stat));
        }
    }

    pub fn evaluate(&self, target_entity: Entity, stat_path: &str) -> f32 {
        if let Ok(stats) = self.stats_query.get(target_entity) {
            stats.evaluate_by_string(stat_path)
        } else {
            0.0
        }
    }

    pub fn update_stat(&mut self, target_entity: Entity, stat_path: &StatPath) {
        let mut processed = HashSet::new();
        self.update_stat_recursive(target_entity, stat_path, &mut processed);
    }

    fn update_stat_recursive(&mut self, target_entity: Entity, stat_path: &StatPath, processed: &mut HashSet<(Entity, String)>) {
        let process_key = (target_entity, stat_path.to_string());
        
        if processed.contains(&process_key) {
            return;
        }
        
        processed.insert(process_key);
        
        // Calculate new value and update cache
        let current_value = if let Ok(stats) = self.stats_query.get(target_entity) {
            let value = stats.evaluate(stat_path);
            stats.set_cached(&stat_path.path, value);
            value
        } else {
            return; // Entity doesn't have stats, nothing to do
        };
        
        let mut local_dependents = Vec::new();
        let mut entity_dependents = Vec::new();
        
        if let Ok(stats) = self.stats_query.get(target_entity) {
            // Get all dependents for this stat
            for dependent in stats.get_stat_dependents(&stat_path.path) {
                match dependent {
                    DependentType::LocalStat(local_stat) => {
                        local_dependents.push(StatPath::parse(&local_stat));
                    },
                    DependentType::EntityStat(dependent_entity) => {
                        entity_dependents.push(dependent_entity);
                    }
                }
            }
        }
        
        // Update all local dependents
        for local_dependent in local_dependents {
            self.update_stat_recursive(target_entity, &local_dependent, processed);
        }
        
        // Update all entity dependents
        for dependent_entity in entity_dependents {
            if let Ok(dependent_stats) = self.stats_query.get(dependent_entity) {
                // Find all prefixes that reference this entity
                let prefixes: Vec<String> = dependent_stats.sources
                    .iter()
                    .filter_map(|(prefix, &entity)| {
                        if entity == target_entity {
                            Some(prefix.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                
                // Update cached values and get stats to update
                let mut stats_to_update = Vec::new();
                for prefix in prefixes {
                    let cache_key = format!("{}@{}", prefix, stat_path.path);
                    dependent_stats.set_cached(&cache_key, current_value);
                    
                    for cache_dependent in dependent_stats.get_stat_dependents(&cache_key) {
                        if let DependentType::LocalStat(dependent_stat) = cache_dependent {
                            stats_to_update.push(StatPath::parse(&dependent_stat));
                        }
                    }
                }
                
                // Update the dependent stats recursively
                for stat_to_update in stats_to_update {
                    self.update_stat_recursive(dependent_entity, &stat_to_update, processed);
                }
            }
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
        let Ok(target_stats) = self.stats_query.get(target_entity) else {
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
            let Ok(dependent_stats) = self.stats_query.get(dependent_entity) else {
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
            self.update_stat(dependent_entity, &StatPath::parse(&cache_key));
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