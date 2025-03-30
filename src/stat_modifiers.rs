use std::{cell::SyncUnsafeCell, sync::{Arc, RwLock}};
use bevy::{ecs::component::Component, utils::{HashMap, HashSet}};
use evalexpr::{Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};
use crate::error::StatError;

#[derive(Debug, Clone, Default)]
pub enum ModType {
    #[default]
    Add,
    Mul,
}

// TODO Fix overuse of .unwrap(). It's fine for now (maybe preferable during development) but in the future we'll want proper errors, panics, and warnings.

// TODO ContextDrivenStats type that wraps stats, but contains a context (Hashmap of strings to entities). Can only call evaluate on it if you pass in a StatContextRefs

/// A collection of stats keyed by their names.
#[derive(Component, Debug, Default)]
pub struct Stats {
    // Holds the definitions of stats. This includes default values, their modifiers, and their dependents
    pub definitions: HashMap<String, StatType>,
    cached_stats: SyncContext,
    dependency_graph: SyncDependents,
}

#[derive(Debug, Default)]
struct SyncContext(SyncUnsafeCell<HashMapContext>);

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

#[derive(Debug, Default)]
struct SyncDependents(Arc<RwLock<HashMap<String, HashMap<String, u32>>>>);

impl SyncDependents {
    fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
   
    fn add_dependent(&self, stat_path: &str, dependent: &str) {
        let mut graph = self.0.write().unwrap();
        let entry = graph.entry(stat_path.to_string()).or_insert(HashMap::new());
        *entry.entry(dependent.to_string()).or_insert(0) += 1;
    }
    
    fn remove_dependent(&self, stat_path: &str, dependent: &str) {
        let Ok(mut graph) = self.0.write() else {
            return;
        };
        let Some(dependents) = graph.get_mut(stat_path) else {
            return;
        };
        if let Some(weight) = dependents.get_mut(dependent) {
            *weight -= 1;
            if *weight == 0 {
                dependents.remove(dependent);
            }
        }
        if dependents.is_empty() {
            graph.remove(stat_path);
        }
    }
    
    fn get_dependents(&self, stat_path: &str) -> Vec<String> {
        let graph = self.0.read().unwrap();
        match graph.get(stat_path) {
            Some(dependents) => dependents.iter().map(|(dep, _)| dep.clone()).collect(),
            None => Vec::new(),
        }
    }
}

impl Stats {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            cached_stats: SyncContext::new(),
            dependency_graph: SyncDependents::new(),
        }
    }

    pub fn get(&self, stat_path: &str) -> Result<f32, StatError> {
        self.cached_stats.get(stat_path)
    }

    /// Public accessor for the cached value retrieval.
    pub fn get_cached(&self, key: &str) -> Result<f32, StatError> {
        self.cached_stats.get(key)
    }

    /// Public accessor to update the cache.
    pub fn set_cached(&self, key: &str, value: f32) {
        self.cached_stats.set(key, value)
    }

    /// Public accessor to retrieve the evaluation context.
    pub fn cached_context(&self) -> &HashMapContext {
        self.cached_stats.context()
    }

    /// Public accessors for dependency management.
    pub fn add_dependent(&self, stat: &str, dependent: &str) {
        self.dependency_graph.add_dependent(stat, dependent);
    }

    pub fn remove_dependent(&self, stat: &str, dependent: &str) {
        self.dependency_graph.remove_dependent(stat, dependent);
    }

    pub fn get_dependents(&self, stat: &str) -> Vec<String> {
        self.dependency_graph.get_dependents(stat)
    }

    /// Evaluates a stat by gathering all its parts and combining their values.
    pub fn evaluate(&self, stat_path: &str) -> f32 {
        let segments: Vec<&str> = stat_path.split("_").collect();
        let head = segments[0];
        let stat_type = self.definitions.get(head);
        let Some(stat_type) = stat_type else { return 0.0; };

        if self.get_cached(stat_path).is_ok() {
            stat_type.evaluate(&segments, self)
        } else {
            let value = stat_type.evaluate(&segments, self);
            self.set_cached(stat_path, value);
            value
        }
    }

    /// Updates a stat's cached value and propagates to dependents
    pub fn update_stat(&self, stat_path: &str) {
        let segments: Vec<&str> = stat_path.split("_").collect();
        let base_stat = segments[0];
        let value = self.evaluate(stat_path);
        self.set_cached(stat_path, value);
        self.update_dependents(base_stat);
    }
    
    /// Updates all stats that depend on the given base stat
    fn update_dependents(&self, base_stat: &str) {
        let dependents = self.get_dependents(base_stat);
        let mut processed = HashSet::new();
        processed.insert(base_stat.to_string());
        for dependent in dependents {
            if !processed.contains(&dependent) {
                processed.insert(dependent.clone());
                let value = self.evaluate(&dependent);
                self.set_cached(&dependent, value);
                self.update_dependents(&dependent);
            }
        }
    }

    pub fn add_modifier<V, S>(&mut self, stat_path: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType> + Clone,
    {
        let stat_path_str = stat_path.into();
        let stat_path_segments: Vec<&str> = stat_path_str.split("_").collect();
        let base_stat = stat_path_segments[0].to_string();

        {
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.add_modifier(&stat_path_segments, value.clone());
            } else {
                let new_stat = StatType::new(&stat_path_str, value.clone());
                new_stat.on_insert(self, &stat_path_segments);
                self.definitions.insert(base_stat.clone(), new_stat);
            }
            let vt: ValueType = value.into();
            if let ValueType::Expression(depends_on_expression) = vt {
                self.register_dependencies(&stat_path_str, &depends_on_expression);
            }
        }
        self.update_stat(&stat_path_str);
    }

    pub fn remove_modifier<V, S>(&mut self, stat_path: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType> + Clone,
    {
        let stat_path_str = stat_path.into();
        let stat_path_segments: Vec<&str> = stat_path_str.split("_").collect();
        let base_stat = stat_path_segments[0].to_string();

        {
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.remove_modifier(&stat_path_segments, value.clone());
            }
            let vt: ValueType = value.into();
            if let ValueType::Expression(expression) = vt {
                self.unregister_dependencies(&base_stat, &expression);
            }
        }
        self.update_stat(&stat_path_str);
    }

    fn register_dependencies(&self, dependent_stat: &str, depends_on_expression: &Expression) {
        for var_name in depends_on_expression.value.iter_variable_identifiers() {
            self.evaluate(var_name);
            self.add_dependent(var_name, dependent_stat);
        }
    }

    fn unregister_dependencies(&self, dependent_stat: &str, depends_on_expression: &Expression) {
        for depends_on_stat in depends_on_expression.value.iter_variable_identifiers() {
            self.remove_dependent(depends_on_stat, dependent_stat);
        }
    }
}

pub trait StatLike {
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V);
    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V);
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32;
    fn on_insert(&self, stats: &Stats, stat_path: &[&str]);
}

#[derive(Debug)]
pub enum StatType {
    Simple(Simple),
    Modifiable(Modifiable),
    Complex(ComplexModifiable),
}

impl StatType {
    pub fn new<V>(stat_path: &str, value: V) -> Self
    where
        V: Into<ValueType>,
    {
        let stat_path_segments: Vec<&str> = stat_path.split("_").collect();
        match stat_path_segments.len() {
            1 => {
                let mut stat = Simple::new(stat_path_segments[0]);
                stat.add_modifier(&stat_path_segments, value);
                StatType::Simple(stat)
            },
            2 => {
                let mut stat = Modifiable::new(stat_path_segments[0]);
                stat.add_modifier(&stat_path_segments, value);
                StatType::Modifiable(stat)
            },
            3 => {
                let mut stat = ComplexModifiable::new(stat_path_segments[0]);
                stat.add_modifier(&stat_path_segments, value);
                StatType::Complex(stat)
            },
            _ => panic!("Invalid stat path format: {}", stat_path)
        }
    }
}

impl StatLike for StatType {
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        match self {
            StatType::Simple(simple) => simple.add_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.add_modifier(stat_path, value),
        }
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        match self {
            StatType::Simple(simple) => simple.remove_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.remove_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.remove_modifier(stat_path, value),
        }
    }
    
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32 {
        match self {
            StatType::Simple(simple) => simple.evaluate(stat_path, stats),
            StatType::Modifiable(modifiable) => modifiable.evaluate(stat_path, stats),
            StatType::Complex(complex_modifiable) => complex_modifiable.evaluate(stat_path, stats),
        }
    }
    
    fn on_insert(&self, stats: &Stats, stat_path: &[&str]) {
        match self {
            StatType::Simple(simple) => simple.on_insert(stats, stat_path),
            StatType::Modifiable(modifiable) => modifiable.on_insert(stats, stat_path),
            StatType::Complex(complex_modifiable) => complex_modifiable.on_insert(stats, stat_path),
        }
    }
}

#[derive(Debug)]
pub struct Simple {
    pub relationship: ModType,
    pub base: f32,
    pub mods: Vec<Expression>,
}

impl Simple {
    pub fn new(name: &str) -> Self {
        let base = get_initial_value_for_modifier(name);
        Self { relationship: ModType::Add, base: base, mods: Vec::new() }
    }
}

impl StatLike for Simple {
    fn add_modifier<V: Into<ValueType>>(&mut self, _stat_path: &[&str], value: V) {
        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(vals) => { self.base += vals; }
            ValueType::Expression(expression) => { self.mods.push(expression.clone()); }
        }
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, _stat_path: &[&str], value: V) {
        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(vals) => { self.base -= vals; }
            ValueType::Expression(expression) => {
                let Some(pos) = self.mods.iter().position(|e| *e == expression) else { return; };
                self.mods.remove(pos);
            }
        }
    }

    fn evaluate(&self, _stat_path: &[&str], stats: &Stats) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stats.cached_context())).collect();
        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => self.base * computed.iter().product::<f32>(),
        }
    }

    fn on_insert(&self, _stats: &Stats, _stat_path: &[&str]) { }
}

#[derive(Debug)]
pub struct Modifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_steps: HashMap<String, Simple>,
}

impl Modifiable {
    pub fn new(name: &str) -> Self {
        let original_expr = get_total_expr_from_name(name);
        let mut modifier_steps = HashMap::new();
        let modifier_names: Vec<&str> = original_expr.split(|c: char| !c.is_alphabetic())
            .filter(|s| !s.is_empty())
            .collect();
        for modifier_name in modifier_names.iter() {
            let step = Simple::new(modifier_name);
            modifier_steps.insert(modifier_name.to_string(), step);
        }
        let transformed_expr = original_expr.split(|c: char| !c.is_alphabetic())
            .fold(original_expr.to_string(), |expr, word| {
                if modifier_names.contains(&word) {
                    expr.replace(word, &format!("{}_{}", name, word))
                } else {
                    expr
                }
            });
        Modifiable { 
            total: Expression { 
                string: transformed_expr.clone(),
                value: evalexpr::build_operator_tree(&transformed_expr).unwrap(),
            },
            modifier_steps,
        }
    }
}

impl StatLike for Modifiable  {
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() != 2 { return; }
        let key = stat_path[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.add_modifier(stat_path, value);
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() != 2 { return; }
        let key = stat_path[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.remove_modifier(stat_path, value);
    }
    
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32 {
        match stat_path.len() {
            1 => {
                self.total.value
                    .eval_with_context(stats.cached_context())
                    .unwrap()
                    .as_number()
                    .unwrap() as f32
            }
            2 => {
                let Some(part) = self.modifier_steps.get(stat_path[1]) else { return 0.0; };
                part.evaluate(stat_path, stats)
            }
            _ => 0.0
        }
    }
    
    fn on_insert(&self, stats: &Stats, stat_path: &[&str]) {
        let base_name = stat_path[0];
        for (modifier_name, _) in self.modifier_steps.iter() {
            let full_modifier_path = format!("{}_{}", base_name, modifier_name);
            if stats.get_cached(&full_modifier_path).is_err() {
                let val = self.modifier_steps.get(modifier_name).unwrap().evaluate(stat_path, stats);
                stats.set_cached(&full_modifier_path, val);
            }
        }
    }
}

#[derive(Debug)]
pub struct ComplexModifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, HashMap<u32, Simple>>,
}

impl ComplexModifiable {
    pub fn new(name: &str) -> Self {
        Self {
            total: Expression { 
                string: get_total_expr_from_name(name).to_string(), 
                value: evalexpr::build_operator_tree(get_total_expr_from_name(name)).unwrap() 
            }, 
            modifier_types: HashMap::new(),
        }
    }
}

impl StatLike for ComplexModifiable {
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() != 3 { return; }
        let modifier_type = stat_path[1];
        let Ok(tag) = stat_path[2].parse::<u32>() else { return; };
        let step_map = self.modifier_types.entry(modifier_type.to_string())
            .or_insert(HashMap::new());
        let step = step_map.entry(tag).or_insert(Simple::new(modifier_type));
        step.add_modifier(stat_path, value);
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() != 3 { return; }
        let Some(step_map) = self.modifier_types.get_mut(stat_path[1]) else { return; };
        let Ok(tag) = stat_path[2].parse::<u32>() else { return; };
        let Some(step) = step_map.get_mut(&tag) else { return; };
        step.remove_modifier(stat_path, value);
    }
    
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32 {
        let full_path = stat_path.join("_");
        if let Ok(value) = stats.get_cached(&full_path) {
            return value;
        }
        let search_bitflags = match stat_path.get(1) {
            Some(query_str) => query_str.parse::<u32>().unwrap_or(0),
            None => match stat_path.get(2) {
                Some(query_str) => query_str.parse::<u32>().unwrap_or(0),
                None => todo!(),
            },
        };
        let mut context = HashMapContext::new();
        for name in self.total.value.iter_variable_identifiers() {
            let val = get_initial_value_for_modifier(name);
            context.set_value(name.to_string(), Value::Float(val as f64)).unwrap();
        }
        for (category, values) in &self.modifier_types {
            let category_sum: f32 = values
                .iter()
                .filter_map(|(&mod_bitflags, value)| {
                    if (mod_bitflags & search_bitflags) == search_bitflags {
                        if stat_path.len() == 2 {
                            stats.add_dependent(&format!("{}_{}_{}", stat_path[0], category, mod_bitflags.to_string()), &full_path);
                        }
                        Some(value.evaluate(stat_path, stats))
                    } else {
                        None
                    }
                })
                .sum();
            context.set_value(category.clone(), Value::Float(category_sum as f64)).ok();
        }
        let total = self.total.value
            .eval_with_context(&context)
            .unwrap()
            .as_number()
            .unwrap() as f32;
        stats.set_cached(&full_path, total);
        total
    }
    
    fn on_insert(&self, _stats: &Stats, _stat_path: &[&str]) { }
}

#[derive(Debug, Clone)]
pub struct Expression {
    pub string: String,
    pub value: Node<DefaultNumericTypes>,
}

impl Expression {
    pub fn evaluate(&self, context: &HashMapContext) -> f32 {
        self.value
            .eval_with_context(context)
            .unwrap_or(Value::Float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32
    }
}

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        self.string == other.string
    }
}

#[derive(Debug, Clone)]
pub enum ValueType {
    Literal(f32),
    Expression(Expression),
}

impl Default for ValueType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

impl From<&str> for ValueType {
    fn from(value: &str) -> Self {
        Self::Expression(Expression {
            string: value.to_string(),
            value: evalexpr::build_operator_tree(value).unwrap(),
        })
    }
}

impl From<String> for ValueType {
    fn from(value: String) -> Self {
        Self::Expression(Expression {
            string: value.clone(),
            value: evalexpr::build_operator_tree(&value).unwrap(),
        })
    }
}

impl From<f32> for ValueType {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

impl From<u32> for ValueType {
    fn from(value: u32) -> Self {
        Self::Literal(value as f32)
    }
}

fn get_total_expr_from_name(name: &str) -> &'static str {
    match name {
        "Damage" => "Added * Increased * More",
        "Life" => "Added * Increased * More",
        _ => "Added * Increased * More",
    }
}

fn get_initial_value_for_modifier(modifier_type: &str) -> f32 {
    match modifier_type {
        "Added" | "Base" | "Flat" => 0.0,
        "Increased" | "More" | "Multiplier" => 1.0,
        "Override" => 1.0,
        _ => 0.0,
    }
}

stat_macros::define_tags! {
    damage {
        damage_type {
            elemental { fire, cold, lightning },
            physical,
            chaos,
        },
        weapon_type {
            melee { sword, axe },
            ranged { bow, wand },
        },
    }
}

#[cfg(test)]
mod stat_operation_tests {
    use super::*;
    fn assert_approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < f32::EPSILON * 100.0, "left: {}, right: {}", a, b);
    }
    fn test_stats() -> Stats {
        let mut stats = Stats::new();
        stats.add_modifier("Movespeed", 10.0);
        stats.add_modifier("Life_Added", 20.0);
        stats.add_modifier("Life_Increased", 0.1);
        stats.add_modifier(&format!("Damage_Added_{}", (u32::MAX & !Damage::DAMAGE_TYPE) | Damage::FIRE), 5.0);
        stats.add_modifier(&format!("Damage_Added_{}", (u32::MAX & !Damage::DAMAGE_TYPE) | Damage::CHAOS), 8.0);
        stats.add_modifier(&format!("Damage_Added_{}", (u32::MAX & !Damage::WEAPON_TYPE) | Damage::SWORD), 3.0);
        stats.add_modifier(&format!("Damage_Increased_{}", (u32::MAX & !Damage::DAMAGE_TYPE) | Damage::FIRE), 0.2);
        stats.add_modifier(&format!("Damage_Increased_{}", (u32::MAX & !Damage::WEAPON_TYPE) | Damage::SWORD), 0.15);
        stats
    }

    #[test]
    fn test_simple_stat() {
        let stats = test_stats();
        assert_eq!(stats.evaluate("Movespeed"), 10.0);
        let mut stats = test_stats();
        stats.add_modifier("Movespeed", 5.0);
        assert_eq!(stats.evaluate("Movespeed"), 15.0);
        stats.remove_modifier("Movespeed", 3.0);
        assert_eq!(stats.evaluate("Movespeed"), 12.0);
    }

    #[test]
    fn test_modifiable_stat() {
        let stats = test_stats();
        assert_approx_eq(stats.evaluate("Life"), 20.0 * 1.1);
        assert_approx_eq(stats.evaluate("Life_Added"), 20.0);
        assert_approx_eq(stats.evaluate("Life_Increased"), 1.1);
        let mut stats = test_stats();
        stats.add_modifier("Life_Added", 10.0);
        stats.add_modifier("Life_More", 0.1);
        assert_approx_eq(stats.evaluate("Life"), 30.0 * 1.1 * 1.1);
        stats.remove_modifier("Life_Increased", 0.05);
        assert_approx_eq(stats.evaluate("Life_Increased"), 1.05);
    }

    #[test]
    fn test_complex_stat() {
        let stats = test_stats();
        assert_approx_eq(
            stats.evaluate(&format!("Damage_{}", Damage::FIRE | Damage::SWORD)), 
            (5.0 + 3.0) * (1.2 + 1.15)
        );
    }
    
    #[test]
    fn test_expression_stats() {
        let mut stats = test_stats();
        stats.add_modifier("Life_More", "Life_Added / 2.0");
        assert_approx_eq(stats.evaluate("Life"), 20.0 * 1.1 * (1.0 + (20.0 / 2.0)));
    }
    
    #[test]
    fn test_dependent_stats() {
        let mut stats = test_stats();
        stats.add_modifier("Life_More", "Movespeed * 0.1");
        assert_approx_eq(stats.evaluate("Life"), 20.0 * 1.1 * (1.0 + 10.0 * 0.1));
        let dependents = stats.get_dependents("Movespeed");
        assert!(!dependents.is_empty(), "Dependency not registered");
        let life_dependency = dependents.iter().find(|dep| *dep == "Life_More");
        assert!(life_dependency.is_some(), "Life_More should depend on Movespeed");
        stats.add_modifier("Life_More", "Movespeed * 0.05");
        stats.add_modifier("Movespeed", 10.0);
        assert_approx_eq(stats.evaluate("Life"), 20.0 * 1.1 * (1.0 + 20.0 * 0.15));
        stats.remove_modifier("Life_More", "Movespeed * 0.05");
        assert_approx_eq(stats.evaluate("Life"), 20.0 * 1.1 * (1.0 + 20.0 * 0.1));
        stats.remove_modifier("Life_More", "Movespeed * 0.1");
        let dependents = stats.get_dependents("Movespeed");
        assert!(!dependents.iter().any(|dep| dep == "Life"), "Life dependency should be completely removed when weight reaches 0");
    }
    
    #[test]
    fn test_stat_removal() {
        let mut stats = test_stats();
        stats.remove_modifier("Movespeed", 5.0);
        assert_approx_eq(stats.evaluate("Movespeed"), 5.0);
        stats.remove_modifier("Life_Added", 10.0);
        assert_approx_eq(stats.evaluate("Life_Added"), 10.0);
        stats.remove_modifier(&format!("Damage_Added_{}", (u32::MAX & !Damage::DAMAGE_TYPE) | Damage::FIRE), 3.0);
        assert_approx_eq(
            stats.evaluate(&format!("Damage_{}", Damage::FIRE | Damage::SWORD)), 
            (5.0 - 3.0 + 3.0) * (1.2 + 1.15)
        );
    }

    #[test]
    fn test_empty_stats() {
        let stats = Stats::new();
        assert_eq!(stats.evaluate("Nonexistent"), 0.0);
        assert_eq!(stats.evaluate("Damage_1"), 0.0);
        assert_eq!(stats.evaluate("Life_Added"), 0.0);
    }

    #[test]
    fn test_stat_type_creation() {
        let simple = StatType::new("Test", 10.0);
        assert!(matches!(simple, StatType::Simple(_)));
        let modifiable = StatType::new("Test_Added", 5.0);
        assert!(matches!(modifiable, StatType::Modifiable(_)));
        let complex = StatType::new("Test_Added_1", 3.0);
        assert!(matches!(complex, StatType::Complex(_)));
    }
}

#[cfg(test)]
mod thread_safety_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn setup_stats() -> Arc<Stats> {
        let mut stats = Stats::new();
        stats.add_modifier("AttackSpeed", 1.0);
        stats.add_modifier(&format!("Damage_Added_{}", u32::MAX), 10.0);
        stats.add_modifier(&format!("Damage_Added_{}", Damage::FIRE | Damage::SWORD), 5.0);
        stats.add_modifier("FireSwordDPS", format!("Damage_{} * AttackSpeed", Damage::FIRE | Damage::SWORD));
        Arc::new(stats)
    }

    #[test]
    fn test_concurrent_stat_evaluation() {
        let stats = setup_stats();
        let mut handles = vec![];
        for _ in 0..10 {
            let stats = Arc::clone(&stats);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let dps = stats.evaluate("FireSwordDPS");
                    assert!(dps >= 10.0);
                    let fire_sword = stats.evaluate(&format!("Damage_{}", Damage::FIRE | Damage::SWORD));
                    assert!(fire_sword >= 15.0);
                }
            }));
        }
        for handle in handles { handle.join().unwrap(); }
    }

    #[test]
    fn test_concurrent_cache_population() {
        let mut stats = Stats::new();
        for i in 0..10 {
            let damage_type = if i % 2 == 0 { Damage::FIRE } else { Damage::COLD };
            let weapon_type = if i < 5 { Damage::SWORD } else { Damage::BOW };
            let stat_key = format!("Damage_Added_{}", damage_type | weapon_type);
            stats.add_modifier(stat_key, 5.0);
        }
        let mut handles = vec![];
        let stats = Arc::new(stats);
        for i in 0..10 {
            let stats = Arc::clone(&stats);
            handles.push(thread::spawn(move || {
                let damage_type = if i % 2 == 0 { Damage::FIRE } else { Damage::COLD };
                let weapon_type = if i < 5 { Damage::SWORD } else { Damage::BOW };
                let _result = stats.evaluate(&format!("Damage_{}", damage_type | weapon_type));
            }));
        }
        for handle in handles { handle.join().unwrap(); }
        for i in 0..10 {
            let damage_type = if i % 2 == 0 { Damage::FIRE } else { Damage::COLD };
            let weapon_type = if i < 5 { Damage::SWORD } else { Damage::BOW };
            let key = format!("Damage_{}", damage_type | weapon_type);
            assert!(stats.get_cached(&key).is_ok());
        }
    }

    #[test]
    fn test_concurrent_complex_stat_evaluation() {
        let stats = Arc::new({
            let mut s = Stats::new();
            s.add_modifier(&format!("Damage_Added_{}", u32::MAX), 5.0);
            s.add_modifier(&format!("Damage_Added_{}", Damage::FIRE | Damage::SWORD), 5.0);
            s
        });
        let mut handles = vec![];
        for i in 0..10 {
            let stats = Arc::clone(&stats);
            handles.push(thread::spawn(move || {
                let damage_type = if i % 2 == 0 { Damage::FIRE } else { Damage::COLD };
                let weapon_type = if i < 5 { Damage::SWORD } else { Damage::BOW };
                for _ in 0..50 {
                    let result = stats.evaluate(&format!("Damage_{}", damage_type | weapon_type));
                    assert!(result >= 0.0);
                }
            }));
        }
        for handle in handles { handle.join().unwrap(); }
    }

    #[test]
    fn test_concurrent_dependent_evaluation() {
        let stats = Arc::new({
            let mut s = Stats::new();
            s.add_modifier("Base", 10.0);
            s.add_modifier("Derived1", "Base * 2");
            s.add_modifier("Derived2", "Derived1 + 5");
            s
        });
        let mut handles = vec![];
        for stat in &["Base", "Derived1", "Derived2"] {
            let stats = Arc::clone(&stats);
            let stat = stat.to_string();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let value = stats.evaluate(&stat);
                    assert!(value > 0.0);
                }
            }));
        }
        for handle in handles { handle.join().unwrap(); }
        assert_eq!(stats.evaluate("Base"), 10.0);
        assert_eq!(stats.evaluate("Derived1"), 20.0);
        assert_eq!(stats.evaluate("Derived2"), 25.0);
    }
}
