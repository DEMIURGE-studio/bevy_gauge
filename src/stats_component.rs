use std::cell::SyncUnsafeCell;

use bevy::{prelude::*, utils::HashMap};
use evalexpr::{Context, ContextWithMutableVariables, HashMapContext, Value};
use super::prelude::*;

#[derive(Component, Clone, Debug, Default)]
pub struct Stats {
    pub(crate) definitions: HashMap<String, StatType>,
    pub(crate) cached_stats: SyncContext,
    pub(crate) dependents_map: DependencyMap,
    pub(crate) depends_on_map: DependencyMap,
    pub(crate) sources: HashMap<String, Entity>,
}

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

    pub fn from_set(modifier_set: &ModifierSet, stat_config: &StatConfig) -> Self {
        let mut stats = Stats::new();
        for (stat, modifiers) in modifier_set.iter() {
            for modifier in modifiers.iter() {
                stats.add_modifier_value(&StatPath::parse(stat), modifier.clone(), stat_config);
            }
        }
        return stats;
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
        if path.segments.is_empty() {
            return 0.0;
        }
        
        let head = &path.segments[0];
        let stat_type = self.definitions.get(head);
        let Some(stat_type) = stat_type else { return 0.0; };

        stat_type.evaluate(path, self)
    }

    pub fn add_modifier<V: Into<ValueType>>(&mut self, path: &str, modifier: V, stat_config: &StatConfig) {
        let vt = modifier.into();
        self.add_modifier_value(&StatPath::parse(path), vt, stat_config);
    }

    pub(crate) fn add_modifier_value(&mut self, path: &StatPath, modifier: ValueType, stat_config: &StatConfig) {
        if path.segments.is_empty() {
            return;
        }
        
        let base_stat = path.segments[0].to_string();

        {
            if let ValueType::Expression(ref depends_on_expression) = modifier {
                self.register_dependencies(path, &depends_on_expression);
            }
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.add_modifier(path, modifier, stat_config);
            } else {
                let mut new_stat = StatType::new(path, stat_config);
                new_stat.add_modifier(path, modifier, stat_config);
                self.definitions.insert(base_stat.clone(), new_stat);
            }
        }
    }

    pub(crate) fn remove_modifier_value(&mut self, path: &StatPath, modifier: &ValueType, stat_config: &StatConfig) {
        if path.segments.is_empty() {
            return;
        }
        
        let base_stat = path.segments[0].to_string();

        {
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.remove_modifier(path, modifier, stat_config);
            }
            if let ValueType::Expression(expression) = modifier {
                self.unregister_dependencies(&base_stat, &expression);
            }
        }
    }

    pub(crate) fn register_dependencies(&mut self, path: &StatPath, depends_on_expression: &Expression) {
        for var_name in depends_on_expression.compiled.iter_variable_identifiers() {
            self.evaluate(path);
            self.add_dependent(var_name, DependentType::LocalStat(path.path.to_string()));
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

    fn context(&self) -> &HashMapContext {
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
    EntityStat(Entity),
}

#[derive(Clone, Debug, Default)]
struct DependencyMap(HashMap<String, HashMap<DependentType, u32>>);

impl DependencyMap {
    fn new() -> Self {
        Self(HashMap::new())
    }
   
    fn add_dependent(&mut self, path: &str, dependent: DependentType) {
        let entry = self.0
            .entry(path.to_string())
            .or_insert_with(HashMap::new);
        
        *entry.entry(dependent).or_insert(0) += 1;
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
        self.0
            .get(path)
            .map(|dependents| dependents.keys().cloned().collect())
            .unwrap_or_else(Vec::new)
    }
   
    // No need for the clone note anymore since we're not dealing with locks
    fn get_dependents(&self) -> &HashMap<String, HashMap<DependentType, u32>> {
        &self.0
    }
}