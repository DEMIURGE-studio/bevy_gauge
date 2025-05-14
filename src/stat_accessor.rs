use bevy::{ecs::system::SystemParam, prelude::*, utils::{HashSet, HashMap}};
use super::prelude::*;

// TODO:  
// 1. Some way to set a stat
// 2. Some way to initialize a Stats component

// SystemParam for accessing stats from systems
#[derive(SystemParam)]
/// A Bevy `SystemParam` that provides access to query and modify stats for entities.
///
/// `StatAccessor` is the primary way to interact with the stat system from within Bevy systems.
/// It allows for operations like adding/removing modifiers, evaluating stat values,
/// registering dependencies between entities (sources), and managing the lifecycle of stats.
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
    /// Retrieves the evaluated value of a stat for a given entity.
    ///
    /// If the entity does not have a `Stats` component or the path is invalid,
    /// this will typically return `0.0`.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` whose stat value is to be retrieved.
    /// * `path`: A string representing the stat path (e.g., "Damage.total", "Health.base").
    ///
    /// # Returns
    ///
    /// An `f32` representing the evaluated stat value, or `0.0` if not found or an error occurs.
    pub fn get(&self, target_entity: Entity, path: &str) -> f32 {
        let Ok(stats) = self.query.get(target_entity) else {
            return 0.0;
        };

        stats.get(path)
    }

    /// Sets the base value of a specific stat part for a given entity.
    ///
    /// This method is intended for directly setting a stat value, bypassing the
    /// typical modifier system for that specific part. It's often used for initializing
    /// base stats or for stats that are not meant to be modified via the additive/multiplicative
    /// layers.
    ///
    /// If the entity does not have a `Stats` component, this operation will have no effect.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` whose stat is to be set.
    /// * `path`: A string representing the stat path to set (e.g., "Health.base").
    /// * `value`: The `f32` value to set for the stat.
    pub fn set(&mut self, target_entity: Entity, path: &str, value: f32) {
        let Ok(mut stats) = self.query.get_mut(target_entity) else {
            return;
        };

        stats.set(path, value);
    }
    
    /// Retrieves a read-only reference to the `Stats` component of an entity.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` whose `Stats` component is to be retrieved.
    ///
    /// # Returns
    ///
    /// A `Result` containing a reference to the `Stats` component if successful,
    /// or an empty error `()` if the entity does not have a `Stats` component.
    pub fn get_stats(&self, target_entity: Entity) -> Result<&Stats, ()> {
        let Ok(stats) = self.query.get(target_entity) else {
            return Err(());
        };

        Ok(stats)
    }

    /// Adds a modifier to a stat on the target entity.
    ///
    /// The `modifier` can be a literal value (e.g., `f32`) or an `Expression`.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` to which the modifier will be added.
    /// * `path`: The stat path (e.g., "Damage.increased.Fire") to modify.
    /// * `modifier`: The modifier to add, convertible into `ModifierType` (e.g., `50.0f32` or `Expression::new(...)`).
    pub fn add_modifier<V: Into<ModifierType>>(&mut self, target_entity: Entity, path: &str, modifier: V) {
        let vt = modifier.into();
        self.add_modifier_value(target_entity, path, vt);
    }

    /// Adds a `ModifierType` (literal or expression) to a stat on the target entity.
    ///
    /// This is a more direct version of `add_modifier` that takes an explicit `ModifierType`.
    /// It handles registering dependencies if the modifier is an expression involving sources,
    /// caches necessary values, and triggers updates to the modified stat and its dependents.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` to which the modifier will be added.
    /// * `path_str`: The string representation of the stat path to modify.
    /// * `modifier`: The `ModifierType` (literal value or expression) to add.
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

    /// Removes a modifier from a stat on the target entity.
    ///
    /// The `modifier` to be removed must match one that was previously added.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` from which the modifier will be removed.
    /// * `path`: The stat path (e.g., "Damage.increased.Fire") from which to remove the modifier.
    /// * `modifier`: The modifier to remove, convertible into `ModifierType`.
    pub fn remove_modifier<V: Into<ModifierType>>(&mut self, target_entity: Entity, path: &str, modifier: V) {
        let vt = modifier.into();
        self.remove_modifier_value(target_entity, path, &vt);
    }

    /// Removes a specific `ModifierType` from a stat on the target entity.
    ///
    /// This is a more direct version of `remove_modifier`. It ensures that if the
    /// removed modifier was an expression, its dependencies are cleaned up.
    /// It then triggers updates to the modified stat and its dependents.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` from which the modifier will be removed.
    /// * `path_str`: The string representation of the stat path.
    /// * `modifier`: The `ModifierType` to remove.
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

    /// Registers a source entity for a target entity, allowing the target's stats
    /// to reference the source's stats in expressions.
    ///
    /// For example, after registering `source_B` with alias `"B_Stats"` for `target_A`,
    /// `target_A` can have a stat modifier like `"Strength@B_Stats * 2.0"`.
    ///
    /// This function handles:
    /// - Storing the source alias mapping on the target entity.
    /// - Identifying existing expressions on the target that now match the new source.
    /// - Establishing dependency links from the source to the target for these expressions.
    /// - Caching initial values from the source for these expressions on the target.
    /// - Triggering updates for stats on the target that are affected by this new source.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` that will gain access to the source.
    /// * `name`: The alias (e.g., "PlayerShield", "AllyBuffs") to use in expressions on the `target_entity`
    ///           to refer to the `source_entity`.
    /// * `source_entity`: The `Entity` whose stats will be made available to the `target_entity`.
    pub fn register_source(&mut self, target_entity: Entity, name: &str, source_entity: Entity) {
        if !self.query.contains(target_entity) {
            return;
        }

        if let Ok(mut target_stats) = self.query.get_mut(target_entity) {
            target_stats.sources.insert(name.to_string(), source_entity);
        } else {
            return;
        }

        let (dependency_updates_for_source_registration, stats_to_update_on_target_due_to_new_source, cache_updates_for_target) = 
            self.collect_source_updates(target_entity, name, source_entity);
        
        if !dependency_updates_for_source_registration.is_empty() {
            self.apply_dependency_updates(&dependency_updates_for_source_registration);
        }

        // Apply cache updates to the target entity FIRST
        if !cache_updates_for_target.is_empty() {
            if let Ok(target_stats_mut) = self.query.get(target_entity) {
                for cache_op in cache_updates_for_target {
                    // Ensure we are updating the correct entity's cache, though in this flow it should always be target_entity
                    if cache_op.entity == target_entity {
                        target_stats_mut.cache_stat(&cache_op.key, cache_op.value);
                    }
                }
            }
        }

        for stat_update_instruction in stats_to_update_on_target_due_to_new_source {
            // Invalidate the specific query cache for Tagged stats (and potentially other types)
            // before re-evaluating the stat.
            if let Ok(mut target_stats_mut) = self.query.get_mut(stat_update_instruction.entity) {
                 let path_to_clear = StatPath::parse(&stat_update_instruction.stat_path_on_dependent);
                 target_stats_mut.clear_internal_cache_for_path(&path_to_clear);
            }

            self.update_stat(stat_update_instruction.entity, &stat_update_instruction.stat_path_on_dependent); 
        }
    }

    fn collect_source_updates(
        &mut self,
        target_entity: Entity,
        source_alias_being_registered: &str,
        source_entity: Entity,
    ) -> (Vec<DependencyUpdate>, Vec<StatUpdate>, Vec<CacheUpdate>) {
        let mut updates_to_add = Vec::new();
        let mut stat_updates_needed = Vec::new();
        let mut cache_updates_for_target: Vec<CacheUpdate> = Vec::new();

        // Get read-only access to target_stats. No clone needed.
        if let Ok(target_stats_ro) = self.query.get(target_entity) {
            // Directly look up the requirements for the source_alias being registered.
            if let Some(requirements) = target_stats_ro.source_requirements.get(source_alias_being_registered) {
                for requirement in requirements {
                    let path_on_source_entity_str = &requirement.path_on_source;
                    let path_on_target_with_expression_str = &requirement.path_on_target_with_expression;
                    let full_variable_in_expression_str = &requirement.full_variable_in_expression;

                    updates_to_add.push(DependencyUpdate::new_add(
                        source_entity,
                        path_on_source_entity_str,
                        target_entity,
                        path_on_target_with_expression_str,
                        source_alias_being_registered,
                    ));

                    let value_from_source = if let Ok(source_entity_stats_ro) = self.query.get(source_entity) {
                        source_entity_stats_ro.evaluate_by_string(path_on_source_entity_str)
                    } else {
                        // Consider logging a warning if the source entity cannot be read
                        warn!(
                            "In collect_source_updates: Source entity {:?} for alias '{}' on target {:?} not found or missing Stats. Defaulting value to 0.0 for variable '{}'.",
                            source_entity, source_alias_being_registered, target_entity, full_variable_in_expression_str
                        );
                        0.0
                    };

                    cache_updates_for_target.push(CacheUpdate {
                        entity: target_entity,
                        key: full_variable_in_expression_str.clone(),
                        value: value_from_source,
                    });

                    stat_updates_needed.push(StatUpdate::new(target_entity, path_on_target_with_expression_str));
                }
            }
        } else {
            // Consider logging a warning if the target entity cannot be read
            warn!(
                "In collect_source_updates: Target entity {:?} not found or missing Stats when processing alias '{}' from source {:?}.",
                target_entity, source_alias_being_registered, source_entity
            );
        }

        (updates_to_add, stat_updates_needed, cache_updates_for_target)
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

    /// Evaluates the final value of a stat for a given entity, considering all modifiers,
    /// expressions, sources, and configurations.
    ///
    /// This method leverages caching for performance. If a stat's value has been
    /// computed recently and none of its inputs have changed, a cached value may be returned.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` whose stat is to be evaluated.
    /// * `path`: The string representation of the stat path (e.g., "Damage.total", "Health.current").
    ///
    /// # Returns
    ///
    /// An `f32` representing the final computed value of the stat. Returns `0.0` if the
    /// entity or stat cannot be found, or if an error occurs during evaluation.
    pub fn evaluate(&self, target_entity: Entity, path: &str) -> f32 {
        if let Ok(stats) = self.query.get(target_entity) {
            stats.evaluate_by_string(path)
        } else {
            0.0
        }
    }

    /// Triggers an update and re-evaluation of a specific stat on an entity and all its dependents.
    ///
    /// This is typically called internally after a modifier is added/removed or a source changes.
    /// It ensures that the stat's value is recalculated and any downstream stats that
    /// depend on it are also updated.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` whose stat needs updating.
    /// * `stat_path`: The string representation of the stat path that has changed.
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

        // Clear internal cache for the stat we are about to re-evaluate, if applicable (e.g., for Tagged stats).
        // This ensures that when stats_comp.evaluate() is called below, it doesn't use stale internal caches.
        if let Ok(mut stats_comp_for_clear) = self.query.get_mut(entity_updated) {
            let path_obj_for_clear = StatPath::parse(path_updated);
            stats_comp_for_clear.clear_internal_cache_for_path(&path_obj_for_clear);
        }

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

    /// Applies all modifiers from a `ModifierSet` to the target entity.
    ///
    /// This is a convenience method to add multiple modifiers at once, typically
    /// defined in an external source like an item, buff, or skill.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` to which the modifiers will be applied.
    /// * `modifier_set`: A reference to the `ModifierSet` containing the modifiers to apply.
    pub fn apply_modifier_set(&mut self, target_entity: Entity, modifier_set: &ModifierSet) {
        modifier_set.apply(self, &target_entity);
    }

    /// Removes all modifiers from a `ModifierSet` from the target entity.
    ///
    /// This is a convenience method to remove multiple modifiers at once, assuming they
    /// were previously added via a similar `ModifierSet`.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` from which the modifiers will be removed.
    /// * `modifier_set`: A reference to the `ModifierSet` containing the modifiers to remove.
    pub fn remove_modifier_set(&mut self, target_entity: Entity, modifier_set: &ModifierSet) {
        modifier_set.remove(self, &target_entity);
    }

    /// Removes an entity from the stat system's tracking, cleaning up its dependencies.
    ///
    /// This function is intended to be called when an entity with a `Stats` component
    /// is being despawned or is otherwise ceasing to participate in the stat system.
    /// It iterates through the entity's stats and:
    /// 1. For each stat that acts as a source for other entities (dependents):
    ///    a. It finds all dependent stats on those other entities.
    ///    b. It effectively "nullifies" the contribution from the removed source entity
    ///       by setting the cached value for the source part of the expression to 0.0
    ///       on the dependent entity.
    ///    c. It then triggers an update for that dependent stat on the other entity.
    /// 2. It clears all source registrations *from* this entity (i.e., other entities this entity was sourcing from).
    ///
    /// This ensures that dependents are updated correctly and dangling references are handled.
    ///
    /// # Arguments
    ///
    /// * `target_entity`: The `Entity` to remove from stat tracking.
    pub fn remove_stat_entity(&mut self, target_entity: Entity) {
        let Ok(target_stats_ro) = self.query.get(target_entity) else {
            return;
        };

        // Collect information about which external entities depended on the target_entity.
        // The key is stat_on_target_str (e.g., "Health.base" on target_entity).
        // The value is a list of (dependent_entity_id, source_alias_used_by_dependent, path_on_dependent_that_used_this_source_stat)
        // This part needs to be accurate from the target_entity's dependents_map
        let mut external_dependents_info: HashMap<String, Vec<(Entity, String, String)>> = HashMap::new();
        let target_s_dependents_map = target_stats_ro.get_dependents();

        for (stat_on_this_target_str, specific_dependents_map) in target_s_dependents_map.iter() {
            for (dependent_type, _count) in specific_dependents_map.iter() {
                if let DependentType::EntityStat { entity: dependent_entity_id, path: path_on_dependent, source_alias } = dependent_type {
                    external_dependents_info
                        .entry(stat_on_this_target_str.clone())
                        .or_default()
                        .push((*dependent_entity_id, source_alias.clone(), path_on_dependent.clone()));
                }
            }
        }

        let mut stat_cache_keys_to_clear_and_update = Vec::new();

        // For each stat on the removed entity that was used as a source by others...
        for (stat_on_removed_entity_str, dependents_list) in external_dependents_info {
            for (dependent_entity_id_val, alias_used_by_dependent, _path_on_dependent_originally_tracked) in dependents_list {
                if let Ok(dependent_stats_ro) = self.query.get(dependent_entity_id_val) {
                    // Construct the variable name as it would appear in the dependent's expressions
                    let variable_from_removed_source = format!("{}@{}", stat_on_removed_entity_str, alias_used_by_dependent);

                    // Use the dependent's own dependents_map to find which of its local stats used this variable.
                    // dependent_stats_ro.get_stat_dependents(variable_from_removed_source) returns Vec<DependentType>
                    // where DependentType::LocalStat(local_stat_path) indicates a local stat using the variable.
                    let local_stats_on_dependent_using_the_variable = dependent_stats_ro.get_stat_dependents(&variable_from_removed_source);
                    
                    for dependent_type_entry in local_stats_on_dependent_using_the_variable {
                        if let DependentType::LocalStat(local_stat_path_str) = dependent_type_entry {
                            stat_cache_keys_to_clear_and_update.push((
                                dependent_entity_id_val, 
                                variable_from_removed_source.clone(), 
                                local_stat_path_str // This is the stat on the dependent entity that needs updating
                            ));
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

/// A Bevy observer system that automatically calls `StatAccessor::remove_stat_entity`
/// when an entity with a `Stats` component is removed/despawned.
///
/// This ensures that an entity's dependencies are properly cleaned up within the stat system
/// when it ceases to exist.
pub(crate) fn remove_stats(
    trigger: Trigger<OnRemove, Stats>,
    mut stat_accessor: StatAccessor,
) {
    let removed_entity = trigger.entity();
    stat_accessor.remove_stat_entity(removed_entity);
}