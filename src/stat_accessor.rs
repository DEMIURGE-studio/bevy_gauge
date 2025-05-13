use bevy::{ecs::system::SystemParam, prelude::*, utils::{HashSet, HashMap}};
use super::prelude::*;
use crate::stat_types::StatUtilMethods;

// TODO:  
// 1. Some way to set a stat
// 2. Some way to initialize a Stats component

// SystemParam for accessing stats from systems
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    query: Query<'w, 's, &'static mut Stats>,
    config: Res<'w, Config>,
}

/// Represents a cache key update needed in a dependent entity.
#[derive(Debug)]
struct CacheUpdate {
    entity: Entity,
    key: String,
    value: f32,
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

    pub fn add_modifier_value(&mut self, target_entity: Entity, path_str: &str, modifier: ModifierType) {
        let path = StatPath::parse(path_str);

        if let ModifierType::Expression(ref expression_details) = modifier {
            let expression_vars: Vec<String> = expression_details.compiled.iter_variable_identifiers().map(|s| s.to_string()).collect();
            let mut values_from_sources_to_cache: Vec<(String, f32)> = Vec::new();
            let mut new_dependencies_to_register: Vec<DependencyUpdate> = Vec::new(); // For registering source -> target

            // Phase 1: Read source values and identify new dependencies
            if let Ok(target_stats_ro) = self.query.get(target_entity) {
                for expression_var_str in &expression_vars { // Iterate over borrowed strings
                    let parsed_var_path = StatPath::parse(expression_var_str);
                    if let Some(source_alias_ref) = parsed_var_path.target {
                        let source_alias = source_alias_ref.to_string();
                        let path_on_source_str = parsed_var_path.without_target_as_string();

                        if let Some(&source_entity_id) = target_stats_ro.sources.get(&source_alias) {
                            if let Ok(source_entity_stats_ro) = self.query.get(source_entity_id) {
                                let initial_value_from_source = source_entity_stats_ro.evaluate_by_string(&path_on_source_str);
                                values_from_sources_to_cache.push((expression_var_str.clone(), initial_value_from_source));
                                
                                new_dependencies_to_register.push(DependencyUpdate::new_add(
                                    source_entity_id,
                                    &path_on_source_str,
                                    target_entity,
                                    path.full_path, // path_str is the stat on target being modified
                                    &source_alias
                                ));
                            } else {
                                values_from_sources_to_cache.push((expression_var_str.clone(), 0.0));
                            }
                        } else {
                            values_from_sources_to_cache.push((expression_var_str.clone(), 0.0));
                        }
                    }
                }
            } else {
                // This else block is for if target_stats_ro couldn't be fetched.
                // If target_entity doesn't have Stats, we wouldn't reach add_modifier_value from StatsComponent.
                // This implies a logic error if hit, or query.get failed for other reasons.
            }

            // Phase 2: Write to target's Stats component (add modifier, cache source values) and register dependencies
            if let Ok(mut target_entity_stats_mut) = self.query.get_mut(target_entity) {
                // Add the actual modifier to the target stat (e.g. TaggedStat.mods)
                target_entity_stats_mut.add_modifier_value(&path, modifier.clone(), &self.config);

                for (key, val) in values_from_sources_to_cache {
                    target_entity_stats_mut.cache_stat(&key, val);
                }
            } else {
                return; // If we can't get mut stats for target, abort.
            }

            // Apply the newly identified dependencies (source -> target)
            if !new_dependencies_to_register.is_empty() {
                self.apply_dependency_updates(&new_dependencies_to_register);
            }

        } else { // Modifier is Literal, no source dependencies to handle here regarding caching or reverse map.
            if let Ok(mut stats_comp) = self.query.get_mut(target_entity) {
                stats_comp.add_modifier_value(&path, modifier.clone(), &self.config);
            } else {
                return;
            }
        }

        // Update the stat on the target entity and its dependents
        self.update_stat(target_entity, path_str);
    }

    pub fn remove_modifier<V: Into<ModifierType>>(&mut self, target_entity: Entity, path: &str, modifier: V) {
        let vt = modifier.into();
        self.remove_modifier_value(target_entity, path, &vt);
    }

    pub fn remove_modifier_value(&mut self, target_entity: Entity, path_str: &str, modifier: &ModifierType) {
        let path = StatPath::parse(path_str);
        if let Ok(mut stats) = self.query.get_mut(target_entity) {
            stats.remove_modifier_value(&path, modifier);
        }

        if let ModifierType::Expression(ref expression_details) = modifier {
            if let Ok(mut entity_stats) = self.query.get_mut(target_entity) {
                for var_name_in_expr in expression_details.compiled.iter_variable_identifiers() {
                    entity_stats.remove_dependent(var_name_in_expr, DependentType::LocalStat(path.full_path.to_string()));
                }
            }
        }

        self.update_stat(target_entity, &path.full_path);
    }

    pub fn register_source(&mut self, target_entity: Entity, name: &str, source_entity: Entity) {
        if !self.query.contains(target_entity) {
            return;
        }

        if let Ok(mut target_stats) = self.query.get_mut(target_entity) {
            target_stats.sources.insert(name.to_string(), source_entity);
        } else {
            return;
        }

        let (dependency_updates_for_source_registration, stats_to_update_on_target_due_to_new_source) = 
            self.collect_source_updates(target_entity, name, source_entity);
        
        if dependency_updates_for_source_registration.is_empty() {
            
        } else {
            self.apply_dependency_updates(&dependency_updates_for_source_registration);
        }

        for stat_update_instruction in stats_to_update_on_target_due_to_new_source {
            self.update_stat(stat_update_instruction.entity, &stat_update_instruction.stat_path_on_dependent); 
        }
    }

    fn collect_source_updates(
        &mut self,
        target_entity: Entity,
        name: &str,
        source_entity: Entity,
    ) -> (Vec<DependencyUpdate>, Vec<StatUpdate>) {
        let mut updates_to_add = Vec::new();
        let mut stat_updates_needed = Vec::new();

        let target_stats_opt = self.query.get(target_entity).ok().cloned();

        if let Some(target_stats) = target_stats_opt {
            for (base_stat_name_on_target_str, stat_definition_on_target) in target_stats.definitions.iter() {
                for (modifier_full_path_str, expression_obj) in stat_definition_on_target.get_all_expressions(base_stat_name_on_target_str) {
                    for var_name_in_expr_str in expression_obj.compiled.iter_variable_identifiers() {
                        let parsed_var_path_obj = StatPath::parse(var_name_in_expr_str);
                        if let Some(source_alias_in_var_str) = parsed_var_path_obj.target {
                            if source_alias_in_var_str == name {
                                let mut source_path_strings: Vec<String> = Vec::new();
                                source_path_strings.push(parsed_var_path_obj.name.to_string());
                                if let Some(p_part) = parsed_var_path_obj.part { source_path_strings.push(p_part.to_string()); }
                                if let Some(t_tag) = parsed_var_path_obj.tag { source_path_strings.push(t_tag.to_string()); }
                                let path_on_source_entity_str = source_path_strings.join(".");

                                updates_to_add.push(DependencyUpdate::new_add(
                                    source_entity,
                                    &path_on_source_entity_str,
                                    target_entity,
                                    &modifier_full_path_str,
                                    name,
                                ));
                                stat_updates_needed.push(StatUpdate::new(target_entity, &modifier_full_path_str));
                            }
                        }
                    }
                }
            }
        }
        
        (updates_to_add, stat_updates_needed)
    }

    fn apply_dependency_updates(&mut self, updates: &[DependencyUpdate]) {
        for update in updates {
            if let Ok(mut source_stats) = self.query.get_mut(update.source_entity) {
                source_stats.add_dependent(
                    &update.source_path,
                    DependentType::EntityStat {
                        entity: update.target_entity,
                        path: update.target_path.clone(),
                        source_alias: update.source_alias_in_target.clone(),
                    },
                );
            } else {
            
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

    pub(crate) fn update_stat_recursive(
        &mut self, 
        entity_updated: Entity, 
        path_updated: &str, 
        processed_for_this_call_chain: &mut HashSet<StatUpdate>
    ) {
        if processed_for_this_call_chain.contains(&StatUpdate { entity: entity_updated, stat_path_on_dependent: path_updated.to_string() }) {
            return; // Already processed this specific entity/path combination in this update chain
        }
        processed_for_this_call_chain.insert(StatUpdate { entity: entity_updated, stat_path_on_dependent: path_updated.to_string() });

        let new_value = self.query.get(entity_updated).map_or(0.0, |stats_comp| {
            let path = StatPath::parse(path_updated);
            let value = stats_comp.evaluate(&path);
            stats_comp.set_cached(path_updated, value);
            value
        });

        let (cache_updates_needed, stat_updates_needed) = 
            self.collect_dependent_updates(entity_updated, path_updated, new_value);

        if cache_updates_needed.is_empty() && stat_updates_needed.is_empty() {
            return;
        }

        for update in cache_updates_needed {
            if let Ok(dependent_stats) = self.query.get_mut(update.entity) {
                dependent_stats.cache_stat(&update.key, update.value);
            }
        }

        self.process_dependent_updates(stat_updates_needed, processed_for_this_call_chain);
    }

    fn collect_dependent_updates(
        &self,
        source_entity_updated: Entity, 
        source_path_updated_on_source: &str, 
        new_source_value: f32 
    ) -> (Vec<CacheUpdate>, Vec<StatUpdate>) {
        let mut cache_updates = Vec::new();
        let mut stat_updates = Vec::new();

        if let Ok(source_stats_ro) = self.query.get(source_entity_updated) {
            let dependents = source_stats_ro.get_stat_dependents(source_path_updated_on_source);

            for dependent_type in dependents {
                match dependent_type {
                    DependentType::EntityStat { entity: dependent_entity_id, path: dependent_stat_path_on_it, source_alias: source_alias_in_dependent } => {
                        let cache_key = format!("{}@{}", source_path_updated_on_source, source_alias_in_dependent);
                        cache_updates.push(CacheUpdate { entity: dependent_entity_id, key: cache_key, value: new_source_value });
                        stat_updates.push(StatUpdate::new(dependent_entity_id, &dependent_stat_path_on_it)); 
                    }
                    DependentType::LocalStat(path_on_self) => {
                        stat_updates.push(StatUpdate::new(source_entity_updated, &path_on_self));
                    }
                }
            }
        }
        (cache_updates, stat_updates)
    }

    fn process_dependent_updates(&mut self, updates: Vec<StatUpdate>, processed: &mut HashSet<StatUpdate>) {
        for update in updates {
            if processed.contains(&update) {
                continue;
            }
            // Note: We do not insert `update` into `processed` here.
            // `update_stat_recursive` will handle adding its own entity/path to `processed` at its start.

            // Attempt to clear internal cache of the dependent stat first
            if let Ok(mut stats_comp) = self.query.get_mut(update.entity) {
                let stat_path_obj = StatPath::parse(&update.stat_path_on_dependent);
                stats_comp.clear_internal_cache_for_path(&stat_path_obj);
            } else {
                
            }

            // Now, recursively call update_stat for this dependent.
            // update_stat_recursive will handle the evaluation, caching, and further propagation.
            self.update_stat_recursive(update.entity, &update.stat_path_on_dependent, processed);
        }
    }

    pub fn apply_modifier_set(&mut self, target_entity: Entity, modifier_set: &ModifierSet) {
        modifier_set.apply(self, &target_entity);
    }

    pub fn remove_modifier_set(&mut self, target_entity: Entity, modifier_set: &ModifierSet) {
        modifier_set.remove(self, &target_entity);
    }

    pub fn remove_stat_entity(&mut self, target_entity: Entity) {
        let Ok(target_stats_ro) = self.query.get(target_entity) else {
            return;
        };

        let mut old_dependent_entities_tuples = Vec::new();
        let target_dependents_map = target_stats_ro.get_dependents();
        
        for (stat_on_target_str, specific_dependents_map) in target_dependents_map.iter() {
            for (dependent_type, _count) in specific_dependents_map.iter() {
                if let DependentType::EntityStat { entity: dependent_entity_id, .. } = dependent_type { 
                    old_dependent_entities_tuples.push((stat_on_target_str.clone(), *dependent_entity_id));
                }
            }
        }

        let mut stat_cache_keys_to_clear_and_update = Vec::new();

        for (stat_on_removed_entity_str, dependent_entity_id_val) in old_dependent_entities_tuples {
            if let Ok(dependent_stats_ro) = self.query.get(dependent_entity_id_val) {
                for (alias_used_in_dependent_expr_str, &actual_source_entity_id) in dependent_stats_ro.sources.iter() {
                    if actual_source_entity_id == target_entity {
                        let cache_key_to_remove_str = format!("{}@{}", stat_on_removed_entity_str, alias_used_in_dependent_expr_str);
                        
                        for (base_path_on_dependent_str, def_on_dependent) in dependent_stats_ro.definitions.iter() {
                            for (modifier_path_str, expression_obj) in def_on_dependent.get_all_expressions(base_path_on_dependent_str) {
                                for var_name_in_expr_str in expression_obj.compiled.iter_variable_identifiers() {
                                    if var_name_in_expr_str == cache_key_to_remove_str {
                                        stat_cache_keys_to_clear_and_update.push((
                                            dependent_entity_id_val, 
                                            cache_key_to_remove_str.clone(), 
                                            modifier_path_str.clone()
                                        ));
                                        break; 
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        for (dep_entity_to_update, key_to_clear_on_it_str, stat_path_on_dep_to_update_str) in stat_cache_keys_to_clear_and_update {
            if let Ok(dependent_stats) = self.query.get_mut(dep_entity_to_update) {
                dependent_stats.remove_cached(&key_to_clear_on_it_str);
            }
            self.update_stat(dep_entity_to_update, &stat_path_on_dep_to_update_str);
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
    source_path: String,
    /// The entity that depends on the source
    target_entity: Entity,
    /// The path of the stat on the target entity that has this dependency
    target_path: String,
    /// The alias used in the target's expression for this source
    source_alias_in_target: String,
}

impl DependencyUpdate {
    /// Creates a new dependency update for adding a dependency
    fn new_add(
        source_entity: Entity, 
        source_path: &str, 
        target_entity: Entity, 
        target_path: &str, 
        source_alias_in_target: &str
    ) -> Self {
        Self {
            source_entity,
            source_path: source_path.to_string(),
            target_entity,
            target_path: target_path.to_string(),
            source_alias_in_target: source_alias_in_target.to_string(),
        }
    }

    /// Creates a new dependency update for removing a dependency
    fn new_remove(
        source_entity: Entity, 
        source_path: &str, 
        target_entity: Entity, 
        target_path: &str, 
        source_alias_in_target: &str
    ) -> Self {
        Self {
            source_entity,
            source_path: source_path.to_string(),
            target_entity,
            target_path: target_path.to_string(),
            source_alias_in_target: source_alias_in_target.to_string(),
        }
    }
}

/// Represents a stat that needs to be recalculated due to dependency changes.
/// Used to track which stats need to be updated after dependency changes are applied.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StatUpdate {
    pub entity: Entity,
    pub stat_path_on_dependent: String,
}

impl StatUpdate {
    /// Creates a new stat update
    fn new(entity: Entity, path: &str) -> Self {
        Self {
            entity,
            stat_path_on_dependent: path.to_string(),
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