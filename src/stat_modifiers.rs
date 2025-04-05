use std::{cell::SyncUnsafeCell, sync::{Arc, RwLock}};
use bevy::{core_pipeline::prepass::OpaqueNoLightmap3dBinKey, ecs::system::SystemParam, prelude::*, utils::{HashMap, HashSet}};
use evalexpr::{Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};
use crate::{error::StatError, tags::TagLike};

// TODO Fix overuse of .unwrap(). It's fine for now (maybe preferable during development) but in the future we'll want proper errors, panics, and warnings.

// TODO ContextDrivenStats type that wraps stats, but contains a context (Hashmap of strings to entities). Can only call evaluate on it if you pass in a StatContextRefs

// TODO Stats.definitions should match String -> T where T implements StatLike. Convert the current StatType into DefaultStatType.

// TODO Systemetize asset-like definitions.
//     - get_total_expr_from_name
//     - get_initial_value_for_modifier
//     - match strings to sets of tags, i.e., "damage" -> Damage

// TODO wrapper for u32 that lets us conveniently do queries (HasTag, HasAny, HasAll). Possibly change ComplexModifiable to take type T where T implements TagLike

// TODO Implement fasteval instead of evalexpr

// TODO Consider some scheme to avoid having to parse and reparse these strings. FName could be some inspiration. Why am I splitting and un-splitting these strings
// during stat operations? 
//     - One thing to consider is a type that behaves like an address. Basically turn strings into u32's with some FName-like implementation, and an array of u32s
//       is a 'path.' Then you could key the context (and everything else that currently uses strings) on this vec-of-u32s type.

// TODO Build some examples 
//     - Path of Exile
//     - World of Warcraft
//     - Dungeons and Dragons
//     - Halo

// TODO Reintegrate with other stats code
//     - StatContext
//     - StatEffect
//     - StatRequirements

// StatPath struct to handle path parsing and avoid repetitive string operations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatPath {
    path: String,
    owner: Option<String>,
    segments: Vec<String>,
}

impl StatPath {
    pub fn parse(string: &str) -> Self {
        let (owner, segments) = if string.contains('@') {
            let parts: Vec<&str> = string.split('@').collect();
            let owner = Some(parts[0].to_string());
            let segments = parts[1].split('_').map(|s| s.to_string()).collect();
            (owner, segments)
        } else {
            let segments = string.split('_').map(|s| s.to_string()).collect();
            (None, segments)
        };
        Self { 
            path: string.to_string(), 
            owner, 
            segments,
        }
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn to_string(&self) -> String {
        self.path.clone()
    }

    pub fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    pub fn segments(&self) -> Vec<&str> {
        self.segments.iter().map(|s| s.as_str()).collect()
    }

    pub fn base(&self) -> Option<&str> {
        self.segments.first().map(|s| s.as_str())
    }

    pub fn with_owner(stat_path: &str, owner_prefix: &str) -> String {
        format!("{}@{}", owner_prefix, stat_path)
    }
}

pub fn to_path_string(segments: &[&str]) -> String {
    segments.join("_")
}

#[derive(Debug, Clone, Default)]
pub enum ModType {
    #[default]
    Add,
    Mul,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DependentType {
    LocalStat(String),
    EntityStat(Entity), // Entity that depends on this stat
}

// SystemParam for accessing stats from systems
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    stats_query: Query<'w, 's, &'static mut Stats>,
}

impl StatAccessor<'_, '_> {
    // Public API: Get the value of a stat
    pub fn get(&self, target_entity: Entity, stat_path: &str) -> f32 {
        let Ok(stats) = self.stats_query.get(target_entity) else {
            return 0.0;
        };

        stats.get(stat_path).unwrap_or(0.0)
    }

    // Public API: Add a modifier to a stat
    pub fn add_modifier<V: Into<ValueType> + Clone>(&mut self, target_entity: Entity, stat_path: &str, modifier: V) {
        let stat_path = StatPath::parse(stat_path);
        // Lets cover a few possible setups: 
        //    case 1: target_entity: ability_entity, stat_path: "Damage_Added_1", modifier: "Invoker@Added_Life * 0.5"
        let vt: ValueType = modifier.into();
        
        if !self.stats_query.contains(target_entity) {
            return;
        }
        
        match vt {
            ValueType::Literal(value) => {
                if let Ok(mut target_stats) = self.stats_query.get_mut(target_entity) {
                    target_stats.add_modifier(&stat_path, value);
                }
            },
            // case 1: the modifier is an expression, and may depend on other stats. 
            ValueType::Expression(expression) => {
                let mut dependencies_info = Vec::new();
                let mut dependents_to_add = Vec::new();
                
                {
                    let Ok(target_stats) = self.stats_query.get(target_entity) else {
                        return;
                    };
                    
                    // case 1: iter through each variable in the expression. We find "Invoker@Life_Added"
                    for depends_on in expression.value.iter_variable_identifiers() {

                        // case 1: "Invoker@Life_Added" contains and @!
                        if depends_on.contains('@') {
                            
                            // case 1: We split the strings into 2 parts: head: "Invoker", dependency_stat_path: "Life_Added"
                            let depends_on_segments: Vec<&str> = depends_on.split('@').collect();
                            let head = depends_on_segments[0];
                            let dependency_stat_path = depends_on_segments[1];
                            
                            // case 1: We get the entity that matches the "Invoker". We will call this the invoker_entity.
                            //         We cache the dependency info and dependents-to-add for later use. This is to get around
                            //         the borrow checker.
                            if let Some(&depends_on_entity) = target_stats.dependent_on.get(head) {
                                // case 1: "Invoker@Added_Life", invoker_entity, "Life_Added"
                                dependencies_info.push((
                                    depends_on.to_string(),
                                    depends_on_entity,
                                    dependency_stat_path.to_string(),
                                ));
                                
                                // case 1: invoker_entity, "Life_Added", ability_entity
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
                                
                // case 1: Get the ability_entity's stats. Add the modifier "Invoker@Added_Life * 0.5" to the stat.
                if let Ok(mut target_stats) = self.stats_query.get_mut(target_entity) {
                    target_stats.add_modifier(&stat_path, expression);
                }

                // case 1: We iter through and find a single entry. ("Invoker@Added_Life", invoker_entity, "Life_Added")
                //         We get the invoker's stats and put the output value into our dependencies_to_cache vec as
                //         ("Invoker@Added_Life", 100.0) because the invoker has 100 added life.
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
                
                // case 1: Get the ability_entity's stats. Iter through dependencies and cache them. 
                if let Ok(target_stats) = self.stats_query.get(target_entity) {
                    for (depends_on, value) in dependencies_to_cache {
                        target_stats.cache_stat(&depends_on, value);
                    }
                }
                
                for (depends_on_entity, dependency_stat_path, dependent_type) in dependents_to_add {
                    if let Ok(depends_on_stats) = self.stats_query.get(depends_on_entity) {
                        depends_on_stats.add_dependent_internal(&dependency_stat_path, dependent_type);
                    }
                }
            },
        }

        self.update_stat(target_entity, &stat_path);
    }

    // Public API: Remove a modifier from a stat
    pub fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, target_entity: Entity, stat_path: &str, modifier: V) {
        let stat_path = StatPath::parse(stat_path);

        let vt: ValueType = modifier.into();

        let Ok(mut target_stats) = self.stats_query.get_mut(target_entity) else {
            return;
        };

        match vt {
            ValueType::Literal(value) => {
                target_stats.remove_modifier(&stat_path, value);
            },
            ValueType::Expression(expression) => {
                // First, collect all the dependencies to remove
                let mut dependencies_to_remove = Vec::new();
                
                for depends_on in expression.value.iter_variable_identifiers() {
                    let depends_on = StatPath::parse(depends_on);
                    if let Some(head) = depends_on.owner {
                        let head = &depends_on.segments[0]; // "Invoker"
                        let dependency_stat_path = &depends_on.segments[1]; // "Life_Added"
                        
                        if let Some(&depends_on_entity) = target_stats.dependent_on.get(head) {
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
                
                // Remove the expression modifier
                target_stats.remove_modifier(&stat_path, expression);
                
                // Release the mutable borrow to target_stats
                drop(target_stats);
                
                // Now remove all dependencies
                for (depends_on_entity, dependency_stat_path, dependent_type) in dependencies_to_remove {
                    if let Ok(depends_on_stats) = self.stats_query.get(depends_on_entity) {
                        depends_on_stats.remove_dependent_internal(&dependency_stat_path, dependent_type);
                    }
                }
            },
        }

        self.update_stat(target_entity, &stat_path);
    }

    // Public API: Register an entity dependency
    pub fn register_dependency(&mut self, target_entity: Entity, name: &str, dependency_entity: Entity) {
        if let Ok(mut stats) = self.stats_query.get_mut(target_entity) {
            stats.dependent_on.insert(name.to_string(), dependency_entity);
        }
    }

    // Public API: Evaluate a stat
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
        
        let mut current_value = 0.0;
        if let Ok(stats) = self.stats_query.get(target_entity) {
            current_value = stats.evaluate(stat_path);
            
            let full_path = stat_path.to_string();
            stats.set_cached(&full_path, current_value);
        }
        
        processed.insert(process_key);
        
        let mut local_dependents = Vec::new();
        let mut entity_dependents = Vec::new();
        
        if let Ok(stats) = self.stats_query.get(target_entity) {
            let dependents = stats.get_dependents_internal(&stat_path.to_string());
            
            for dependent in dependents {
                match dependent {
                    DependentType::LocalStat(local_stat) => {
                        let dependent_path = StatPath::parse(&local_stat);
                        local_dependents.push(dependent_path);
                    },
                    DependentType::EntityStat(dependent_entity) => {
                        entity_dependents.push(dependent_entity);
                    }
                }
            }
        }
        
        for local_dependent in local_dependents {
            self.update_stat_recursive(target_entity, &local_dependent, processed);
        }
        
        for dependent_entity in entity_dependents {
            let mut stats_to_update = Vec::new();
            
            if let Ok(dependent_stats) = self.stats_query.get(dependent_entity) {
                let mut prefixes = Vec::new();
                for (prefix, &entity) in &dependent_stats.dependent_on {
                    if entity == target_entity {
                        prefixes.push(prefix.clone());
                    }
                }
                
                for prefix in prefixes {
                    let cache_key = format!("{}@{}", prefix, stat_path.path);
                    
                    dependent_stats.set_cached(&cache_key, current_value);
                    
                    let cache_dependents = dependent_stats.get_dependents_internal(&cache_key);
                    for cache_dependent in cache_dependents {
                        if let DependentType::LocalStat(dependent_stat) = cache_dependent {
                            let stat_path = StatPath::parse(&dependent_stat);
                            stats_to_update.push(stat_path);
                        }
                    }
                }
            }
            
            for stat_to_update in stats_to_update {
                self.update_stat_recursive(dependent_entity, &stat_to_update, processed);
            }
        }
    }
}

/// A collection of stats keyed by their names.
#[derive(Component, Debug, Default)]
pub struct Stats {
    // Holds the definitions of stats. This includes default values, their modifiers, and their dependents
    definitions: HashMap<String, StatType>,
    cached_stats: SyncContext,
    dependency_graph: SyncDependents,
    dependent_on: HashMap<String, Entity>,
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
struct SyncDependents(Arc<RwLock<HashMap<String, HashMap<DependentType, u32>>>>);

impl SyncDependents {
    fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
   
    fn add_dependent(&self, stat_path: &str, dependent: DependentType) {
        let mut graph = self.0.write().unwrap();
        let entry = graph.entry(stat_path.to_string()).or_insert(HashMap::new());
        *entry.entry(dependent).or_insert(0) += 1;
    }
    
    fn remove_dependent(&self, stat_path: &str, dependent: DependentType) {
        let Ok(mut graph) = self.0.write() else {
            return;
        };
        let Some(dependents) = graph.get_mut(stat_path) else {
            return;
        };
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
    
    fn get_dependents(&self, stat_path: &str) -> Vec<DependentType> {
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
            dependent_on: HashMap::new(),
        }
    }

    // Internal methods, not part of public API

    fn get(&self, stat_path: &str) -> Result<f32, StatError> {
        self.cached_stats.get(stat_path)
    }

    fn get_cached(&self, key: &str) -> Result<f32, StatError> {
        self.cached_stats.get(key)
    }

    fn set_cached(&self, key: &str, value: f32) {
        self.cached_stats.set(key, value)
    }

    fn cached_context(&self) -> &HashMapContext {
        self.cached_stats.context()
    }

    fn add_dependent_internal(&self, stat: &str, dependent: DependentType) {
        self.dependency_graph.add_dependent(stat, dependent);
    }

    fn remove_dependent_internal(&self, stat: &str, dependent: DependentType) {
        self.dependency_graph.remove_dependent(stat, dependent);
    }

    fn get_dependents_internal(&self, stat: &str) -> Vec<DependentType> {
        self.dependency_graph.get_dependents(stat)
    }

    fn evaluate_by_string(&self, stat_path: &str) -> f32 {
        let stat_path = StatPath::parse(stat_path);
        self.evaluate(&stat_path)
    }

    fn evaluate(&self, stat_path: &StatPath) -> f32 {
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

    fn add_modifier<V>(&mut self, stat_path: &StatPath, value: V)
    where
        V: Into<ValueType> + Clone,
    {
        if stat_path.segments.is_empty() {
            return;
        }
        
        let base_stat = stat_path.segments[0].to_string();

        {
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.add_modifier(stat_path, value.clone());
            } else {
                let new_stat = StatType::new(&stat_path.path, value.clone());
                new_stat.on_insert(self, stat_path);
                self.definitions.insert(base_stat.clone(), new_stat);
            }
            let vt: ValueType = value.into();
            if let ValueType::Expression(depends_on_expression) = vt {
                self.register_dependencies(stat_path, &depends_on_expression);
            }
        }
    }

    fn remove_modifier<V>(&mut self, stat_path: &StatPath, value: V)
    where
        V: Into<ValueType> + Clone,
    {
        if stat_path.segments.is_empty() {
            return;
        }
        
        let base_stat = stat_path.segments[0].to_string();

        {
            if let Some(stat) = self.definitions.get_mut(&base_stat) {
                stat.remove_modifier(stat_path, value.clone());
            }
            let vt: ValueType = value.into();
            if let ValueType::Expression(expression) = vt {
                self.unregister_dependencies(&base_stat, &expression);
            }
        }
    }

    fn register_dependencies(&self, stat_path: &StatPath, depends_on_expression: &Expression) {
        for var_name in depends_on_expression.value.iter_variable_identifiers() {
            self.evaluate(stat_path);
            self.add_dependent_internal(var_name, DependentType::LocalStat(stat_path.path.to_string()));
        }
    }

    fn unregister_dependencies(&self, dependent_stat: &str, depends_on_expression: &Expression) {
        for depends_on_stat in depends_on_expression.value.iter_variable_identifiers() {
            self.remove_dependent_internal(depends_on_stat, DependentType::LocalStat(dependent_stat.to_string()));
        }
    }

    // Helper method to store an entity-dependent stat value
    fn cache_stat(&self, key: &str, value: f32) {
        self.set_cached(key, value);
    }
}

pub trait StatLike {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V);
    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V);
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32;
    fn on_insert(&self, stats: &Stats, stat_path: &StatPath);
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
        V: Into<ValueType> + Clone,
    {
        let stat_path = StatPath::parse(stat_path);
        match stat_path.segments.len() {
            1 => {
                let mut stat = Simple::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value);
                StatType::Simple(stat)
            },
            2 => {
                let mut stat = Modifiable::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value);
                StatType::Modifiable(stat)
            },
            3 => {
                let mut stat = ComplexModifiable::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value);
                StatType::Complex(stat)
            },
            _ => panic!("Invalid stat path format: {:#?}", stat_path)
        }
    }
}

impl StatLike for StatType {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        match self {
            StatType::Simple(simple) => simple.add_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.add_modifier(stat_path, value),
        }
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        match self {
            StatType::Simple(simple) => simple.remove_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.remove_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.remove_modifier(stat_path, value),
        }
    }
    
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32 {
        match self {
            StatType::Simple(simple) => simple.evaluate(stat_path, stats),
            StatType::Modifiable(modifiable) => modifiable.evaluate(stat_path, stats),
            StatType::Complex(complex_modifiable) => complex_modifiable.evaluate(stat_path, stats),
        }
    }
    
    fn on_insert(&self, stats: &Stats, stat_path: &StatPath) {
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
        //let base = get_initial_value_for_modifier(name);
        Self { relationship: ModType::Add, base: 0.0, mods: Vec::new() }
    }
}

impl StatLike for Simple {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, _stat_path: &StatPath, value: V) {
        let vt: ValueType = value.into();

        match vt {
            ValueType::Literal(vals) => { self.base += vals; }
            ValueType::Expression(expression) => { self.mods.push(expression.clone()); }
        }
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, _stat_path: &StatPath, value: V) {
        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(vals) => { self.base -= vals; }
            ValueType::Expression(expression) => {
                let Some(pos) = self.mods.iter().position(|e| *e == expression) else { return; };
                self.mods.remove(pos);
            }
        }
    }

    fn evaluate(&self, _stat_path: &StatPath, stats: &Stats) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stats.cached_context())).collect();
        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => self.base * computed.iter().product::<f32>(),
        }
    }

    fn on_insert(&self, _stats: &Stats, _stat_path: &StatPath) { }
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
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 2 { return; }
        let key = stat_path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.add_modifier(stat_path, value);
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 2 { return; }
        let key = stat_path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.remove_modifier(stat_path, value);
    }
    
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32 {
        match stat_path.len() {
            1 => {
                self.total.value
                    .eval_with_context(stats.cached_context())
                    .unwrap()
                    .as_number()
                    .unwrap() as f32
            }
            2 => {
                let Some(part) = self.modifier_steps.get(&stat_path.segments[1]) else { return 0.0; };
                part.evaluate(stat_path, stats)
            }
            _ => 0.0
        }
    }
    
    fn on_insert(&self, stats: &Stats, stat_path: &StatPath) {
        if stat_path.segments.is_empty() { return; }
        let base_name = &stat_path.segments[0];
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
pub struct ComplexEntry(f32, HashMap<u32, Simple>);

#[derive(Debug)]
pub struct ComplexModifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, ComplexEntry>,
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
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 3 { return; }
        let modifier_type = &stat_path.segments[1];
        let Ok(tag) = stat_path.segments[2].parse::<u32>() else { return; };
        let step_map = self.modifier_types.entry(modifier_type.to_string())
            .or_insert(ComplexEntry(get_initial_value_for_modifier(modifier_type), HashMap::new()));
        let step = step_map.1.entry(tag).or_insert(Simple::new(modifier_type));
        step.add_modifier(stat_path, value);
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 3 { return; }
        let Some(step_map) = self.modifier_types.get_mut(&stat_path.segments[1]) else { return; };
        let Ok(tag) = stat_path.segments[2].parse::<u32>() else { return; };
        let Some(step) = step_map.1.get_mut(&tag) else { return; };
        step.remove_modifier(stat_path, value);
    }
    
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32 {
        let full_path = &stat_path.path;

        if let Ok(search_tags) = stat_path.segments.get(1).unwrap().parse::<u32>() {
            let mut context = HashMapContext::new();
            for name in self.total.value.iter_variable_identifiers() {
                let val = get_initial_value_for_modifier(name);
                context.set_value(name.to_string(), Value::Float(val as f64)).unwrap();
            }
            for (category, values) in &self.modifier_types {
                let category_sum: f32 = values.1
                    .iter()
                    .filter_map(|(&mod_tags, value)| {
                        if mod_tags.has_all(search_tags) {
                            let dep_path = format!("{}_{}_{}", stat_path.segments[0], category, mod_tags.to_string());
                            stats.add_dependent_internal(&dep_path, DependentType::LocalStat(full_path.to_string()));
                            Some(value.evaluate(stat_path, stats))
                        } else {
                            None
                        }
                    })
                    .sum();
                context.set_value(category.clone(), Value::Float((category_sum + values.0) as f64)).ok();
            }
            let total = self.total.value
                .eval_with_context(&context)
                .unwrap()
                .as_number()
                .unwrap() as f32;
            stats.set_cached(&full_path, total);
            return total;
        }

        if let Ok(search_tags) = stat_path.segments.get(2).unwrap().parse::<u32>() {
            let category = &stat_path.segments[1];
            let Some(values) = self.modifier_types.get(category) else {
                return 0.0;
            };

            return values.1
                .iter()
                .filter_map(|(&mod_tags, value)| {
                    if mod_tags.has_all(search_tags) {
                        let dep_path = format!("{}_{}_{}", stat_path.segments[0], category, mod_tags.to_string());
                        stats.add_dependent_internal(&dep_path, DependentType::LocalStat(full_path.to_string()));
                        Some(value.evaluate(stat_path, stats))
                    } else {
                        None
                    }
                })
                .sum();
        }

        return 0.0;
    }
    
    fn on_insert(&self, _stats: &Stats, _stat_path: &StatPath) { }
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

impl From<Expression> for ValueType {
    fn from(value: Expression) -> Self {
        Self::Expression(value)
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

// ******************************************************************
// Asset-like
// ******************************************************************

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
mod tests {
    use bevy::prelude::*;
    use crate::stat_modifiers::{Stats, StatAccessor};

    // Helper function for approximate equality checks
    fn assert_approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < f32::EPSILON * 100.0, "left: {}, right: {}", a, b);
    }

    // Helper system to add a modifier to a stat
    fn add_stat_modifier(
        mut stat_accessor: StatAccessor,
        query: Query<Entity, With<Stats>>,
    ) {
        for entity in &query {
            stat_accessor.add_modifier(entity, "Life_Added", 10.0);
        }
    }

    // Helper system to add a dependent modifier
    fn add_dependent_modifier(
        mut stat_accessor: StatAccessor,
        query: Query<Entity, With<Stats>>,
    ) {
        for entity in &query {
            // Add a modifier that depends on Life_Added
            stat_accessor.add_modifier(entity, "Damage_Added", "Life_Added * 0.5");
        }
    }

    // Helper system to add an entity-dependent modifier
    fn add_entity_dependency(
        mut stat_accessor: StatAccessor,
        query: Query<Entity, With<Stats>>,
    ) {
        if let Some(entity_iter) = query.iter().collect::<Vec<_>>().get(0..2) {
            let source = entity_iter[0];
            let target = entity_iter[1];
            
            // Register the dependency (source is known as "Source" to target)
            stat_accessor.register_dependency(target, "Source", source);
            
            // Add a modifier that depends on the source entity's Life_Added
            stat_accessor.add_modifier(target, "Damage_Added", "Source@Life_Added * 0.25");
        }
    }

    // Test simple stat creation and access
    #[test]
    fn test_add_simple_stat() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system once
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);

        // Check if the stat was added correctly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let value = stats.get("Life_Added").unwrap_or(0.0);
        assert_eq!(value, 10.0);
    }

    // Test evaluating a simple stat
    #[test]
    fn test_evaluate_simple_stat() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system once
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);

        // Query the Stats component directly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let value = stats.get("Life_Added").unwrap_or(0.0);
        assert_eq!(value, 10.0);
    }

    // Test dependent stats within one entity
    #[test]
    fn test_dependent_stats() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the systems in sequence
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_dependent_modifier);
        let _ = app.world_mut().run_system(add_dep_id);

        // Query the Stats component directly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let life_value = stats.get("Life_Added").unwrap_or(0.0);
        let damage_value = stats.evaluate_by_string("Damage_Added"); // Use evaluate_by_string for expressions
        
        assert_eq!(life_value, 10.0);
        assert_eq!(damage_value, 5.0); // Should be half of Life_Added
    }

    // Test inter-entity dependencies
    #[test]
    fn test_entity_dependent_stats() {
        // Setup app
        let mut app = App::new();

        // Spawn two entities with Stats component
        let source_entity = app.world_mut().spawn(Stats::new()).id();
        let target_entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the systems in sequence
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_entity_dependency);
        let _ = app.world_mut().run_system(add_dep_id);

        // Check if entity-dependent stat was calculated correctly
        let [source_stats, target_stats] = app.world_mut().query::<&Stats>().get_many(app.world(), [source_entity, target_entity]).unwrap();
        
        let source_life = source_stats.get("Life_Added").unwrap_or(0.0);
        let target_damage = target_stats.evaluate_by_string("Damage_Added"); // Use evaluate for expressions
        
        assert_eq!(source_life, 10.0);
        assert_eq!(target_damage, 2.5); // Should be 0.25 * source Life_Added
    }

    // Test update propagation through stat dependencies
    #[test]
    fn test_update_propagates_to_dependents() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the initial setup systems
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_dependent_modifier);
        let _ = app.world_mut().run_system(add_dep_id);
        
        // Verify initial values
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let initial_damage = stats.evaluate_by_string("Damage_Added");
        assert_eq!(initial_damage, 5.0);
        
        // Register and run a system to increase Life_Added
        let increase_life_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(entity, "Life_Added", 5.0);
        });
        let _ = app.world_mut().run_system(increase_life_id);
        
        // Check if dependent stat was updated
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let updated_life = updated_stats.get("Life_Added").unwrap_or(0.0);
        let updated_damage = updated_stats.evaluate_by_string("Damage_Added");
        
        assert_eq!(updated_life, 15.0); // Original 10 + 5 added
        assert_eq!(updated_damage, 7.5); // Should be half of updated Life_Added
    }

    // Test updating source entity affects dependent entity
    #[test]
    fn test_updating_source_updates_dependent() {
        // Setup app
        let mut app = App::new();

        // Spawn two entities with Stats component
        let source_entity = app.world_mut().spawn(Stats::new()).id();
        let target_entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the initial setup systems
        let add_mod_id = app.world_mut().register_system(add_stat_modifier);
        let _ = app.world_mut().run_system(add_mod_id);
        
        let add_dep_id = app.world_mut().register_system(add_entity_dependency);
        let _ = app.world_mut().run_system(add_dep_id);
        
        // Verify initial values
        let [source_stats, target_stats] = app.world_mut().query::<&Stats>().get_many(app.world(), [source_entity, target_entity]).unwrap();
        
        let initial_source_life = source_stats.get("Life_Added").unwrap_or(0.0);
        let initial_target_damage = target_stats.evaluate_by_string("Damage_Added");
        
        assert_eq!(initial_source_life, 10.0);
        assert_eq!(initial_target_damage, 2.5); // Should be 0.25 * source Life_Added
        
        // Register and run a system to update the source entity
        let update_source_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(source_entity, "Life_Added", 10.0);
        });
        let _ = app.world_mut().run_system(update_source_id);
        
        // Check if target entity's stat was updated
        let [source_stats, target_stats] = app.world_mut().query::<&Stats>().get_many(app.world(), [source_entity, target_entity]).unwrap();
        
        let updated_source_life = source_stats.get("Life_Added").unwrap_or(0.0);
        let updated_target_damage = target_stats.evaluate_by_string("Damage_Added");
        
        assert_eq!(updated_source_life, 20.0); // Original 10 + 10 added
        assert_eq!(updated_target_damage, 5.0); // Should be 0.25 * updated source Life_Added
    }

    // Test complex modifiable stat with tags
    #[test]
    fn test_complex_modifiable_stat() {
        // Define bit flags similar to the damage_type_tags test
        const TAG1: u32 = 0x01;
        const TAG2: u32 = 0x02;
        const TAG3: u32 = 0x04;
        
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system that adds complex stats
        let add_complex_stat_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Add stats with bitflag tags
                stat_accessor.add_modifier(entity, &format!("Damage_Added_{}", TAG1), 10.0); // Tag 1
                stat_accessor.add_modifier(entity, &format!("Damage_Added_{}", TAG2), 5.0);  // Tag 2
                stat_accessor.add_modifier(entity, &format!("Damage_Added_{}", TAG3), 15.0); // Tag 3
            }
        });
        let _ = app.world_mut().run_system(add_complex_stat_id);

        // Check complex stat values by tag
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        // Use evaluate_by_string with the bitflag format
        let tag1_value = stats.evaluate_by_string(&format!("Damage_{}", TAG1));
        let tag2_value = stats.evaluate_by_string(&format!("Damage_{}", TAG2));
        let tag3_value = stats.evaluate_by_string(&format!("Damage_{}", TAG3));
        
        // Check that each tag evaluates to its own value
        assert_eq!(tag1_value, 10.0);
        assert_eq!(tag2_value, 5.0);
        assert_eq!(tag3_value, 15.0);
        
        // Additional test for combined tags - they should not be combined in this case
        // since we're testing individual tag access
        let combined_value = stats.evaluate_by_string(&format!("Damage_{}", TAG1 | TAG2));
        assert_eq!(combined_value, 0.0); // No value is set for the combined tags
    }

    // Test modifier removal
    #[test]
    fn test_modifier_removal() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to add a modifier
        let add_mod_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Life_Added", 10.0);
            }
        });
        let _ = app.world_mut().run_system(add_mod_id);
        
        // Verify the initial value
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let initial_value = stats.get("Life_Added").unwrap_or(0.0);
        assert_eq!(initial_value, 10.0);
        
        // Register and run a system to remove the modifier
        let remove_mod_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.remove_modifier(entity, "Life_Added", 10.0);
        });
        let _ = app.world_mut().run_system(remove_mod_id);
        
        // Check if modifier was removed correctly
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        let updated_value = updated_stats.get("Life_Added").unwrap_or(0.0);
        assert_eq!(updated_value, 0.0);
    }

    // Test multi-level dependency chain
    #[test]
    fn test_multi_level_dependency_chain() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to set up the dependency chain
        let setup_chain_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Base", 10.0);
                stat_accessor.add_modifier(entity, "Level1", "Base * 2");
                stat_accessor.add_modifier(entity, "Level2", "Level1 + 5");
                stat_accessor.add_modifier(entity, "Level3", "Level2 * 1.5");
            }
        });
        let _ = app.world_mut().run_system(setup_chain_id);
        
        // Verify the dependency chain values
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let base_value = stats.evaluate_by_string("Base");
        let level1_value = stats.evaluate_by_string("Level1");
        let level2_value = stats.evaluate_by_string("Level2");
        let level3_value = stats.evaluate_by_string("Level3");
        
        assert_eq!(base_value, 10.0);
        assert_eq!(level1_value, 20.0); // Base * 2
        assert_eq!(level2_value, 25.0); // Level1 + 5
        assert_eq!(level3_value, 37.5); // Level2 * 1.5
        
        // Register and run a system to update the base value
        let update_base_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(entity, "Base", 5.0);
        });
        let _ = app.world_mut().run_system(update_base_id);
        
        // Check if all levels in the chain were updated
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let updated_base = updated_stats.evaluate_by_string("Base");
        let updated_level1 = updated_stats.evaluate_by_string("Level1");
        let updated_level2 = updated_stats.evaluate_by_string("Level2");
        let updated_level3 = updated_stats.evaluate_by_string("Level3");
        
        assert_eq!(updated_base, 15.0); // Original 10 + 5 added
        assert_eq!(updated_level1, 30.0); // Updated Base * 2
        assert_eq!(updated_level2, 35.0); // Updated Level1 + 5
        assert_eq!(updated_level3, 52.5); // Updated Level2 * 1.5
    }

    // Test with damage type tags similar to your existing tests
    #[test]
    fn test_damage_type_tags() {
        // Define constants similar to your Damage enum
        const DAMAGE_TYPE: u32 = 0xFF;
        const WEAPON_TYPE: u32 = 0xFF00;
        
        const FIRE: u32 = 0x01;
        const COLD: u32 = 0x02;
        const SWORD: u32 = 0x0100;
        //const BOW: u32 = 0x0200;
        
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to add tagged damage stats
        let add_damage_stats_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Add base damage
                stat_accessor.add_modifier(entity, &format!("Damage_Added_{}", u32::MAX), 10.0);
                
                // Add elemental damage
                stat_accessor.add_modifier(entity, &format!("Damage_Added_{}", (u32::MAX & !DAMAGE_TYPE) | FIRE), 5.0);
                stat_accessor.add_modifier(entity, &format!("Damage_Added_{}", (u32::MAX & !DAMAGE_TYPE) | COLD), 3.0);
                
                // Add weapon damage
                stat_accessor.add_modifier(entity, &format!("Damage_Added_{}", (u32::MAX & !WEAPON_TYPE) | SWORD), 2.0);
                
                // Add increased damage multipliers
                stat_accessor.add_modifier(entity, &format!("Damage_Increased_{}", (u32::MAX & !DAMAGE_TYPE) | FIRE), 0.2);
                stat_accessor.add_modifier(entity, &format!("Damage_Increased_{}", (u32::MAX & !WEAPON_TYPE) | SWORD), 0.1);
            }
        });
        let _ = app.world_mut().run_system(add_damage_stats_id);
        
        // Check complex tag-based stat values
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        // Calculate the expected value
        let expected_added = 10.0 + 5.0 + 2.0; // Base + Fire + Sword
        let expected_increased = 1.0 + 0.2 + 0.1; // Base + Fire + Sword
        let expected_damage = expected_added * expected_increased;
        
        let actual_damage = stats.evaluate_by_string(&format!("Damage_{}", FIRE | SWORD));
        
        assert_approx_eq(actual_damage, expected_damage);
    }

    // Test multiple levels of entity dependencies (A -> B -> C)
    #[test]
    fn test_multi_level_entity_dependencies() {
        // Setup app
        let mut app = App::new();

        // Spawn three entities with Stats component
        let entity_c = app.world_mut().spawn(Stats::new()).id();
        let entity_b = app.world_mut().spawn(Stats::new()).id();
        let entity_a = app.world_mut().spawn(Stats::new()).id();

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Set base values for entity C
            stat_accessor.add_modifier(entity_c, "Power_Added", 10.0);
            
            // Entity B depends on C
            stat_accessor.register_dependency(entity_b, "Source", entity_c);
            stat_accessor.add_modifier(entity_b, "Strength_Added", "Source@Power_Added * 0.5");
            
            // Entity A depends on B
            stat_accessor.register_dependency(entity_a, "Parent", entity_b);
            stat_accessor.add_modifier(entity_a, "Damage_Added", "Parent@Strength_Added * 2.0");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the dependency chain
        let [stats_c, stats_b, stats_a] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [entity_c, entity_b, entity_a])
            .unwrap();
        
        let c_power = stats_c.evaluate_by_string("Power_Added");
        let b_strength = stats_b.evaluate_by_string("Strength_Added");
        let a_damage = stats_a.evaluate_by_string("Damage_Added");
        
        assert_eq!(c_power, 10.0);
        assert_eq!(b_strength, 5.0);  // 10.0 * 0.5
        assert_eq!(a_damage, 10.0);   // 5.0 * 2.0
        
        // Now modify entity C and verify changes propagate through the chain
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(entity_c, "Power_Added", 10.0); // Increase by 10
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify updated values
        let [stats_c, stats_b, stats_a] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [entity_c, entity_b, entity_a])
            .unwrap();
        
        let updated_c_power = stats_c.evaluate_by_string("Power_Added");
        let updated_b_strength = stats_b.evaluate_by_string("Strength_Added");
        let updated_a_damage = stats_a.evaluate_by_string("Damage_Added");
        
        assert_eq!(updated_c_power, 20.0);      // 10.0 + 10.0
        assert_eq!(updated_b_strength, 10.0);   // 20.0 * 0.5
        assert_eq!(updated_a_damage, 20.0);     // 10.0 * 2.0
    }

    // Test multiple entity dependencies (entity depends on multiple other entities)
    #[test]
    fn test_multiple_entity_dependencies() {
        // Setup app
        let mut app = App::new();

        // Spawn three entities with Stats component
        let owner_entity = app.world_mut().spawn(Stats::new()).id();
        let weapon_entity = app.world_mut().spawn(Stats::new()).id();
        let minion_entity = app.world_mut().spawn(Stats::new()).id();

        // Setup dependencies and initial values
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Set base values for owner
            stat_accessor.add_modifier(owner_entity, "Intelligence_Added", 20.0);
            stat_accessor.add_modifier(owner_entity, "Strength_Added", 15.0);
            
            // Set base value for weapon
            stat_accessor.add_modifier(weapon_entity, "WeaponDamage_Added", 25.0);
            
            // Minion depends on both owner and weapon
            stat_accessor.register_dependency(minion_entity, "Owner", owner_entity);
            stat_accessor.register_dependency(minion_entity, "Weapon", weapon_entity);
            
            // Minion's damage depends on owner's intelligence and weapon's damage
            stat_accessor.add_modifier(minion_entity, "SpellDamage_Added", "Owner@Intelligence_Added * 0.5");
            stat_accessor.add_modifier(minion_entity, "PhysicalDamage_Added", "Weapon@WeaponDamage_Added * 0.8");
            
            // Minion's total damage depends on both types
            stat_accessor.add_modifier(minion_entity, "TotalDamage_Added", "SpellDamage_Added + PhysicalDamage_Added");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the dependencies
        let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        
        let spell_damage = stats_minion.evaluate_by_string("SpellDamage_Added");
        let physical_damage = stats_minion.evaluate_by_string("PhysicalDamage_Added");
        let total_damage = stats_minion.evaluate_by_string("TotalDamage_Added");
        
        assert_eq!(spell_damage, 10.0);     // Owner Intelligence 20.0 * 0.5
        assert_eq!(physical_damage, 20.0);  // Weapon Damage 25.0 * 0.8
        assert_eq!(total_damage, 30.0);     // 10.0 + 20.0
        
        // Now modify both dependencies and verify changes
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(owner_entity, "Intelligence_Added", 10.0);
            stat_accessor.add_modifier(weapon_entity, "WeaponDamage_Added", 15.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify updated values
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        
        let updated_spell = updated_stats.evaluate_by_string("SpellDamage_Added");
        let updated_physical = updated_stats.evaluate_by_string("PhysicalDamage_Added");
        let updated_total = updated_stats.evaluate_by_string("TotalDamage_Added");
        
        assert_eq!(updated_spell, 15.0);      // (20.0 + 10.0) * 0.5
        assert_eq!(updated_physical, 32.0);   // (25.0 + 15.0) * 0.8
        assert_eq!(updated_total, 47.0);      // 15.0 + 32.0
    }

    // Test complex expressions mixing entity dependencies and local dependencies
    #[test]
    fn test_mixed_entity_local_dependencies() {
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component
        let owner_entity = app.world_mut().spawn(Stats::new()).id();
        let minion_entity = app.world_mut().spawn(Stats::new()).id();

        // Setup dependencies and initial values
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Set base values for owner
            stat_accessor.add_modifier(owner_entity, "Power_Added", 20.0);
            
            // Set local multiplier for minion
            stat_accessor.add_modifier(minion_entity, "Multiplier_Added", 2.5);
            
            // Register dependencies
            stat_accessor.register_dependency(minion_entity, "Owner", owner_entity);
            
            // Create a mixed dependency expression
            stat_accessor.add_modifier(minion_entity, "Damage_Added", "Owner@Power_Added * Multiplier_Added");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the mixed dependency calculation
        let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let damage = stats_minion.evaluate_by_string("Damage_Added");
        
        assert_eq!(damage, 50.0);  // Owner Power 20.0 * Local Multiplier 2.5
        
        // Test updating the local multiplier
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(minion_entity, "Multiplier_Added", 0.5);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify only the multiplier changed, not the owner stat
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let updated_damage = updated_stats.evaluate_by_string("Damage_Added");
        
        assert_eq!(updated_damage, 60.0);  // Owner Power 20.0 * Local Multiplier (2.5 + 0.5 = 3.0)
        
        // Test updating the owner stat
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(owner_entity, "Power_Added", 10.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify the owner stat change propagated correctly
        let final_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let final_damage = final_stats.evaluate_by_string("Damage_Added");
        
        assert_eq!(final_damage, 90.0);  // Owner Power (20.0 + 10.0 = 30.0) * Local Multiplier 3.0
    }

    // Test entity dependency removal
    #[test]
    fn test_entity_dependency_removal() {
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component
        let owner_entity = app.world_mut().spawn(Stats::new()).id();
        let minion_entity = app.world_mut().spawn(Stats::new()).id();

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Set base values for owner
            stat_accessor.add_modifier(owner_entity, "Power_Added", 20.0);
            
            // Register dependencies
            stat_accessor.register_dependency(minion_entity, "Owner", owner_entity);
            
            // Create a dependency
            stat_accessor.add_modifier(minion_entity, "Damage_Added", "Owner@Power_Added * 1.5");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify initial dependency
        let stats_minion = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let initial_damage = stats_minion.evaluate_by_string("Damage_Added");
        
        assert_eq!(initial_damage, 30.0);  // Owner Power 20.0 * 1.5
        
        // Remove the entity-dependent modifier
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Remove the modifier that depends on the owner
            stat_accessor.remove_modifier(minion_entity, "Damage_Added", "Owner@Power_Added * 1.5");
            
            // Add a fixed value instead
            stat_accessor.add_modifier(minion_entity, "Damage_Added", 15.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify dependency is removed and fixed value works
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let updated_damage = updated_stats.evaluate_by_string("Damage_Added");
        
        assert_eq!(updated_damage, 15.0);  // Fixed value, no longer depends on owner
        
        // Modify the owner entity and verify it no longer affects the minion
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(owner_entity, "Power_Added", 30.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify the minion's damage didn't change
        let final_stats = app.world_mut().query::<&Stats>().get(app.world(), minion_entity).unwrap();
        let final_damage = final_stats.evaluate_by_string("Damage_Added");
        
        assert_eq!(final_damage, 15.0);  // Still fixed value, owner change had no effect
    }

    // Test complex tag-based entity dependencies
    #[test]
    fn test_complex_tag_based_entity_dependencies() {
        // Define bit flags for damage types
        const FIRE: u32 = 0x01;
        const COLD: u32 = 0x02;
        const LIGHTNING: u32 = 0x04;
        
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component
        let master_entity = app.world_mut().spawn(Stats::new()).id();
        let servant_entity = app.world_mut().spawn(Stats::new()).id();

        println!("Master entity: {}", master_entity);
        println!("Servant entity: {}", servant_entity);

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Set base elemental damage values for master
            stat_accessor.add_modifier(master_entity, &format!("Damage_Added_{}", FIRE), 20.0);
            stat_accessor.add_modifier(master_entity, &format!("Damage_Added_{}", COLD), 15.0);
            stat_accessor.add_modifier(master_entity, &format!("Damage_Added_{}", LIGHTNING), 25.0);
            
            // Set elemental multipliers for master
            stat_accessor.add_modifier(master_entity, &format!("Damage_Increased_{}", FIRE), 0.5);
            stat_accessor.add_modifier(master_entity, &format!("Damage_Increased_{}", COLD), 0.3);
            stat_accessor.add_modifier(master_entity, &format!("Damage_Increased_{}", LIGHTNING), 0.4);
            
            // Register dependency
            stat_accessor.register_dependency(servant_entity, "Master", master_entity);
            
            // Create complex tag-based dependencies on the servant
            stat_accessor.add_modifier(servant_entity, &format!("Damage_Added_{}", FIRE), format!("Master@Damage_Added_{} * 0.6", FIRE));
            stat_accessor.add_modifier(servant_entity, &format!("Damage_Added_{}", COLD), format!("Master@Damage_Added_{} * 0.7", COLD));
            stat_accessor.add_modifier(servant_entity, &format!("Damage_Added_{}", LIGHTNING), format!("Master@Damage_Added_{} * 0.5", LIGHTNING));
            
            // Copy master's multipliers (simplified syntax)
            stat_accessor.add_modifier(servant_entity, &format!("Damage_Increased_{}", FIRE), format!("Master@Damage_Increased_{}", FIRE));
            stat_accessor.add_modifier(servant_entity, &format!("Damage_Increased_{}", COLD), format!("Master@Damage_Increased_{}", COLD));
            stat_accessor.add_modifier(servant_entity, &format!("Damage_Increased_{}", LIGHTNING), format!("Master@Damage_Increased_{}", LIGHTNING));
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify the complex tag-based dependencies
        let stats_servant = app.world_mut().query::<&Stats>().get(app.world(), servant_entity).unwrap();
        
        // Calculate expected values
        // For each damage type: servant's damage = master's base * servant scaling * (1 + master's increased)
        let fire_expected = 20.0 * 0.6 * (1.0 + 0.5);
        let cold_expected = 15.0 * 0.7 * (1.0 + 0.3);
        let lightning_expected = 25.0 * 0.5 * (1.0 + 0.4);
        
        let fire_actual = stats_servant.evaluate_by_string(&format!("Damage_{}", FIRE));
        let cold_actual = stats_servant.evaluate_by_string(&format!("Damage_{}", COLD));
        let lightning_actual = stats_servant.evaluate_by_string(&format!("Damage_{}", LIGHTNING));
        
        assert_approx_eq(fire_actual, fire_expected);
        assert_approx_eq(cold_actual, cold_expected);
        assert_approx_eq(lightning_actual, lightning_expected);
        
        // Now increase the master's fire damage and verify the change propagates
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            stat_accessor.add_modifier(master_entity, &format!("Damage_Added_{}", FIRE), 10.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify updated values
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), servant_entity).unwrap();
        
        // Calculate new expected values - only fire changes
        let updated_fire_expected = 30.0 * 0.6 * (1.0 + 0.5);
        
        let updated_fire = updated_stats.evaluate_by_string(&format!("Damage_{}", FIRE));
        assert_approx_eq(updated_fire, updated_fire_expected);
    }

    // Test concurrent updates to multiple entity stats
    #[test]
    fn test_concurrent_entity_updates() {
        // Setup app
        let mut app = App::new();

        // Spawn entities with Stats component - a buff source and multiple recipients
        let buff_source = app.world_mut().spawn(Stats::new()).id();
        let recipient_a = app.world_mut().spawn(Stats::new()).id();
        let recipient_b = app.world_mut().spawn(Stats::new()).id();
        let recipient_c = app.world_mut().spawn(Stats::new()).id();

        // Setup initial values and dependencies
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Set base buff value
            stat_accessor.add_modifier(buff_source, "AuraPower_Added", 10.0);
            
            // Register dependencies for all recipients
            stat_accessor.register_dependency(recipient_a, "Aura", buff_source);
            stat_accessor.register_dependency(recipient_b, "Aura", buff_source);
            stat_accessor.register_dependency(recipient_c, "Aura", buff_source);
            
            // Each recipient gets the aura buff with a different multiplier
            stat_accessor.add_modifier(recipient_a, "BuffedPower_Added", "Aura@AuraPower_Added * 1.2");
            stat_accessor.add_modifier(recipient_b, "BuffedPower_Added", "Aura@AuraPower_Added * 0.8");
            stat_accessor.add_modifier(recipient_c, "BuffedPower_Added", "Aura@AuraPower_Added * 1.5");
        });
        let _ = app.world_mut().run_system(system_id);

        // Verify initial buffed values
        let [stats_a, stats_b, stats_c] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [recipient_a, recipient_b, recipient_c])
            .unwrap();
        
        let power_a = stats_a.evaluate_by_string("BuffedPower_Added");
        let power_b = stats_b.evaluate_by_string("BuffedPower_Added");
        let power_c = stats_c.evaluate_by_string("BuffedPower_Added");
        
        assert_eq!(power_a, 12.0);  // 10.0 * 1.2
        assert_eq!(power_b, 8.0);   // 10.0 * 0.8
        assert_eq!(power_c, 15.0);  // 10.0 * 1.5
        
        // Now change the aura power value to simulate a buff strengthening
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessor| {
            // Strengthen the aura
            stat_accessor.add_modifier(buff_source, "AuraPower_Added", 5.0);
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify all buffs updated correctly
        let [updated_a, updated_b, updated_c] = app.world_mut().query::<&Stats>()
            .get_many(app.world(), [recipient_a, recipient_b, recipient_c])
            .unwrap();
        
        let updated_power_a = updated_a.evaluate_by_string("BuffedPower_Added");
        let updated_power_b = updated_b.evaluate_by_string("BuffedPower_Added");
        let updated_power_c = updated_c.evaluate_by_string("BuffedPower_Added");
        
        assert_eq!(updated_power_a, 18.0);  // 15.0 * 1.2
        assert_eq!(updated_power_b, 12.0);  // 15.0 * 0.8
        assert_eq!(updated_power_c, 22.5);  // 15.0 * 1.5
    }
}