use std::cell::SyncUnsafeCell;

use bevy::{prelude::*, utils::HashMap};
use evalexpr::{Context, ContextWithMutableVariables, HashMapContext, Value, IterateVariablesContext};
use super::prelude::*;

/// Holds information about a specific stat required from a source alias by an expression
/// on the entity owning this `Stats` component.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SourceRequirement {
    /// Lets say we have the following modifier: "AttackPower" <= "Strength@Parent + 10".
    /// The path on the source entity that is needed (e.g., "Strength")
    /// This is the part of the variable *before* the '@'.
    pub path_on_source: String,
    /// The full path of the stat on *this* (target) entity that contains the expression
    /// using this source requirement (e.g., "AttackPower").
    pub local_dependent: String,
    /// The complete variable name as used in the expression on the target entity
    /// (e.g., "Strength@Parent").
    pub path_in_expression: String,
}

/// A wrapper around an expression evaluation context, used for evaluating stat expressions.
///
/// This struct provides a simplified interface for setting and potentially getting
/// variables (stat values) that are used during the evaluation of a stat expression string.
/// It primarily serves to abstract the underlying `evalexpr::HashMapContext`.
#[derive(Clone, Debug, Default)]
pub struct StatContext(pub(crate) HashMapContext);

impl StatContext {
    /// Creates a new, empty `StatContext`.
    pub fn new() -> Self {
        Self(HashMapContext::new())
    }

    /// Sets a variable (typically a stat path and its value) in the context.
    ///
    /// # Arguments
    ///
    /// * `key`: The name of the variable (e.g., "Health.base", "Damage@Source").
    /// * `value`: The `f32` value of the variable.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the value was set successfully, or an `Err(String)` containing
    /// an error message if setting the value failed.
    pub fn set_value(&mut self, key: String, value: f32) -> Result<(), String> {
        self.0.set_value(key, Value::Float(value as f64)).map_err(|e| e.to_string())
    }

    // Potentially add get_value, iter_variables etc. if needed by users directly
}

/// The Bevy `Component` that holds all stat-related data for an entity.
///
/// An entity with a `Stats` component can have various stats defined for it (e.g., Health, Damage),
/// each with its own modifiers, expressions, and potential dependencies on other stats or entities.
///
/// Most interactions with an entity's stats are performed through the `StatsMutator` system parameter,
/// which operates on this component.
#[derive(Component, Clone, Debug, Default)]
pub struct Stats {
    pub(crate) definitions: HashMap<String, StatType>,
    pub(crate) cached_stats: SyncContext,
    pub(crate) dependents_map: DependencyMap,
    pub(crate) sources: HashMap<String, Entity>,
    /// Maps a source alias (e.g., "Parent", "Weapon") to a list of specific stats
    /// this entity's expressions require from any entity registered with that alias.
    pub(crate) source_requirements: HashMap<String, Vec<SourceRequirement>>,
}

impl Stats {
    /// Creates a new, empty `Stats` component, ready to have stats defined and modifiers added.
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            cached_stats: SyncContext::new(),
            dependents_map: DependencyMap::new(),
            sources: HashMap::new(),
            source_requirements: HashMap::new(),
        }
    }
    
    /// Retrieves the cached evaluated value of a stat for this entity.
    ///
    /// If the stat path has not been evaluated and cached yet during the current update cycle,
    /// this method will trigger an evaluation before returning the value.
    ///
    /// Note: For most use cases, prefer using `StatsMutator::evaluate()` from a system, as it provides
    /// a more comprehensive and managed way to access stat values, including handling dependencies
    /// and updates across entities.
    ///
    /// # Arguments
    ///
    /// * `path`: A string representing the stat path (e.g., "Damage", "Health.base").
    ///
    /// # Returns
    ///
    /// An `f32` representing the evaluated stat value, or `0.0` if the path is invalid or an error occurs.
    pub fn get(&self, path: &str) -> f32 {
        match self.cached_stats.get(path) {
            Ok(value) => value,
            Err(_) => {
                let value = self.evaluate(&StatPath::parse(path));
                self.set_cached(path, value);
                value
            }
        }
    }

    pub(crate) fn set(&mut self, path: &str, base: f32) -> &mut Self {
        self.definitions
            .entry(path.to_string())
            .or_insert(StatType::Flat(Flat::new(&StatPath::parse(path))))
            .set(&StatPath::parse(path), base);
        self
    }

    pub(crate) fn set_cached(&self, key: &str, value: f32) {
        self.cached_stats.set(key, value)
    }

    pub(crate) fn remove_cached(&self, key: &str) {
        self.cached_stats.remove_entry(key);
    }

    pub(crate) fn get_context(&self) -> &HashMapContext {
        self.cached_stats.context()
    }

    pub(crate) fn add_dependent(&mut self, stat: &str, dependent: DependentType) {
        self.dependents_map.add_dependent(stat, dependent);
    }

    pub(crate) fn remove_dependent(&mut self, source_stat_name: &str, dependent_type: DependentType) {
        self.dependents_map.remove_dependent(source_stat_name, dependent_type);
    }

    pub(crate) fn get_stat_dependents(&self, stat: &str) -> Vec<DependentType> {
        self.dependents_map.get_stat_dependents(stat)
    }

    pub(crate) fn get_dependents(&self) -> &HashMap<String, HashMap<DependentType, u32>> {
        self.dependents_map.get_dependents()
    }

    pub fn evaluate_by_string(&self, path: &str) -> f32 {
        let path = StatPath::parse(path);
        self.evaluate(&path)
    }

    pub(crate) fn evaluate(&self, path: &StatPath) -> f32 {
        let stat_definition_opt = self.definitions.get(path.name);
        
        let Some(stat_definition) = stat_definition_opt else {
            return 0.0;
        };

        let value = stat_definition.evaluate(path, self);
        self.set_cached(&path.full_path, value); 
        value
    }

    pub(crate) fn add_modifier_value(&mut self, path: &StatPath, modifier: ModifierType) {
        let base_stat_name = path.name; // base_stat_name is &String

        if let ModifierType::Expression(ref expression_details) = modifier {
            // register_dependencies expects path: &StatPath
            self.register_dependencies(path, expression_details);

            for var_name_in_expr_str in expression_details.compiled.iter_variable_identifiers() {
                let parsed_var_path = StatPath::parse(var_name_in_expr_str);
                if let Some(source_alias_ref) = &parsed_var_path.target { // source_alias_ref is &String
                    let requirement = SourceRequirement {
                        path_on_source: parsed_var_path.without_target_as_string(),
                        local_dependent: path.full_path.to_string(),
                        path_in_expression: var_name_in_expr_str.to_string(),
                    };
                    self.source_requirements
                        .entry(source_alias_ref.to_string()) // Clone &String to String for entry key
                        .or_default()
                        .push(requirement);
                }
            }
        }

        if let Some(stat) = self.definitions.get_mut(base_stat_name) { // Pass &String directly
            stat.add_modifier(path, modifier);
        } else {
            let mut new_stat = StatType::new(path);
            new_stat.add_modifier(path, modifier);
            self.definitions.insert(base_stat_name.to_string(), new_stat);
        }
    }

    pub(crate) fn remove_modifier_value(&mut self, path: &StatPath, modifier: &ModifierType) {
        let base_stat_name = path.name; // base_stat_name is &String

        if let Some(stat) = self.definitions.get_mut(base_stat_name) { // Pass &String directly
            stat.remove_modifier(path, modifier);
        }

        if let ModifierType::Expression(ref expression_details) = modifier {
            // path.full_path is String, .as_ref() gives &str
            self.unregister_dependencies(path.full_path.as_ref(), expression_details);

            for var_name_in_expr_str in expression_details.compiled.iter_variable_identifiers() {
                let parsed_var_path = StatPath::parse(var_name_in_expr_str);
                if let Some(source_alias_ref) = parsed_var_path.target { // source_alias_ref is &String
                    if let Some(requirements_for_alias) = self.source_requirements.get_mut(source_alias_ref) { // Pass &String directly
                        requirements_for_alias.retain(|req| {
                            !(req.local_dependent == path.full_path &&
                              req.path_in_expression == var_name_in_expr_str)
                        });
                        if requirements_for_alias.is_empty() {
                            self.source_requirements.remove(source_alias_ref); // Pass &String directly
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn register_dependencies(&mut self, path: &StatPath, depends_on_expression: &Expression) {
        for var_name in depends_on_expression.compiled.iter_variable_identifiers() {
            self.add_dependent(var_name, DependentType::LocalStat(path.to_string()));
        }
    }

    pub(crate) fn unregister_dependencies(&mut self, dependent_stat: &str, depends_on_expression: &Expression) {
        for depends_on_stat in depends_on_expression.compiled.iter_variable_identifiers() {
            self.remove_dependent(depends_on_stat, DependentType::LocalStat(dependent_stat.to_string()));
        }
    }

    // Helper method to store an entity-dependent stat value
    pub(crate) fn cache_stat(&self, key: &str, value: f32) {
        self.set_cached(key, value);
    }

    // Made public, now uses StatContext wrapper
    /// Evaluates a given mathematical expression string using a provided context and this entity's cached stat values.
    ///
    /// The expression can reference variables that should be present in either the `base_context_opt` or
    /// within this `Stats` component's own cached values (e.g., other stats of this entity).
    /// Values from `self.cached_stats` are added to the context before evaluation.
    ///
    /// # Arguments
    ///
    /// * `expr_str`: The mathematical expression string to evaluate (e.g., "Strength * 2 + Agility").
    /// * `base_context_opt`: An optional `StatContext` providing initial variables for the evaluation.
    ///                     If `None`, a new empty context is used, augmented by this entity's cached stats.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `f32` result of the expression if successful, or a `StatError` if evaluation fails
    /// (e.g., due to a malformed expression or missing variables).
    pub fn evaluate_expression(&self, expr_str: &str, base_context_opt: Option<&StatContext>) -> Result<f32, StatError> {
        let mut new_eval_context = base_context_opt.map(|sc| sc.0.clone()).unwrap_or_else(HashMapContext::new);
        
        // Add own cached_stats to the context
        for (key, val) in self.cached_stats.context().iter_variables() {
            new_eval_context.set_value(key.into(), val.clone()).map_err(|e| StatError::Internal { details: format!("Failed to set internal context var: {}", e)})?;
        }
        
        // Parse the expression to find all variable identifiers
        if let Ok(compiled_expr) = evalexpr::build_operator_tree::<evalexpr::DefaultNumericTypes>(expr_str) {
            // Ensure all variables in the expression have values, defaulting missing ones to 0.0
            for var_name in compiled_expr.iter_variable_identifiers() {
                if new_eval_context.get_value(var_name).is_none() {
                    new_eval_context.set_value(var_name.to_string(), Value::Float(0.0))
                        .map_err(|e| StatError::Internal { details: format!("Failed to set default value for missing variable '{}': {}", var_name, e)})?;
                }
            }
        }
        
        let eval_result = evalexpr::eval_with_context(expr_str, &new_eval_context)
            .map_err(|e| StatError::ExpressionError { expression: expr_str.to_string(), details: e.to_string() })?;
        
        Ok(eval_result.as_number().unwrap_or(0.0) as f32)
    }

    pub(crate) fn clear_internal_cache_for_path(&mut self, path: &StatPath) {
        // First, collect paths to invalidate for Tagged stats
        let mut paths_to_invalidate = Vec::new();
        
        if let Some(stat_definition) = self.definitions.get_mut(path.name) {
            // For Tagged stats, we need to clear Stats cache entries for affected queries
            if let StatType::Tagged(tagged_stat) = stat_definition {
                if let Some(tag) = path.tag {
                    // Find all tracked queries that would be affected by this tag change
                    tagged_stat.query_tracker.retain(|(part, query_tag_from_key), _| {                        
                        let should_invalidate = if *query_tag_from_key == 0 {
                            true // query tag 0 means "match everything", so any change affects it
                        } else if tag == u32::MAX {
                            false // u32::MAX means no valid tags, shouldn't affect anything
                        } else {
                            // Check if the affected permissive modifier would apply to this tracked query
                            (*query_tag_from_key & tag) == *query_tag_from_key
                        };
                        
                        if should_invalidate {
                            // Build the full path for this query to invalidate in Stats cache
                            let full_path = format!("{}.{}.{}", path.name, part, query_tag_from_key);
                            paths_to_invalidate.push(full_path);
                        }
                        
                        !should_invalidate // retain returns true for items to keep, false for items to remove
                    });
                }
            }
            
            stat_definition.clear_internal_cache(path);
        }
        
        // Now invalidate the affected paths in the Stats component's cache
        for invalidate_path in paths_to_invalidate {
            self.remove_cached(&invalidate_path);
        }
        
        // Also clear the Stats component's own cache for this specific path
        self.remove_cached(&path.full_path);
    }
}

#[derive(Debug, Default)]
pub(crate) struct SyncContext(SyncUnsafeCell<HashMapContext>);

impl SyncContext {
    fn new() -> Self {
        Self(SyncUnsafeCell::new(HashMapContext::new()))
    }

    fn get(&self, path: &str) -> Result<f32, StatError> {
        unsafe {
            if let Some(stat_value) = (*self.0.get()).get_value(path.into()) {
                return Ok(stat_value.as_float().unwrap_or(0.0) as f32);
            }
        }
        Err(StatError::StatNotFound { path: path.to_string() })
    }

    fn set(&self, path: &str, value: f32) {
        unsafe {
            (*self.0.get()).set_value(path.to_string(), Value::Float(value as f64)).unwrap()
        }
    }

    fn remove_entry(&self, key: &str) {
        unsafe {
            let old_context = &*self.0.get();
            let mut new_context = HashMapContext::new();
            
            // Copy all entries except the one we want to remove
            for (existing_key, value) in old_context.iter_variables() {
                if existing_key != key {
                    new_context.set_value(existing_key.into(), value.clone()).unwrap();
                }
            }
            
            // Replace the old context with the new one
            *self.0.get() = new_context;
        }
    }

    pub(crate) fn context(&self) -> &HashMapContext {
        unsafe { &*self.0.get() }
    }
}

impl Clone for SyncContext {
    fn clone(&self) -> Self {
        let cloned_context = self.context().clone();
        Self(SyncUnsafeCell::new(cloned_context))
    }
}

/// Represents the type of dependency a stat can have.
///
/// This is used internally to track how changes in one stat (the source)
/// should propagate to other stats (the dependents).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DependentType {
    /// The dependent is another stat on the same entity.
    /// The `String` is the full path of the local stat that depends on the source.
    LocalStat(String),
    /// The dependent is a stat on a different entity, referencing this entity as a source.
    EntityStat {
        /// The `Entity` that depends on the source stat.
        entity: Entity,
        /// The full path of the stat on the dependent `entity` that uses the source.
        path: String,
        /// The alias used in the dependent `entity`'s expression to refer to this source.
        source_alias: String
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DependencyMap(HashMap<String, HashMap<DependentType, u32>>);

impl DependencyMap {
    fn new() -> Self {
        Self(HashMap::new())
    }
   
    fn add_dependent(&mut self, path: &str, dependent: DependentType) {
        let entry = self.0
            .entry(path.to_string())
            .or_insert_with(HashMap::new);
        
        *entry.entry(dependent.clone()).or_insert(0) += 1;
    }
   
    fn remove_dependent(&mut self, path: &str, dependent: DependentType) {
        if let Some(dependents) = self.0.get_mut(path) {
            if let Some(weight) = dependents.get_mut(&dependent) {
                *weight -= 1;
                if *weight == 0 {
                    dependents.remove(&dependent);
                }
            }
            
            if dependents.is_empty() {
                self.0.remove(path);
            }
        }
    }
   
    fn get_stat_dependents(&self, path: &str) -> Vec<DependentType> {
        let result = self.0
            .get(path)
            .map(|dependents| dependents.keys().cloned().collect())
            .unwrap_or_else(Vec::new);
        result
    }
   
    // No need for the clone note anymore since we're not dealing with locks
    fn get_dependents(&self) -> &HashMap<String, HashMap<DependentType, u32>> {
        &self.0
    }
}