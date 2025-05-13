use std::cell::SyncUnsafeCell;

use bevy::{prelude::*, utils::HashMap};
use evalexpr::{Context, ContextWithMutableVariables, HashMapContext, Value, IterateVariablesContext};
use super::prelude::*;

/// A wrapper around the expression evaluation context, hiding `evalexpr` details.
#[derive(Clone, Debug, Default)]
pub struct StatContext(pub(crate) HashMapContext);

impl StatContext {
    /// Creates a new, empty context.
    pub fn new() -> Self {
        Self(HashMapContext::new())
    }

    /// Sets a variable in the context.
    pub fn set_value(&mut self, key: String, value: f32) -> Result<(), String> {
        self.0.set_value(key, Value::Float(value as f64)).map_err(|e| e.to_string())
    }

    // Potentially add get_value, iter_variables etc. if needed by users directly
}

#[derive(Component, Clone, Debug, Default)]
pub struct Stats {
    pub(crate) definitions: HashMap<String, StatType>,
    pub(crate) cached_stats: SyncContext,
    pub(crate) dependents_map: DependencyMap,
    pub(crate) depends_on_map: DependencyMap,
    pub(crate) sources: HashMap<String, Entity>,
}

// TODO needs to track dependencies BOTH WAYS
impl Stats {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            cached_stats: SyncContext::new(),
            dependents_map: DependencyMap::new(),
            depends_on_map: DependencyMap::new(),
            sources: HashMap::new(),
        }
    }
    
    pub fn get(&self, path: &str) -> f32 {
        if self.cached_stats.get(path).is_err() {
            self.cached_stats.set(path, self.evaluate(&StatPath::parse(path)));
        }
        self.cached_stats.get(path).unwrap_or(0.0)
    }

    pub(crate) fn set(&mut self, path: &str, base: f32) -> &mut Self {
        // should use stat's built-in set
        todo!()
    }

    pub(crate) fn set_cached(&self, key: &str, value: f32) {
        self.cached_stats.set(key, value)
    }

    // TODO should remove the entry, no?
    pub(crate) fn remove_cached(&self, key: &str) {
        self.cached_stats.set(key, 0.0);
    }

    pub(crate) fn get_context(&self) -> &HashMapContext {
        self.cached_stats.context()
    }

    pub(crate) fn add_dependent(&mut self, stat: &str, dependent: DependentType) {
        self.dependents_map.add_dependent(stat, dependent);
    }

    pub(crate) fn remove_dependent(&mut self, stat: &str, dependent: DependentType) {
        self.dependents_map.remove_dependent(stat, dependent);
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
        // Note: self.set_cached updates Stats::cached_stats, not the StatType's internal cache like TaggedStat::query_cache
        // This is for direct lookups via Stats::get later.
        self.set_cached(&path.full_path, value); 
        value
    }

    pub(crate) fn add_modifier_value(&mut self, path: &StatPath, modifier: ModifierType, config: &Config) {
        let base_stat = path.name;

        {
            if let ModifierType::Expression(ref depends_on_expression) = modifier {
                self.register_dependencies(path, &depends_on_expression);
            }
            if let Some(stat) = self.definitions.get_mut(base_stat) {
                stat.add_modifier(path, modifier, config);
            } else {
                let mut new_stat = StatType::new(path, config);
                new_stat.add_modifier(path, modifier, config);
                self.definitions.insert(base_stat.to_string(), new_stat);
            }
        }
    }

    pub(crate) fn remove_modifier_value(&mut self, path: &StatPath, modifier: &ModifierType) {
        let base_stat = path.name.to_string();

        {
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.remove_modifier(path, modifier);
            }
            if let ModifierType::Expression(expression) = modifier {
                self.unregister_dependencies(&base_stat, &expression);
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
    pub fn evaluate_expression(&self, expr_str: &str, base_context_opt: Option<&StatContext>) -> Result<f32, StatError> {
        let mut new_eval_context = base_context_opt.map(|sc| sc.0.clone()).unwrap_or_else(HashMapContext::new);
        
        // Add own cached_stats to the context
        for (key, val) in self.cached_stats.context().iter_variables() {
            new_eval_context.set_value(key.into(), val.clone()).map_err(|e| StatError::Internal { details: format!("Failed to set internal context var: {}", e)})?;
        }
        
        let eval_result = evalexpr::eval_with_context(expr_str, &new_eval_context)
            .map_err(|e| StatError::ExpressionError { expression: expr_str.to_string(), details: e.to_string() })?;
        
        Ok(eval_result.as_number().unwrap_or(0.0) as f32)
    }

    pub(crate) fn clear_internal_cache_for_path(&mut self, path: &StatPath) {
        if let Some(stat_definition) = self.definitions.get_mut(path.name) {
            match stat_definition {
                StatType::Tagged(tagged_stat) => {
                    // Assuming Tagged has a method to clear its specific cache.
                    // We also need to ensure the specific part.tag combination is cleared if possible,
                    // or just clear the whole query_cache for that Tagged stat for simplicity.
                    tagged_stat.clear_query_cache(); // Needs to be implemented on Tagged
                }
                // Other StatType variants might have their own caches in the future.
                _ => {}
            }
        }
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DependentType {
    LocalStat(String),
    EntityStat { entity: Entity, path: String, source_alias: String },
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