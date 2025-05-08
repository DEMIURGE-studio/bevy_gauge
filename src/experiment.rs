use std::{cell::SyncUnsafeCell, fmt, sync::RwLock};

use bevy::{ecs::system::SystemParam, prelude::*, utils::{HashMap, HashSet}};
use evalexpr::{Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};
use regex::Regex;

/// Error type for the stat system
#[derive(Debug, Clone, PartialEq)]
pub enum StatError {
    /// Failed to parse a stat path
    InvalidStatPath { path: String, details: String },
    
    /// Error when evaluating an expression
    ExpressionError { expression: String, details: String },
    
    /// Entity not found
    EntityNotFound { entity: Entity },
    
    /// Stat not found
    StatNotFound { path: String },
    
    /// Invalid tag format in path
    InvalidTagFormat { tag: String, path: String },
    
    /// Dependency cycle detected
    DependencyCycle { path: String },
    
    /// Missing source entity reference
    MissingSource { source_name: String, path: String },
    
    /// Internal error
    Internal { details: String },
}

impl fmt::Display for StatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatError::InvalidStatPath { path, details } => {
                write!(f, "Invalid stat path '{}': {}", path, details)
            }
            StatError::ExpressionError { expression, details } => {
                write!(f, "Failed to evaluate expression '{}': {}", expression, details)
            }
            StatError::EntityNotFound { entity } => {
                write!(f, "Entity {:?} not found", entity)
            }
            StatError::StatNotFound { path } => {
                write!(f, "Stat '{}' not found", path)
            }
            StatError::InvalidTagFormat { tag, path } => {
                write!(f, "Invalid tag format '{}' in path '{}'", tag, path)
            }
            StatError::DependencyCycle { path } => {
                write!(f, "Dependency cycle detected for stat '{}'", path)
            }
            StatError::MissingSource { source_name, path } => {
                write!(f, "Missing source '{}' referenced by '{}'", source_name, path)
            }
            StatError::Internal { details } => {
                write!(f, "Internal error: {}", details)
            }
        }
    }
}

impl std::error::Error for StatError {}

// Type alias for Result with StatError
pub type StatResult<T> = Result<T, StatError>;

// Internal mutability used for cached_values and dependents_map because those should
// be able to be safely updated due to their values being derived from internal values.
// We actively update the values whenever a modifier changes, ensuring the cache is
// always up to date. 
#[derive(Component, Default)]
pub struct Stats {
    definitions: HashMap<String, StatType>,
    cached_values: SyncContext,
    dependents_map: DependencyMap,
    depends_on_map: DependencyMap,
    sources: HashMap<String, Entity>,
}

impl Stats {
    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType, config: &StatConfig) {
        
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType, config: &StatConfig) {
        
    }

    fn set(&mut self, path: &StatPath, value: f32, config: &StatConfig) {
        
    }

    pub fn get(&self, path: &str, config: &StatConfig) -> f32 {
        if self.cached_values.get(path).is_err() {
            self.cached_values.set(path, self.evaluate(&StatPath::parse(path).unwrap(), config));
        }
        self.cached_values.get(path).unwrap_or(0.0)
    }

    pub fn get_by_path(&self, path: &StatPath, config: &StatConfig) -> f32 {
        self.get(&path.path, config)
    }

    fn evaluate(&self, path: &StatPath, config: &StatConfig) -> f32 {
        let Some(stat) = self.definitions.get(&path.path) else {
            return 0.0;
        };

        stat.evaluate(path, self, config)
    }

    fn get_dependents(&self) -> &HashMap<String, HashMap<DependentType, u32>> {
        self.dependents_map.get_dependents()
    }

    // Return all the dependencies in a given expression.
    // "Life.Added" += "Strength / 5" will return DependentType::Local("Strength")
    // "Damage.Added.PHYSICAL" += "Strength@EquippedTo / 2" will return
    // DependentType::Entity(equipped_to_entity, "Strength")
    fn get_dependencies(&self, expression: &Expression) -> Vec<DependentType> {
        let dependencies: Vec<&str> = expression.compiled.iter_identifiers().collect();
        let mut d = Vec::new();
        for dependency in dependencies {
            let path = StatPath::parse(dependency).unwrap();
            if let Some(owner) = &path.owner {
                if let Some(source_entity) = self.sources.get(owner) {
                    d.push(DependentType::Entity(*source_entity, path.local_path));
                }
            } else {
                d.push(DependentType::Local(path.path));
            }
        }
        d
    }
    
    fn add_dependency(&self, part_path: &str, parts: &str) {
        todo!()
    }
}

// Holds the memoized values for stats in the form of a HashMapContext, which can be
// used to evaluate stat values. When a stats value changes, it's cached value must be
// actively updated anywhere it exists.
#[derive(Default)]
struct SyncContext(SyncUnsafeCell<HashMapContext>);

impl SyncContext {
    fn new() -> Self {
        Self(SyncUnsafeCell::new(HashMapContext::new()))
    }

    fn get(&self, path: &str) -> StatResult<f32> {
        unsafe {
            // Use .get_value_ref() if available and appropriate, or clone if necessary
            // to avoid holding a reference across potential mutations if HashMapContext isn't Sync.
            // However, evalexpr::Value is copyable for simple types like Float.
            if let Some(stat_value) = (*self.0.get()).get_value(path.into()) {
                stat_value.as_float().map(|f| f as f32).map_err(|_eval_err| {
                    StatError::ExpressionError { // Or a new error type like CacheCorruption
                        expression: path.to_string(), // Path acts as the "expression" for cache lookup
                        details: format!("Cached stat '{}' is not a valid float: {:?}", path, stat_value),
                    }
                })
            } else {
                Err(StatError::StatNotFound {
                    path: path.to_string(),
                })
            }
        }
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

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum DependentType {
    Local(String),
    Entity(Entity, String),
}

// Tracks the dependency relationships. A stat name is matched to its dependent types.
// If it's a "Local" then a local stat will be re-evaluated. If it's on another entity,
// that entities cached value for the remote stat will be updated and any stats dependent
// on that remote stat will also be updated. Updating a stat should trigger a sort of 
// cross-entity recursive update. The SyncDependents is how we know what to update.

// A stat will be a dependent of another stat if one of its modifiers contains that stat.
// For instance, one of the modifiers for like might be an expression like so:
// "Life.Added" += "Strength / 5". This would make Life.Added dependent on Strength.
// Life would automatically depend on Life.Added as per the "total" expression in the 
// Life Modifiable stat entry.

// Cross entity dependents might look something like this:
// "Damage.Added.PHYSICAL" += "Strength@EquippedTo / 2". This indicates that a weapons
// damage is dependent on the entity it's equipped to's strength. This would also require
// an entry in "sources" for "EquippedTo" to map to a specific entity.
#[derive(Default)]
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

#[derive(Resource, Default)]
pub struct StatConfig {
    total_expressions: HashMap<String, String>,
    default_total_expression: String,
    default_base_value: HashMap<String, f32>,
    stat_type: HashMap<String, String>,
}

impl StatConfig {
    fn get_total_expression(&self, path: &StatPath) -> String {
        self.total_expressions.get(&path.path).unwrap_or(&self.default_total_expression).to_string()
    }
    
    fn get_default_value(&self, path: &StatPath) -> f32 {
        self.default_base_value.get(&path.path).unwrap_or(&0.0).clone()
    }
    
    fn new_stat(&self, path: &StatPath) -> StatType {
        let default = "".to_string().clone();
        let stat_type = self.stat_type.get(&path.path).unwrap_or(&default);
        match stat_type.as_str() {
            "Flat" => {
                StatType::Flat(Flat::new(path, self))
            },
            "Modifiable" => {
                StatType::Modifiable(Modifiable::new(path, self))
            },
            "Complex" => {
                StatType::Complex(Complex::new(path, self))
            },
            "Tagged" => {
                StatType::Tagged(Tagged::new(path, self))
            },
            _ => StatType::Flat(Flat::new(path, self))
        }
    }
}

// Stats cannot handle cross-entity stat updates, so all stat value changes must be done
// through the StatAccessor so it can keep everything up to date. 
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    query: Query<'w, 's, &'static mut Stats>,
    config: Res<'w, StatConfig>,
}

impl StatAccessor<'_, '_> {
    pub fn add_modifier<M: Into<ModifierType>>(&mut self, target: Entity, path: &str, modifier: M) {

    }
    
    pub fn remove_modifier<M: Into<ModifierType>>(&mut self, target: Entity, path: &str, modifier: M) {

    }

    // Handle all value changing, cache updating, and dependency registration.
    pub fn add_modifier_value(&mut self, target: Entity, path: &StatPath, modifier: ModifierType) {
        
    }
    
    // add_modifier_value but in reverse.
    pub fn remove_modifier_value(&mut self, target: Entity, path: &StatPath, modifier: &ModifierType) {
        
    }
    
    pub fn set(&mut self, target: Entity, path: &StatPath, value: f32) {
        
    }

    // Registers a source. For instance target: item_entity, source_type: "EquippedTo", source: equipped_to_entity.
    // When a new source is registered the old cached values for that source must be updated. For instance, if 
    // we change who the sword is equipped to, we must update the "Strength@EquippedTo" cached value for the sword
    // and all dependent stat values (via the update function)
    pub fn register_source(&mut self, target: Entity, source_type: String, source: Entity) {
        
    }

    // Helper method to update dependencies when a source changes
    fn update_source_dependencies(&mut self, target: Entity, old_source: Entity, new_source: Entity) {
        
    }

    // go through all dependencies and add them in reverse. That is, DependentType::Entity(equipped_to_entity, "Strength")
    // will get the equipped_to_entity's Stats, and add DependencyType::Entity(target, "Life.Added") as the dependency of
    // the local stat "Strength".
    fn add_dependents(&mut self, target: Entity, modified_path: &StatPath, dependencies: Vec<DependentType>) {
        
    }
    
    fn remove_dependents(&mut self, target: Entity, modified_path: &StatPath, dependencies: Vec<DependentType>) {
        
    }

    // recursively go over all dependent stats, re-evaluate their values and update their caches.
    fn update(&mut self, target: Entity, path: &StatPath) {
        
    }

    fn update_recursive(&mut self, target: Entity, path_str: String, visited: &mut HashSet<(Entity, String)>) {
        
    }

    // handles safe tear-down of a destroyed stat entity. Removes all dependencies from relevant entities
    pub fn remove_stat_entity(&mut self, target_entity: Entity) {
        
    }
}

// Modifiers can be flat (literal) values or expressions.
#[derive(Clone)]
pub enum ModifierType {
    Literal(f32),
    Expression(Expression),
}

// Expressions store their definition and use evalexpr to calculate stat values.
#[derive(Clone)]
pub struct Expression {
    definition: String,
    compiled: Node<DefaultNumericTypes>,
}

impl Expression {
    fn new(expression: &str) -> StatResult<Self> {
        let compiled = evalexpr::build_operator_tree(expression)
            .map_err(|err| StatError::ExpressionError {
                expression: expression.to_string(),
                details: err.to_string(),
            })?;
            
        Ok(Self {
            definition: expression.to_string(),
            compiled,
        })
    }
    
    // Safe version that panics on error, for backwards compatibility
    fn new_unchecked(expression: &str) -> Self {
        Self::new(expression).unwrap_or_else(|err| {
            panic!("Failed to create expression: {}", err)
        })
    }

    // Updated evaluate to handle errors
    fn evaluate(&self, context: &HashMapContext) -> f32 {
        self.compiled
            .eval_with_context(context)
            .unwrap_or(Value::Float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)] // Added derives
pub struct StatPath {
    owner: Option<String>, // Name of the source (e.g., "EquippedTo", "Parent")
    path: String,          // The full original string (e.g., "Strength@EquippedTo")
    local_path: String,    // Path relative to the owner (e.g., "Strength")
    parts: Vec<String>,    // Local path split by '.' (e.g., ["Damage", "Added", "12"]), where 12 is a u32 representing arbitrary tags.
}

impl StatPath {
    // Parse a string into a StatPath
    fn parse<S: AsRef<str>>(path_str_ref: S) -> StatResult<Self> {
        let path_str = path_str_ref.as_ref();
        let mut owner = None;
        let local_path_str;

        if let Some((local, owner_str)) = path_str.rsplit_once('@') {
            owner = Some(owner_str.to_string());
            local_path_str = local;
        } else {
            local_path_str = path_str;
        }

        let parts: Vec<String> = local_path_str.split('.').map(String::from).collect();
        if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
            return Err(StatError::InvalidStatPath {
                path: path_str.to_string(),
                details: "Path cannot be empty or contain empty parts".to_string(),
            });
        }

        Ok(Self {
            owner,
            path: path_str.to_string(),
            local_path: local_path_str.to_string(),
            parts,
        })
    }
}

// Update the From implementation to propagate errors
impl<T: Into<String>> From<T> for StatPath where T: AsRef<str> {
    fn from(value: T) -> Self {
        Self::parse(value.as_ref()).unwrap_or_else(|err| {
            panic!("Failed to parse stat path: {}", err)
        })
    }
}

// A common interface for adding modifiers, removing modifiers, etc. Should gracefully handle
// the unique problems that face specific stats. Not entirely sure what "on_update" is doing, 
// but there needs to be a built-in way for something like a Compound stat to add its part variants
// to the cache and its total variant as a dependent of its part variants (i.e., when "Life" is added
// we add "Life" as a dependent of "Life.Added", "Life.Increased", etc., AND we add "Life.Added" and
// the other variants to the cache.)
trait StatLike {
    fn new(path: &StatPath, stats: &mut Stats, config: &StatConfig) -> Self;
    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType, config: &StatConfig);
    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType, config: &StatConfig);
    fn set(&mut self, path: &StatPath, value: f32, config: &StatConfig);
    fn evaluate(&self, path: &StatPath, stats: &Stats, config: &StatConfig) -> f32;
}

// A catch-all for stat types.
enum StatType {
    Flat(Flat),
    Modifiable(Modifiable),
    Complex(Complex),
    Tagged(Tagged),
}

impl StatLike for StatType {
    fn new(path: &StatPath, stats: &mut Stats, config: &StatConfig) -> Self {
        config.new_stat(path)
    }

    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, config: &StatConfig) {
        match self {
            StatType::Flat(flat) => flat.add_modifier(path, value, config),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(path, value, config),
            StatType::Complex(complex) => complex.add_modifier(path, value, config),
            StatType::Tagged(tagged) => tagged.add_modifier(path, value, config),
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ModifierType, config: &StatConfig) {
        match self {
            StatType::Flat(flat) => flat.remove_modifier(path, value, config),
            StatType::Modifiable(modifiable) => modifiable.remove_modifier(path, value, config),
            StatType::Complex(complex) => complex.remove_modifier(path, value, config),
            StatType::Tagged(tagged) => tagged.remove_modifier(path, value, config),
        }
    }

    fn set(&mut self, path: &StatPath, value: f32, config: &StatConfig) {
        match self {
            StatType::Flat(flat) => flat.set(path, value, config),
            StatType::Modifiable(_) => {},
            StatType::Complex(_) => {},
            StatType::Tagged(_) => {},
        }
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats, config: &StatConfig) -> f32 {
        match self {
            StatType::Flat(flat) => flat.evaluate(path, stats, config),
            StatType::Modifiable(modifiable) => modifiable.evaluate(path, stats, config),
            StatType::Complex(complex) => complex.evaluate(path, stats, config),
            StatType::Tagged(tagged) => tagged.evaluate(path, stats, config),
        }
    }
}

// Only has a base value. Cannot have expression modifiers added to it.
struct Flat {
    base: f32,
}

impl StatLike for Flat {
    fn new(path: &StatPath, stats: &mut Stats, config: &StatConfig) -> Self {
        Self { base: config.get_default_value(path) }
    }

    fn add_modifier(&mut self, _path: &StatPath, value: ModifierType, config: &StatConfig) {
        // For Flat stats, we only handle Literal values
        if let ModifierType::Literal(val) = value {
            self.base += val;
        }
        // Silently ignore Expression modifiers as they're not applicable
    }

    fn remove_modifier(&mut self, _path: &StatPath, value: &ModifierType, config: &StatConfig) {
        // For Flat stats, we only handle Literal values
        if let ModifierType::Literal(val) = value {
            self.base -= val;
        }
        // Silently ignore Expression modifiers as they're not applicable
    }

    fn set(&mut self, _path: &StatPath, value: f32, config: &StatConfig) {
        // Direct setting replaces the base value
        self.base = value;
    }

    fn evaluate(&self, _path: &StatPath, _stats: &Stats, config: &StatConfig) -> f32 {
        // Flat stats just return their base value
        self.base
    }
}

// A base and a vec of expressions.
struct Modifiable {
    base: f32,
    expressions: Vec<Expression>,
}

impl StatLike for Modifiable {
    fn new(path: &StatPath, stats: &mut Stats, config: &StatConfig) -> Self {
        todo!()
    }

    fn add_modifier(&mut self, _path: &StatPath, value: ModifierType, config: &StatConfig) {
        match value {
            ModifierType::Literal(val) => {
                // For literal values, we just add to the base
                self.base += val;
            },
            ModifierType::Expression(expr) => {
                // For expressions, we add them to the list
                self.expressions.push(expr);
            }
        }
    }

    fn remove_modifier(&mut self, _path: &StatPath, value: &ModifierType, config: &StatConfig) {
        match value {
            ModifierType::Literal(val) => {
                // For literal values, we subtract from the base
                self.base -= val;
            },
            ModifierType::Expression(expr) => {
                // For expressions, we remove the matching one
                if let Some(pos) = self.expressions.iter().position(|e| e.definition == expr.definition) {
                    self.expressions.remove(pos);
                }
            }
        }
    }

    fn set(&mut self, _path: &StatPath, _value: f32, _config: &StatConfig) { return }

    fn evaluate(&self, _path: &StatPath, stats: &Stats, config: &StatConfig) -> f32 {
        // Start with the base value
        let mut result = self.base;
        
        // Apply all expression modifiers
        for expression in &self.expressions {
            // For evalexpr, we need to provide a context with all stat values
            let context = stats.cached_values.context();
            
            // Evaluate the expression and add it to the result
            result += expression.evaluate(context);
        }
        
        result
    }
}

// Maps strings to expression with a total. Total might be "Added * Increased * More"
// and there will be an "Added", "Increased", and "More" entry in the "parts"
// Need some way (perhaps through StatConfig) to give default values for "Added",
// "Increased", "More", etc. For instance, multipliers like Increased or More might
// want a default value of 1 where Added might be 0.

// A complex stat like Life should have a "Life" entry is cached stats, as well as a 
// "Life.Added", "Life.Increased", etc. Basically all the subtypes should be in there,
// and "Life" needs to be a dependent of "Life.Added." This is the kind of special
// stat-type specific dependency registration that the StatLike trait should help handle.
struct Complex {
    total: Expression,
    parts: HashMap<String, Modifiable>,
}

impl StatLike for Complex {
    fn new(path: &StatPath, stats: &mut Stats, config: &StatConfig) -> Self {
        // get total equation from StatConfig
        // compile
        // iterate through identifiers
        // for each identifier, add a part with the correct default from the StatConfig
        let total = config.get_total_expression(path);

        let expression = Expression::new_unchecked(&total);

        let mut parts = HashMap::new();
        for part_id in expression.compiled.iter_identifiers() {
            // TODO unwrap
            parts.insert(part_id.to_string(), Modifiable::new(&StatPath::parse(part_id).unwrap(), stats, config));

            let part_path = format!("{}.{}", path.parts[0], part_id);

            stats.add_dependency(&part_path, &path.parts[0])
        }

        Self {
            total: expression,
            parts,
        }
    }

    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, config: &StatConfig) {
        // Get the appropriate part based on the path
            let part_name = &path.parts[1]; // Second part of the path is the modifier type (e.g., "Added")
            let part = self.parts.get_mut(part_name).unwrap();
            
            // Add the modifier to the appropriate part
            part.add_modifier(path, value);
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ModifierType, config: &StatConfig) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get_mut(part_name) {
                part.remove_modifier(path, value);
            }
        }
    }

    fn set(&mut self, path: &StatPath, value: f32, config: &StatConfig) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get_mut(part_name) {
                part.set(path, value);
            }
        }
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats, config: &StatConfig) -> f32 {
        // If path specifies a part, evaluate just that part
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get(part_name) {
                return part.evaluate(path, stats, config);
            }
            
            return 0.0;
        }
        
        // Otherwise, evaluate the total expression
        // First, ensure all parts are cached
        for (part_name, part) in &self.parts {
            let part_path = format!("{}.{}", path.parts[0], part_name);
            let part_value = part.evaluate(&StatPath::parse(&part_path).unwrap(), stats, config);
            stats.cached_values.set(&part_path, part_value);
        }
        
        // Then evaluate the total expression
        let context = stats.cached_values.context();
        self.total.evaluate(context)
    }
}

// Similar to Complex but allows & operation queries to match on different types.
// This is most useful for Damage stats, which might look something like "Increased
// fire damage with axes." Tags are a u32 so in actuality that could look like
// Damage.Increased.14 for example.

// Tagged stats present a special problem because there's so many variants you can't
// just cache all of them. For instance, when "Life" is added, we cache "Life", "Life.Added",
// etc. For Damage we don't want to cache every damage variant because there could be millions.
// Instead we should only cache a value after it has been queried. I'm not sure how to handle 
// this exacly. Maybe we use internal mutability to track previous queries, and then when 
// a value is updated we iterate over all previous queries and update their values?
// A version of Modifiable for tag values
struct TaggedEntry {
    base: f32,
    expressions: HashMap<u32, Vec<Expression>>,
}

impl TaggedEntry {
    fn new(modifier_type: &str) -> Self {
        let config = StatConfig::global().lock().unwrap();
        let default_base = config.get_default_base(modifier_type);
        
        Self {
            base: default_base,
            expressions: HashMap::new(),
        }
    }
    
    fn add_modifier(&mut self, tag: u32, value: ModifierType) {
        match value {
            ModifierType::Literal(val) => {
                // For literal values, we just add to the base
                self.base += val;
            },
            ModifierType::Expression(expr) => {
                // For expressions, we add them to the list for the specific tag
                self.expressions.entry(tag)
                    .or_insert_with(Vec::new)
                    .push(expr);
            }
        }
    }
    
    fn remove_modifier(&mut self, tag: u32, value: ModifierType) {
        match value {
            ModifierType::Literal(val) => {
                // For literal values, we subtract from the base
                self.base -= val;
            },
            ModifierType::Expression(expr) => {
                // For expressions, remove the matching one for the specific tag
                if let Some(exprs) = self.expressions.get_mut(&tag) {
                    if let Some(pos) = exprs.iter().position(|e| e.definition == expr.definition) {
                        exprs.remove(pos);
                    }
                    
                    // If there are no more expressions, remove the tag entry
                    if exprs.is_empty() {
                        self.expressions.remove(&tag);
                    }
                }
            }
        }
    }
    
    fn evaluate_for_tag(&self, search_tag: u32, path: &StatPath, stats: &Stats) -> f32 {
        // Start with the base value
        let mut result = self.base;
        
        // Sum all expressions from all matching tags
        for (&tag, exprs) in &self.expressions {
            if has_matching_tag(tag, search_tag) {
                for expr in exprs {
                    let context = stats.cached_values.context();
                    result += expr.evaluate(context);
                }
            }
        }
        
        result
    }
}

// Helper function to check if tags match (e.g., has_all in your example)
fn has_matching_tag(mod_tag: u32, search_tag: u32) -> bool {
    // Implement your tag matching logic here
    // For example, bit flags: (mod_tag & search_tag) == search_tag
    (mod_tag & search_tag) == search_tag
}

// Similar to Complex but allows & operation queries to match on different types.

// When modifiers are stored in a Tagged stat, they are stored permissively. That is,
// "Damage.Added.ANY" will have a tag value of u32::ALL. Damage.Added.AXES will apply
// to any physical or elemental damage, but not weapon types besides axes. We may also
// store categories, such as "Damage.Added.MELEE" which includes all melee weapons.
// When storing modifiers, we must specify that they are "Added" or "Increased," or
// whichever modifier applies.

// In contrast when we query for a tag, we query specifically. That means that we must
// specify an item in every category to expect to get a meaningful result. For instance,
// a query might look like "Damage.Added.AXE|FIRE," which represents "Added fire damage
// with axes." Queries can also look for total values. You would do that by querying
// "Damage.FIRE|AXE," which would total up the fire damage with axes.
struct Tagged {
    total: Expression,
    parts: HashMap<String, TaggedEntry>,
    // Track which tag/part combinations have been queried
    cached_queries: RwLock<HashMap<u32, HashSet<String>>>,
}

impl Tagged {
    // Record that a particular tag/part combination has been queried
    fn record_query(&self, tag: u32, part: Option<&str>) {
        let mut cached_queries = self.cached_queries.write().unwrap();
        
        let entry = cached_queries.entry(tag).or_insert_with(HashSet::new);
        
        match part {
            Some(part_name) => {
                entry.insert(part_name.to_string());
            },
            None => {
                // When no specific part is queried, we're querying the total
                // We might want to mark that all parts are needed for this tag
                for identifier in self.total.compiled.iter_identifiers() {
                    entry.insert(identifier.to_string());
                }
            }
        }
    }
    
    // Get all cached queries that might need updating
    fn get_affected_queries(&self) -> HashMap<u32, HashSet<String>> {
        self.cached_queries.read().unwrap().clone()
    }
}

impl StatLike for Tagged {
    fn new(path: &StatPath, stat: &mut Stats, config: &StatConfig) -> Self {
        let expr_template = config.get_total_expression(path);
            
        Self {
            total: Expression::new_unchecked(&expr_template),
            parts: HashMap::new(),
            cached_queries: RwLock::new(HashMap::new()),
        }
    }

    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, config: &StatConfig) {
        // Expect path format like "Damage.Added.PHYSICAL" where PHYSICAL is a tag (u32)
        if path.parts.len() < 3 {
            // Invalid path format for Tagged
            return;
        }
    
        // Get modifier type (e.g., "Added")
        let modifier_type = &path.parts[1];
    
        // Parse the tag
        let tag_str = &path.parts[2];
        let Ok(tag) = tag_str.parse::<u32>() else {
            // Could log an error here
            return;
        };
    
        // Get or create the entry for this modifier type
        let entry = self.parts.entry(modifier_type.clone())
            .or_insert_with(|| TaggedEntry::new(modifier_type));
    
        // Add the modifier directly to the TaggedEntry
        match value {
            ModifierType::Literal(val) => {
                // For literal values, adjust the base
                entry.base += val;
            },
            ModifierType::Expression(expr) => {
                // For expressions, add to the map for the specific tag
                entry.expressions.entry(tag)
                    .or_insert_with(Vec::new)
                    .push(expr);
            }
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ModifierType, config: &StatConfig) {
        // Similar to add_modifier but for removal
        if path.parts.len() < 3 {
            return;
        }
       
        let modifier_type = &path.parts[1];
        let tag_str = &path.parts[2];
       
        let Ok(tag) = tag_str.parse::<u32>() else {
            return;
        };
       
        // Try to get the entry for this modifier type
        if let Some(entry) = self.parts.get_mut(modifier_type) {
            match value {
                ModifierType::Literal(val) => {
                    // For literal values, subtract from the base
                    entry.base -= val;
                },
                ModifierType::Expression(expr) => {
                    // For expressions, remove from the specific tag
                    if let Some(exprs) = entry.expressions.get_mut(&tag) {
                        if let Some(pos) = exprs.iter().position(|e| e.definition == expr.definition) {
                            exprs.remove(pos);
                        }
                        
                        // If this tag has no more expressions, remove it
                        if exprs.is_empty() {
                            entry.expressions.remove(&tag);
                        }
                    }
                }
            }
        }
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats, config: &StatConfig) -> f32 {
        let full_path = &path.path;
       
        // Record that this path was queried for efficient future updates
        // This only uses interior mutability for the cached_queries, not for dependents_map
        if path.parts.len() == 2 {
            if let Ok(tag) = path.parts[1].parse::<u32>() {
                self.record_query(tag, None);
            }
        } else if path.parts.len() == 3 {
            if let Ok(tag) = path.parts[2].parse::<u32>() {
                self.record_query(tag, Some(&path.parts[1]));
            }
        }
       
        // Evaluate based on the path structure
        if path.parts.len() == 2 {
            // Evaluating a total for a specific tag (e.g., "Damage.5")
            if let Ok(search_tag) = path.parts[1].parse::<u32>() {
                // Create a context for evaluating the total expression
                let mut context = HashMapContext::new();
               
                // Initialize all variables in the expression with default values
                for part_name in self.total.compiled.iter_identifiers() {
                    let config = StatConfig::global().lock().unwrap();
                    let default_value = config.get_default_base(part_name);
                    context.set_value(part_name.to_string(), Value::Float(default_value as f64)).unwrap();
                   
                    // Calculate the value for each part
                    let part_path = format!("{}.{}.{}", path.parts[0], part_name, search_tag);
                    
                    // Get the part value from cache if available, or calculate it
                    let part_value = if stats.get(&part_path) > 0.0 {
                        stats.get(&part_path)
                    } else {
                        let part_stat_path = StatPath::parse(&part_path).unwrap();
                        self.evaluate(&part_stat_path, stats)
                    };
                   
                    context.set_value(part_name.to_string(), Value::Float(part_value as f64)).unwrap();
                }
               
                // Evaluate the total expression with our context
                let total = self.total.compiled
                    .eval_with_context(&context)
                    .unwrap_or(Value::Float(0.0))
                    .as_number()
                    .unwrap_or(0.0) as f32;
               
                return total;
            }
        } else if path.parts.len() == 3 {
            // Evaluating a specific part for a specific tag (e.g., "Damage.Added.5")
            let part_name = &path.parts[1];
           
            if let Ok(search_tag) = path.parts[2].parse::<u32>() {
                // Get the entry for this part
                if let Some(entry) = self.parts.get(part_name) {
                    // Start with the base value
                    let mut result = entry.base;
                    
                    // Add contributions from all matching tags
                    for (&mod_tag, expressions) in &entry.expressions {
                        if has_matching_tag(mod_tag, search_tag) {
                            for expr in expressions {
                                let context = stats.cached_values.context();
                                result += expr.evaluate(context);
                            }
                        }
                    }
                    
                    return result;
                }
               
                // Part doesn't exist, return default value
                let config = StatConfig::global().lock().unwrap();
                return config.get_default_base(part_name);
            }
        }
       
        // Invalid path format for this type
        0.0
    }
}

pub trait StatEffect {
    type Context: StatEffectContext = Entity;
   
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context);

    fn remove(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {}
}

pub trait StatEffectContext {}

impl StatEffectContext for Entity {}

#[derive(Component, Clone, Default, Deref, DerefMut)]
pub struct ModifierSet(HashMap<String, Vec<ModifierType>>);

impl ModifierSet {
    pub fn new(modifiers: HashMap<String, Vec<ModifierType>>) -> Self {
        Self(modifiers)
    }

    pub fn add<V: Into<ModifierType>>(&mut self, stat_path: &str, value: V) {
        self.entry(stat_path.to_string())
            .or_insert_with(Vec::new)
            .push(value.into());
    }
}

impl StatEffect for ModifierSet {
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = context;
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.add_modifier_value(*target_entity, &StatPath::parse(stat).unwrap(), modifier.clone());
            }
        }
    }

    fn remove(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_entity = context;
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.remove_modifier_value(*target_entity, &StatPath::parse(stat).unwrap(), modifier);
            }
        }
    }
}

#[derive(Component, Deref)]
#[require(Stats)]
struct StatsInitializer {
    modifier_set: ModifierSet,
}

fn initialize_stat_entity(
    trigger: Trigger<OnAdd, Stats>,
    query: Query<&StatsInitializer>,
    mut stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    println!("0");
    let entity = trigger.entity();

    let Ok(stats_initializer) = query.get(entity) else {
        return;
    };
    println!("1");

    stats_initializer.apply(&mut stat_accessor, &entity);

    println!("2");
    // commands.entity(entity).remove::<StatsInitializer>();
    // println!("3");
}

pub fn plugin(app: &mut App) {
    app.add_observer(initialize_stat_entity);
}