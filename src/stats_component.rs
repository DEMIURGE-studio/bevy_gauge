use std::{cell::SyncUnsafeCell, sync::{Arc, RwLock}};

use bevy::{prelude::*, utils::HashMap};
use evalexpr::{Context, ContextWithMutableVariables, HashMapContext, Value};
use super::prelude::*;

#[derive(Component, Debug, Default)]
pub struct Stats {
    pub(crate) definitions: HashMap<String, StatType>,
    pub(crate) cached_stats: SyncContext,
    pub(crate) dependency_graph: SyncDependents,
    pub(crate) sources: HashMap<String, Entity>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            cached_stats: SyncContext::new(),
            dependency_graph: SyncDependents::new(),
            sources: HashMap::new(),
        }
    }
    
    pub fn get(&self, stat_path: &str) -> Result<f32, StatError> {
        self.cached_stats.get(stat_path)
    }

    pub(crate) fn set_cached(&self, key: &str, value: f32) {
        self.cached_stats.set(key, value)
    }

    pub(crate) fn cached_context(&self) -> &HashMapContext {
        self.cached_stats.context()
    }

    pub(crate) fn add_dependent(&self, stat: &str, dependent: DependentType) {
        self.dependency_graph.add_dependent(stat, dependent);
    }

    pub(crate) fn remove_dependent(&self, stat: &str, dependent: DependentType) {
        self.dependency_graph.remove_dependent(stat, dependent);
    }

    pub(crate) fn get_dependents(&self, stat: &str) -> Vec<DependentType> {
        self.dependency_graph.get_dependents(stat)
    }

    pub fn evaluate_by_string(&self, stat_path: &str) -> f32 {
        let stat_path = StatPath::parse(stat_path);
        self.evaluate(&stat_path)
    }

    pub(crate) fn evaluate(&self, stat_path: &StatPath) -> f32 {
        if stat_path.segments.is_empty() {
            return 0.0;
        }
        
        let head = &stat_path.segments[0];
        let stat_type = self.definitions.get(head);
        let Some(stat_type) = stat_type else { return 0.0; };

        let value = stat_type.evaluate(stat_path, self);
        self.set_cached(&stat_path.path, value);
        value
    }

    pub(crate) fn add_modifier(&mut self, stat_path: &StatPath, modifier: ValueType) {
        if stat_path.segments.is_empty() {
            return;
        }
        
        let base_stat = stat_path.segments[0].to_string();

        {
            if let ValueType::Expression(ref depends_on_expression) = modifier {
                self.register_dependencies(stat_path, &depends_on_expression);
            }
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.add_modifier(stat_path, modifier);
            } else {
                let new_stat = StatType::new(&stat_path.path, modifier);
                new_stat.on_insert(self, stat_path);
                self.definitions.insert(base_stat.clone(), new_stat);
            }
        }
    }

    pub(crate) fn remove_modifier(&mut self, stat_path: &StatPath, modifier: &ValueType) {
        if stat_path.segments.is_empty() {
            return;
        }
        
        let base_stat = stat_path.segments[0].to_string();

        {
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.remove_modifier(stat_path, modifier);
            }
            if let ValueType::Expression(expression) = modifier {
                self.unregister_dependencies(&base_stat, &expression);
            }
        }
    }

    pub(crate) fn register_dependencies(&self, stat_path: &StatPath, depends_on_expression: &Expression) {
        for var_name in depends_on_expression.value.iter_variable_identifiers() {
            self.evaluate(stat_path);
            self.add_dependent(var_name, DependentType::LocalStat(stat_path.path.to_string()));
        }
    }

    pub(crate) fn unregister_dependencies(&self, dependent_stat: &str, depends_on_expression: &Expression) {
        for depends_on_stat in depends_on_expression.value.iter_variable_identifiers() {
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

    fn get(&self, stat_path: &str) -> Result<f32, StatError> {
        unsafe {
            if let Some(stat_value) = (*self.0.get()).get_value(stat_path.into()) {
                return Ok(stat_value.as_float().unwrap_or(0.0) as f32);
            }
        }
        Err(StatError::NotFound("Stat not found in get".to_string()))
    }

    fn set(&self, stat_path: &str, value: f32) {
        unsafe {
            (*self.0.get()).set_value(stat_path.to_string(), Value::Float(value as f64)).unwrap()
        }
    }

    fn context(&self) -> &HashMapContext {
        unsafe { &*self.0.get() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DependentType {
    LocalStat(String),
    EntityStat(Entity), // Entity that depends on this stat
}

#[derive(Debug, Default)]
pub(crate) struct SyncDependents(Arc<RwLock<HashMap<String, HashMap<DependentType, u32>>>>);

impl SyncDependents {
    fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
   
    fn add_dependent(&self, stat_path: &str, dependent: DependentType) {
        if let Ok(mut graph) = self.0.write() {
            let entry = graph.entry(stat_path.to_string()).or_insert_with(HashMap::new);
            *entry.entry(dependent).or_insert(0) += 1;
        }
    }
    
    fn remove_dependent(&self, stat_path: &str, dependent: DependentType) {
        if let Ok(mut graph) = self.0.write() {
            if let Some(dependents) = graph.get_mut(stat_path) {
                if let Some(weight) = dependents.get_mut(&dependent) {
                    *weight -= 1;
                    if *weight == 0 {
                        dependents.remove(&dependent);
                    }
                }
                if dependents.is_empty() {
                    graph.remove(stat_path);
                }
            }
        }
    }
    
    fn get_dependents(&self, stat_path: &str) -> Vec<DependentType> {
        if let Ok(graph) = self.0.read() {
            graph.get(stat_path)
                .map(|dependents| dependents.keys().cloned().collect())
                .unwrap_or_else(Vec::new)
        } else {
            Vec::new()
        }
    }
}