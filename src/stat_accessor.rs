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
    /// * `path`: A string representing the stat path (e.g., "Damage", "Health.base").
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
                                    path.full_path,
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

        let (dependency_updates_for_source_registration, target_stats_to_update, cache_updates_for_target) = 
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

        // Ensure values for directly affected stats are correctly evaluated and in component cache using StatAccessor's full view
        for update_info in &target_stats_to_update { // Iterate by reference first
            let correct_value = self.evaluate(update_info.entity, &update_info.stat_path_on_dependent);
            if let Ok(stats_comp) = self.query.get_mut(update_info.entity) {
                stats_comp.set_cached(&update_info.stat_path_on_dependent, correct_value);
            }
        }

        // Then, trigger recursive updates for these (and their further dependents)
        for stat_update in target_stats_to_update { // Consumes the Vec target_stats_to_update
            // Invalidate the specific query cache for Tagged stats (and potentially other types)
            // before re-evaluating the stat.
            if let Ok(mut target_stats_mut) = self.query.get_mut(stat_update.entity) {
                 let path_to_clear = StatPath::parse(&stat_update.stat_path_on_dependent);
                 target_stats_mut.clear_internal_cache_for_path(&path_to_clear);
            }

            self.update_stat(stat_update.entity, &stat_update.stat_path_on_dependent); 
        }
    }

    fn collect_source_updates(
        &mut self,
        target_entity: Entity,
        source_alias_in_target: &str,
        source_entity: Entity,
    ) -> (Vec<DependencyUpdate>, Vec<StatUpdate>, Vec<CacheUpdate>) {
        let mut updates_to_add = Vec::new();
        let mut stat_updates_needed = Vec::new();
        let mut cache_updates_for_target: Vec<CacheUpdate> = Vec::new();

        // Get read-only access to target_stats. No clone needed.
        if let Ok(target_stats_ro) = self.query.get(target_entity) {
            // Directly look up the requirements for the source_alias being registered.
            if let Some(requirements) = target_stats_ro.source_requirements.get(source_alias_in_target) {
                for requirement in requirements {
                    let source_path = &requirement.path_on_source;
                    let target_path = &requirement.local_dependent;
                    let full_variable_in_expression_str = &requirement.path_in_expression;

                    updates_to_add.push(DependencyUpdate::new_add(
                        source_entity,
                        source_path,
                        target_entity,
                        target_path,
                        source_alias_in_target,
                    ));

                    let value_from_source = if let Ok(source_entity_stats_ro) = self.query.get(source_entity) {
                        source_entity_stats_ro.evaluate_by_string(source_path)
                    } else {
                        // Consider logging a warning if the source entity cannot be read
                        warn!(
                            "In collect_source_updates: Source entity {:?} for alias '{}' on target {:?} not found or missing Stats. Defaulting value to 0.0 for variable '{}'.",
                            source_entity, source_alias_in_target, target_entity, full_variable_in_expression_str
                        );
                        0.0
                    };

                    cache_updates_for_target.push(CacheUpdate {
                        entity: target_entity,
                        key: full_variable_in_expression_str.clone(),
                        value: value_from_source,
                    });

                    stat_updates_needed.push(StatUpdate::new(target_entity, target_path));
                }
            }
        } else {
            // Consider logging a warning if the target entity cannot be read
            warn!(
                "In collect_source_updates: Target entity {:?} not found or missing Stats when processing alias '{}' from source {:?}.",
                target_entity, source_alias_in_target, source_entity
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
    /// * `path`: The string representation of the stat path (e.g., "Damage", "Health.current").
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

        // Evaluate using StatAccessor's own evaluate method
        let new_value = self.evaluate(entity_updated, path_updated);

        // Cache this new_value into the component's Stats
        if let Ok(stats_comp) = self.query.get_mut(entity_updated) {
            stats_comp.set_cached(path_updated, new_value);
        }
        // If query failed, new_value remains what self.evaluate returned (e.g. 0.0 if entity gone)
        // but we couldn't cache it. This is probably fine as the entity might be despawning.

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

        // Explicitly evaluate and cache main dependent stats *after* their source components are cached.
        for update in &stat_updates_needed {
            let fully_evaluated_value = self.evaluate(update.entity, &update.stat_path_on_dependent);
            if let Ok(stats_comp) = self.query.get_mut(update.entity) {
                stats_comp.set_cached(&update.stat_path_on_dependent, fully_evaluated_value);
            }
        }

        for update in &stat_updates_needed {
            // This will further propagate to other dependents of these stats.
            self.update_stat(update.entity, &update.stat_path_on_dependent);
        }
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
        // --- Step 1: Gather all information from target_entity (the one being removed) ---
        let (
            cloned_target_dependents_map,    // Cloned map of what depended ON target_entity (stat_on_target -> (dependent_type -> count))
            cloned_target_sources,           // Cloned map of what target_entity sourced FROM (alias -> source_id)
            cloned_target_source_requirements // Cloned map of what target_entity required from its sources (alias -> Vec<SourceRequirement>)
        ) = if let Ok(target_stats_ro) = self.query.get(target_entity) {
            (
                target_stats_ro.get_dependents().clone(),
                target_stats_ro.sources.clone(),
                target_stats_ro.source_requirements.clone(),
            )
        } else {
            // target_entity doesn't have Stats or query failed, nothing to do.
            return;
        };

        // --- Part 1: Process entities that depended ON target_entity (target_entity was their source) ---

        // Step 1.A: Collect details about how target_entity was used by its dependents.
        // Key: dependent_entity_id
        // Value: Vec of (alias_used_by_dependent, stat_on_target_that_was_sourced, path_on_dependent_to_update)
        let mut dependent_processing_details: HashMap<Entity, Vec<(String, String, String)>> = HashMap::new();
        for (stat_on_target, specific_deps_map) in &cloned_target_dependents_map {
            for (dependent_type, _count) in specific_deps_map.iter() {
                if let DependentType::EntityStat { entity: dep_e, path: path_on_dep, source_alias: alias } = dependent_type {
                    // Ensure we are not processing target_entity itself if it somehow listed itself (should not happen for EntityStat)
                    if *dep_e == target_entity { continue; }

                    dependent_processing_details.entry(*dep_e)
                        .or_default()
                        .push((alias.clone(), stat_on_target.clone(), path_on_dep.clone()));
                }
            }
        }

        // Step 1.B: Apply changes to these dependent entities
        let mut stats_to_update_on_dependents: HashSet<(Entity, String)> = HashSet::new();
        for (dependent_entity_id, details_vec) in dependent_processing_details {
            if let Ok(mut dependent_stats_mut) = self.query.get_mut(dependent_entity_id) {
                // Clean the sources map on the dependent: remove all entries where target_entity was the source.
                dependent_stats_mut.sources.retain(|_alias, &mut source_id| source_id != target_entity);

                for (alias_used_by_dependent, stat_on_target_orig, path_on_dependent_to_update) in details_vec {
                    // Construct the cache key that the dependent would have used for this specific sourced value.
                    let cache_key_on_dependent = format!("{}@{}", stat_on_target_orig, alias_used_by_dependent);
                    // Remove/nullify the cached value.
                    dependent_stats_mut.remove_cached(&cache_key_on_dependent);
                    // Mark the dependent's stat for update.
                    stats_to_update_on_dependents.insert((dependent_entity_id, path_on_dependent_to_update));
                }
            }
        }

        // Step 1.C: Trigger updates for all affected stats on the dependents
        for (entity_to_update, stat_path_to_update) in stats_to_update_on_dependents {
            self.update_stat(entity_to_update, &stat_path_to_update);
        }

        // --- Part 2: Process entities FROM WHICH target_entity sourced stats (target_entity was their dependent) ---
        for (alias_target_used_for_source, actual_source_entity_id) in cloned_target_sources {
            // Skip if the source is target_entity itself (shouldn't be a typical setup)
            if actual_source_entity_id == target_entity { continue; }

            if let Some(requirements_vec) = cloned_target_source_requirements.get(&alias_target_used_for_source) {
                if let Ok(mut actual_source_stats_mut) = self.query.get_mut(actual_source_entity_id) {
                    for req in requirements_vec {
                        // Construct the DependentType that represents target_entity's dependency on this actual_source_entity.
                        let dependent_type_to_remove = DependentType::EntityStat {
                            entity: target_entity, // The entity being removed (who was the dependent)
                            path: req.local_dependent.clone(), // The stat on target_entity that depended on the source
                            source_alias: alias_target_used_for_source.clone(), // The alias target_entity used for this source
                        };
                        actual_source_stats_mut.remove_dependent(&req.path_on_source, dependent_type_to_remove);
                    }
                }
            }
        }
        // The Stats component on target_entity itself will be removed by Bevy when the entity is despawned.
        // No need to manually clear target_entity.sources, target_entity.source_requirements, etc.
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

mod remove_stat_entity_tests {
    use bevy::prelude::*;
    use super::super::prelude::*;

    #[derive(Debug, Clone, Eq, PartialEq, Hash, SystemSet)]
    enum TestPhase {
        Setup,
        RegisterSources,
        PreVerify,
        Remove,
        PostVerify,
    }

    #[derive(Resource, Default, Debug)]
    struct TestEntities {
        a: Option<Entity>,
        b: Option<Entity>,
        c: Option<Entity>,
    }

    const STAT_A_POWER: &str = "Power";
    const STAT_A_BUFFED_POWER: &str = "BuffedPower";
    const STAT_B_STRENGTH: &str = "Strength";
    const STAT_C_BUFF: &str = "Buff";

    const ALIAS_A_AS_SOURCE_FOR_B: &str = "SourceA";
    const ALIAS_C_AS_SOURCE_FOR_A: &str = "SourceC";

    fn setup_test_entities_for_removal(
        mut commands: Commands,
        mut test_entities: ResMut<TestEntities>,
    ) {
        let mut mods_a = ModifierSet::default();
        mods_a.add(&format!("{}.base", STAT_A_POWER), 10.0f32);
        let expr_a_buffed_power = Expression::new(&format!("{}@{} + {}", STAT_C_BUFF, ALIAS_C_AS_SOURCE_FOR_A, STAT_A_POWER))
            .expect("Failed to create A_BuffedPower expression");
        mods_a.add(&format!("{}.expr", STAT_A_BUFFED_POWER), expr_a_buffed_power);
        let entity_a = commands.spawn((
            Stats::new(), 
            StatsInitializer::new(mods_a),
            Name::new("EntityA")
        )).id();

        let mut mods_b = ModifierSet::default();
        let expr_b_strength = Expression::new(&format!("{}@{} * 2.0", STAT_A_POWER, ALIAS_A_AS_SOURCE_FOR_B))
            .expect("Failed to create B_Strength expression");
        mods_b.add(&format!("{}.expr", STAT_B_STRENGTH), expr_b_strength);
        let entity_b = commands.spawn((
            Stats::new(),
            StatsInitializer::new(mods_b),
            Name::new("EntityB")
        )).id();

        let mut mods_c = ModifierSet::default();
        mods_c.add(&format!("{}.base", STAT_C_BUFF), 5.0f32);
        let entity_c = commands.spawn((
            Stats::new(),
            StatsInitializer::new(mods_c),
            Name::new("EntityC")
        )).id();

        test_entities.a = Some(entity_a);
        test_entities.b = Some(entity_b);
        test_entities.c = Some(entity_c);
    }

    fn register_initial_sources(
        mut stat_accessor: StatAccessor,
        test_entities: Res<TestEntities>,
    ) {
        let entity_a = test_entities.a.expect("Entity A missing in register_initial_sources");
        let entity_b = test_entities.b.expect("Entity B missing in register_initial_sources");
        let entity_c = test_entities.c.expect("Entity C missing in register_initial_sources");

        stat_accessor.register_source(entity_b, ALIAS_A_AS_SOURCE_FOR_B, entity_a);
        stat_accessor.register_source(entity_a, ALIAS_C_AS_SOURCE_FOR_A, entity_c);
    }

    fn pre_removal_verification(
        test_entities: Res<TestEntities>,
        query: Query<&Stats>,
    ) {
        let entity_a = test_entities.a.unwrap();
        let entity_b = test_entities.b.unwrap();
        let entity_c = test_entities.c.unwrap();

        let [stats_a, stats_b, stats_c] = query.get_many([entity_a, entity_b, entity_c]).unwrap();

        assert_eq!(stats_b.get(STAT_B_STRENGTH), 20.0, "Initial B.Strength");
        assert_eq!(stats_a.get(STAT_A_BUFFED_POWER), 15.0, "Initial A.BuffedPower");

        assert!(stats_b.sources.contains_key(ALIAS_A_AS_SOURCE_FOR_B), "B.sources should contain SourceA");
        assert_eq!(stats_b.sources.get(ALIAS_A_AS_SOURCE_FOR_B), Some(&entity_a), "B.sources[SourceA] should be entity_A");
        
        let cached_key_b = format!("{}@{}", STAT_A_POWER, ALIAS_A_AS_SOURCE_FOR_B);
        assert_eq!(stats_b.get(&cached_key_b), 10.0, "B's cache for A.Power@SourceA");

        let c_dependents_on_buff = stats_c.get_stat_dependents(STAT_C_BUFF);
        
        let expected_dependent_on_c = DependentType::EntityStat {
            entity: entity_a,
            path: STAT_A_BUFFED_POWER.to_string(),
            source_alias: ALIAS_C_AS_SOURCE_FOR_A.to_string(),
        };
        assert!(
            c_dependents_on_buff.contains(&expected_dependent_on_c),
            "C.dependents_map for C_Buff should contain EntityA's BuffedPower. Found: {:?}", c_dependents_on_buff
        );
    }

    fn do_remove_entity_a(
        test_entities: Res<TestEntities>,
        mut stat_accessor: StatAccessor,
    ) {
        let entity_a = test_entities.a.unwrap();
        stat_accessor.remove_stat_entity(entity_a);
    }

    fn post_removal_verification(
        test_entities: Res<TestEntities>,
        stat_accessor: StatAccessor,
    ) {
        let entity_b = test_entities.b.unwrap();
        let entity_c = test_entities.c.unwrap();

        let stats_b = stat_accessor.get_stats(entity_b).expect("Entity B should still have Stats");

        assert!(!stats_b.sources.values().any(|&id| id == test_entities.a.unwrap()), "B.sources should not contain entity_A anymore");
        if let Some(source_entity_for_alias) = stats_b.sources.get(ALIAS_A_AS_SOURCE_FOR_B) {
            assert_ne!(*source_entity_for_alias, test_entities.a.unwrap(), "B.sources[SourceA] should no longer be entity_A");
        }

        let cached_key_b = format!("{}@{}", STAT_A_POWER, ALIAS_A_AS_SOURCE_FOR_B);
        assert_eq!(stats_b.get(&cached_key_b), 0.0, "B's cache for A.Power@SourceA should be 0.0 after A removed");
        
        assert_eq!(stat_accessor.evaluate(entity_b, STAT_B_STRENGTH), 0.0, "B.Strength after A removed");

        let stats_c = stat_accessor.get_stats(entity_c).expect("Entity C should still have Stats");
        let c_dependents_on_buff = stats_c.get_stat_dependents(STAT_C_BUFF);
        
        let removed_dependent_on_c = DependentType::EntityStat {
            entity: test_entities.a.unwrap(),
            path: STAT_A_BUFFED_POWER.to_string(),
            source_alias: ALIAS_C_AS_SOURCE_FOR_A.to_string(),
        };
        assert!(
            !c_dependents_on_buff.contains(&removed_dependent_on_c),
            "C.dependents_map for C_Buff should no longer list EntityA. Found: {:?}", c_dependents_on_buff
        );
    }

    #[test]
    fn test_remove_stat_entity_full_cleanup() {
        let mut app = App::new();

        let mut config = Config::default();
        config.register_stat_type(STAT_A_POWER, "Modifiable");
        config.register_stat_type(STAT_A_BUFFED_POWER, "Modifiable");
        config.register_stat_type(STAT_B_STRENGTH, "Modifiable");
        config.register_stat_type(STAT_C_BUFF, "Modifiable");
        app.insert_resource(config);

        app.add_plugins(super::super::plugin);
        app.init_resource::<TestEntities>();

        app.add_systems(Update, 
            (
                setup_test_entities_for_removal.in_set(TestPhase::Setup),
                apply_deferred.after(TestPhase::Setup).before(TestPhase::RegisterSources),
                register_initial_sources.in_set(TestPhase::RegisterSources),
                apply_deferred.after(TestPhase::RegisterSources).before(TestPhase::PreVerify),
                pre_removal_verification.in_set(TestPhase::PreVerify),
                apply_deferred.after(TestPhase::PreVerify).before(TestPhase::Remove),
                do_remove_entity_a.in_set(TestPhase::Remove),
                apply_deferred.after(TestPhase::Remove).before(TestPhase::PostVerify),
                post_removal_verification.in_set(TestPhase::PostVerify),
            ).chain()
        );
        
        app.update();
    }
}