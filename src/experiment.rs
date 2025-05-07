use std::{cell::SyncUnsafeCell, fmt, sync::{Mutex, OnceLock, RwLock}};

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
    
    // New helper method: Gets a definition, or creates and registers it via StatConfig.
    // This ensures that when a definition is retrieved, it's already been through its `register` logic.
    fn get_or_create_defined_stat(&mut self, path: &StatPath) {
        if !self.definitions.contains_key(&path.path) {
            let config = StatConfig::global().lock().unwrap();
            let new_stat_definition = config.create_and_register_stat_type(path, self);
            self.definitions.insert(path.path.clone(), new_stat_definition);
        }

        // If the path is a part (e.g. "Life.Added"), ensure its parent ("Life") is also defined and registered.
        // This handles cases where a part is modified *before* its parent Complex/Tagged stat was explicitly touched.
        if path.parts.len() > 1 {
            let parent_path_str = path.parts[0].clone();
            if path.path != parent_path_str { // Avoid self-call for base paths like "Life"
                let parent_stat_path = StatPath::parse(&parent_path_str).unwrap();
                if !self.definitions.contains_key(&parent_stat_path.path) {
                    // Recursively call to ensure parent is also processed.
                    // This will use StatConfig::create_and_register_stat_type for the parent.
                    self.get_or_create_defined_stat(&parent_stat_path);
                }
            }
        }
    }

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType) {
        self.get_or_create_defined_stat(path); // Ensures definition for path (and its parent) exists and is registered.

        let stat_mut = self.definitions.get_mut(&path.path)
            .unwrap_or_else(|| panic!("Stat '{}' should be defined by get_or_create_defined_stat", path.path));
        stat_mut.add_modifier(path, modifier);
        
        self.update_cached_value(&path.path); 
        // StatAccessor calls a broader update using dependency maps after this.
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) {
        self.get_or_create_defined_stat(path); 

        if let Some(stat) = self.definitions.get_mut(&path.path) {
            stat.remove_modifier(path, modifier);
        }
        // It's important to update cache even if stat was not found or modifier not present,
        // though get_or_create should ensure stat exists.
        self.update_cached_value(&path.path);
    }

    fn set(&mut self, path: &StatPath, value: f32) {
        self.get_or_create_defined_stat(path);

        let stat_mut = self.definitions.get_mut(&path.path)
            .unwrap_or_else(|| panic!("Stat '{}' should be defined by get_or_create_defined_stat for set", path.path));
        stat_mut.set(path, value);
        
        self.update_cached_value(&path.path);
    }

    pub fn get(&self, path: &str) -> f32 {
        if self.cached_values.get(path).is_err() {
            self.cached_values.set(path, self.evaluate(&StatPath::parse(path).unwrap()));
        }
        self.cached_values.get(path).unwrap_or(0.0)
    }

    pub fn get_by_path(&self, path: &StatPath) -> f32 {
        self.get(&path.path)
    }

    fn evaluate(&self, path: &StatPath) -> f32 {
        let Some(stat) = self.definitions.get(&path.path) else {
            return 0.0;
        };

        stat.evaluate(path, self)
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

    // Helper method to update a cached value
    fn update_cached_value(&mut self, path_str: &str) {
        let path = StatPath::parse(path_str).unwrap();
        
        if let Some(stat) = self.definitions.get(path_str) {
            let value = stat.evaluate(&path, self);
            self.cached_values.set(path_str, value);
        }
    }

    fn post_update_stat(&mut self, path: &StatPath) {
        if let Some(stat) = self.definitions.get(&path.path) {
            stat.post_update(path, self);
        }
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

// Since stats are just strings, we need some way of knowing how a stat is configured based
// on its name alone. For instance, when we add "Life.Added" we need to know that that's a 
// StatType::Modifiable, where "Damage.Added.LIGHTNING" is a StatType::Tagged
#[derive(Default)]
pub struct StatConfig {
    // Maps stat path patterns to their corresponding StatType
    stat_type_patterns: HashMap<String, StatTypePattern>,
    
    // Default bases for common modifiable parts
    default_bases: HashMap<String, f32>,
    
    // Default expression templates
    default_expressions: HashMap<String, String>,
}

// Pattern matching to determine stat types based on path structure
struct StatTypePattern {
    pattern: Regex,
    stat_type_factory: fn() -> StatType,
}

impl StatConfig {
    // Get the global instance
    pub fn global() -> &'static Mutex<StatConfig> {
        static INSTANCE: OnceLock<Mutex<StatConfig>> = OnceLock::new();
        INSTANCE.get_or_init(|| Mutex::new(StatConfig::create_default()))
    }
    
    // Creates a default configuration
    fn create_default() -> Self {
        let mut config = StatConfig::default();
        
        // Set up default bases for common modifiers
        config.default_bases.insert("Added".to_string(), 0.0);
        config.default_bases.insert("Increased".to_string(), 1.0);
        config.default_bases.insert("More".to_string(), 1.0);
        config.default_bases.insert("Base".to_string(), 0.0);
        
        // Set up default expressions for complex stats
        config.default_expressions.insert(
            "Default".to_string(), 
            "Base + Added * (1 + Increased) * (1 + More)".to_string()
        );
        config.default_expressions.insert(
            "Damage".to_string(), 
            "Added + Base * (1 + Increased) * (1 + More)".to_string()
        );
        
        // Set up stat type patterns
        
        // Simple flat stats (e.g., "Strength", "Dexterity")
        config.add_pattern(
            r"^(Strength|Dexterity|Intelligence|Vitality)$",
            || StatType::Flat(Flat { base: 10.0 }) // Default base of 10 for primary attributes
        );
        
        // Derived complex stats (e.g., "Life", "Mana")
        config.add_pattern(
            r"^(Life|Mana|Shield|Energy)$",
            || {
                let mut parts = HashMap::new();
                
                // Add default parts
                parts.insert("Added".to_string(), Modifiable {
                    base: 0.0,
                    expressions: Vec::new(),
                });
                
                parts.insert("Increased".to_string(), Modifiable {
                    base: 1.0,
                    expressions: Vec::new(),
                });
                
                parts.insert("More".to_string(), Modifiable {
                    base: 1.0,
                    expressions: Vec::new(),
                });
                
                parts.insert("Base".to_string(), Modifiable {
                    base: 100.0, // Default base pool size
                    expressions: Vec::new(),
                });
                
                StatType::Complex(Complex {
                    total: Expression::new_unchecked("Base + Added * (1 + Increased) * (1 + More)"),
                    parts,
                })
            }
        );
        
        // Modifiable parts of complex stats (e.g., "Life.Added", "Mana.Increased")
        config.add_pattern(
            r"^(Life|Mana|Shield|Energy)\.(Added|Increased|More|Base)$",
            || {
                StatType::Modifiable(Modifiable {
                    base: 0.0, // Will be replaced based on the part type
                    expressions: Vec::new(),
                })
            }
        );
        
        // Damage stats with tags (e.g., "Damage.Added.FIRE", "Damage.Increased.PHYSICAL")
        config.add_pattern(
            r"^(Damage)$", // Base pattern for Tagged stats like "Damage"
            || {
                // The string "Damage" here is used to fetch the default expression for the total.
                StatType::Tagged(Tagged::new("Damage"))
            }
        );
        
        config
    }
    
    // Add a new pattern to match against stat paths
    fn add_pattern<S: Into<String>>(&mut self, pattern: S, factory: fn() -> StatType) {
        let regex = Regex::new(&pattern.into()).expect("Invalid regex pattern");
        self.stat_type_patterns.insert(
            regex.as_str().to_string(),
            StatTypePattern {
                pattern: regex,
                stat_type_factory: factory,
            }
        );
    }
    
    // Set a default base value for a modifier type
    fn set_default_base<S: Into<String>>(&mut self, modifier_type: S, value: f32) {
        self.default_bases.insert(modifier_type.into(), value);
    }
    
    // Set a default expression template
    fn set_default_expression<S1: Into<String>, S2: Into<String>>(&mut self, name: S1, expression: S2) {
        self.default_expressions.insert(name.into(), expression.into());
    }
    
    // Get default base value for a modifier type
    fn get_default_base(&self, modifier_type: &str) -> f32 {
        *self.default_bases.get(modifier_type).unwrap_or(&0.0)
    }
    
    // Get default expression template
    fn get_default_expression(&self, name: &str) -> Option<&String> {
        self.default_expressions.get(name).or_else(|| self.default_expressions.get("Default"))
    }
    
    // Create a StatType from a stat path
    fn build_stat_type_definition(&self, path: &StatPath) -> StatType {
        let path_str = &path.path;
        for (_, pattern_def) in &self.stat_type_patterns {
            if pattern_def.pattern.is_match(path_str) {
                let mut stat_type = (pattern_def.stat_type_factory)();
                self.customize_stat_type(&mut stat_type, path);
                return stat_type;
            }
        }
        let mut flat_stat = StatType::Flat(Flat { base: 0.0 });
        // self.customize_stat_type(&mut flat_stat, path); // Customize if Flat stats can have config-driven defaults
        flat_stat
    }

    // New public method
    pub fn create_and_register_stat_type(&self, path: &StatPath, stats_instance: &mut Stats) -> StatType {
        // 1. Build the basic StatType definition from configuration
        let mut new_stat_type = self.build_stat_type_definition(path);

        // 2. Call its 'register' method to interact with the live Stats instance.
        //    This is where Complex/Tagged types initialize their parts' cache values,
        //    dependencies, and their own initial cached value.
        new_stat_type.register(path, stats_instance);

        new_stat_type
    }
    
    // Customize a stat type based on the path
    fn customize_stat_type(&self, stat_type: &mut StatType, path: &StatPath) {
        match stat_type {
            StatType::Modifiable(modifiable) => {
                // If it's a modifiable part of a complex stat (e.g., "Life.Added")
                if path.parts.len() >= 2 {
                    let part_type = &path.parts[1];
                    modifiable.base = self.get_default_base(part_type);
                }
            },
            StatType::Complex(complex) => {
                // If stat name matches one of our expression templates, use it
                if let Some(expression_template) = self.get_default_expression(&path.parts[0]) {
                    complex.total = Expression::new_unchecked(expression_template);
                }
                
                // Set default bases for all parts
                for (part_name, part) in &mut complex.parts {
                    part.base = self.get_default_base(part_name);
                }
            },
            StatType::Tagged(tagged) => {
                // If stat name matches one of our expression templates, use it
                if let Some(expression_template) = self.get_default_expression(&path.parts[0]) {
                    tagged.total = Expression::new_unchecked(expression_template);
                }
            },
            _ => {}
        }
    }
}

// Stats cannot handle cross-entity stat updates, so all stat value changes must be done
// through the StatAccessor so it can keep everything up to date. 
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    query: Query<'w, 's, &'static mut Stats>,
}

impl StatAccessor<'_, '_> {
    pub fn add_modifier<M: Into<ModifierType>>(&mut self, target: Entity, path: &str, modifier: M) {

    }
    
    pub fn remove_modifier<M: Into<ModifierType>>(&mut self, target: Entity, path: &str, modifier: M) {

    }

    // Handle all value changing, cache updating, and dependency registration.
    pub fn add_modifier_value(&mut self, target: Entity, path: &StatPath, modifier: ModifierType) {
        let stats = self.query.get(target).unwrap();

        // Register dependencies for expressions
        if let ModifierType::Expression(expression) = &modifier {
            let dependencies = stats.get_dependencies(expression);
            self.add_dependents(target, path, dependencies);
        }
        
        // Get paths that will be affected before modifying
        let mut paths_to_update = Vec::new();
        if let Ok(stats) = self.query.get(target) {
            if let Some(stat) = stats.definitions.get(&path.path) {
                paths_to_update = stat.get_affected_paths(path);
            } else {
                paths_to_update = vec![path.path.clone()];
            }
        }

        // Apply the modifier
        let mut stats = self.query.get_mut(target).unwrap();
        if !stats.definitions.contains_key(&path.local_path) {
            // TODO maybe a stats.initialize(path) that calls stat.register on adding a new stat.
        }
        stats.add_modifier(path, modifier);
        
        // Update all affected paths
        for affected_path in paths_to_update {
            let affected_path_obj = StatPath::parse(&affected_path).unwrap();
            self.update(target, &affected_path_obj);
        }
    }
    
    // add_modifier_value but in reverse.
    pub fn remove_modifier_value(&mut self, target: Entity, path: &StatPath, modifier: &ModifierType) {
        let stats = self.query.get(target).unwrap();

        // Remove dependencies for expressions
        if let ModifierType::Expression(expression) = modifier {
            let dependencies = stats.get_dependencies(expression);
            self.remove_dependents(target, path, dependencies);
        }
        
        // Get paths that will be affected before modifying
        let mut paths_to_update = Vec::new();
        if let Ok(stats) = self.query.get(target) {
            if let Some(stat) = stats.definitions.get(&path.path) {
                paths_to_update = stat.get_affected_paths(path);
            } else {
                // If the stat doesn't exist, nothing to update
                return;
            }
        }

        // Apply the modification
        let mut stats = self.query.get_mut(target).unwrap();
        stats.remove_modifier(path, modifier);
        
        // Update all affected paths
        for affected_path in paths_to_update {
            let affected_path_obj = StatPath::parse(&affected_path).unwrap();
            self.update(target, &affected_path_obj);
        }
    }
    
    pub fn set(&mut self, target: Entity, path: &StatPath, value: f32) {
        // Get paths that will be affected before modifying
        let mut paths_to_update = Vec::new();
        if let Ok(stats) = self.query.get(target) {
            if let Some(stat) = stats.definitions.get(&path.path) {
                paths_to_update = stat.get_affected_paths(path);
            } else {
                // If the stat doesn't exist yet, we'll just update the path itself
                paths_to_update = vec![path.path.clone()];
            }
        } else {
            // Entity doesn't exist
            return;
        }

        // Apply the modification
        let mut stats = self.query.get_mut(target).unwrap();
        stats.set(path, value);
        
        // Update all affected paths
        for affected_path in paths_to_update {
            let affected_path_obj = StatPath::parse(&affected_path).unwrap();
            self.update(target, &affected_path_obj);
        }
    }

    // Registers a source. For instance target: item_entity, source_type: "EquippedTo", source: equipped_to_entity.
    // When a new source is registered the old cached values for that source must be updated. For instance, if 
    // we change who the sword is equipped to, we must update the "Strength@EquippedTo" cached value for the sword
    // and all dependent stat values (via the update function)
    pub fn register_source(&mut self, target: Entity, source_type: String, source: Entity) {
        // Get the target entity's stats
        let Ok(mut target_stats) = self.query.get_mut(target) else {
            error!("Target entity {:?} not found for registering source", target);
            return;
        };
        
        // Check if this is replacing an existing source
        let old_source = target_stats.sources.get(&source_type).cloned();
        
        // Update the source mapping
        target_stats.sources.insert(source_type.clone(), source);
        
        // If replacing a source, we need to update all dependencies
        if let Some(old_source_entity) = old_source {
            if old_source_entity == source {
                // Nothing changed, so no need to update
                return;
            }
            
            // Find all stats that depend on the changed source
            let affected_stats = {
                let mut affected = Vec::new();
                
                // Look through depends_on_map to find all stats that depend on the old source
                for (local_path, deps) in target_stats.depends_on_map.get_dependents() {
                    for (dep, _weight) in deps {
                        if let DependentType::Entity(entity, _) = dep {
                            if *entity == old_source_entity {
                                affected.push(local_path.clone());
                            }
                        }
                    }
                }
                
                affected
            };
            
            // We need to drop the mutable borrow before updating
            drop(target_stats);
            
            // Update dependencies on the old source to point to the new source
            self.update_source_dependencies(target, old_source_entity, source);
            
            // Update affected stats with new values from the new source
            for path_str in affected_stats {
                let path = StatPath::parse(&path_str).unwrap();
                self.update(target, &path);
            }
        } else {
            // If this is a new source (not replacing an existing one), we don't need to
            // update any existing dependencies since there are none yet
        }
    }

    // Helper method to update dependencies when a source changes
    fn update_source_dependencies(&mut self, target: Entity, old_source: Entity, new_source: Entity) {
        // Get all dependencies that need to be updated
        let dependencies_to_update = {
            let Ok(target_stats) = self.query.get(target) else {
                return;
            };
            
            let mut to_update = Vec::new();
            
            // Find all dependencies that reference the old source
            for (local_path, deps) in target_stats.depends_on_map.get_dependents() {
                for (dep, _weight) in deps {
                    if let DependentType::Entity(entity, remote_path) = dep {
                        if *entity == old_source {
                            to_update.push((local_path.clone(), remote_path.clone()));
                        }
                    }
                }
            }
            
            to_update
        };
        
        // For each dependency that needs updating
        for (local_path, remote_path) in dependencies_to_update {
            // 1. Remove the dependency from the old source entity
            if let Ok(mut old_source_stats) = self.query.get_mut(old_source) {
                let dependent_entry = DependentType::Entity(target, local_path.clone());
                old_source_stats.dependents_map.remove_dependent(&remote_path, dependent_entry);
            }
            
            // 2. Add the dependency to the new source entity
            if let Ok(mut new_source_stats) = self.query.get_mut(new_source) {
                let dependent_entry = DependentType::Entity(target, local_path.clone());
                new_source_stats.dependents_map.add_dependent(&remote_path, dependent_entry);
            }
            
            // 3. Update the depends_on_map in the target entity
            if let Ok(mut target_stats) = self.query.get_mut(target) {
                // Remove the old dependency
                let old_dependency = DependentType::Entity(old_source, remote_path.clone());
                target_stats.depends_on_map.remove_dependent(&local_path, old_dependency);
                
                // Add the new dependency
                let new_dependency = DependentType::Entity(new_source, remote_path);
                target_stats.depends_on_map.add_dependent(&local_path, new_dependency);
            }
        }
    }

    // go through all dependencies and add them in reverse. That is, DependentType::Entity(equipped_to_entity, "Strength")
    // will get the equipped_to_entity's Stats, and add DependencyType::Entity(target, "Life.Added") as the dependency of
    // the local stat "Strength".
    fn add_dependents(&mut self, target: Entity, modified_path: &StatPath, dependencies: Vec<DependentType>) {
        let modified_dependent = DependentType::Local(modified_path.path.clone()); // What depends *locally*
        
        for dependency in dependencies {
            match dependency {
                // The modified stat depends on a *local* stat (dep_path)
                DependentType::Local(dep_path) => {
                    if let Ok(mut stats) = self.query.get_mut(target) {
                        stats.dependents_map.add_dependent(&dep_path, modified_dependent.clone());
                        stats.depends_on_map.add_dependent(&modified_path.path, DependentType::Local(dep_path.clone()));
                    } else {
                        error!("Target entity {} not found for adding local dependency", target);
                    }
                }
                // The modified stat depends on a stat (dep_path) on *another* entity (source_entity)
                DependentType::Entity(source_entity, dep_path) => {
                    // We need to tell the *source* entity that the *target* entity's stat depends on it.
                    if let Ok(mut source_stats) = self.query.get_mut(source_entity) {
                        let dependent_entry = DependentType::Entity(target, modified_path.path.clone());
                        source_stats.dependents_map.add_dependent(&dep_path, dependent_entry.clone());
                        
                        // No need to modify this entity's depends_on_map, since depends_on_map is a local map
                    }
                    
                    // Update the target entity's depends_on_map to indicate it depends on an external stat
                    if let Ok(mut target_stats) = self.query.get_mut(target) {
                        target_stats.depends_on_map.add_dependent(
                            &modified_path.path,
                            DependentType::Entity(source_entity, dep_path.clone())
                        );
                    }
                    // It is not an error if the source entity does not exist yet. 
                }
            }
        }
    }
    
    fn remove_dependents(&mut self, target: Entity, modified_path: &StatPath, dependencies: Vec<DependentType>) {
        let modified_dependent = DependentType::Local(modified_path.path.clone());
        
        for dependency in dependencies {
            match dependency {
                DependentType::Local(dep_path) => {
                    if let Ok(mut stats) = self.query.get_mut(target) {
                        stats.dependents_map.remove_dependent(&dep_path, modified_dependent.clone());
                        stats.depends_on_map.remove_dependent(&modified_path.path, DependentType::Local(dep_path.clone()));
                    } // Else: Log error or handle missing entity
                }
                DependentType::Entity(source_entity, dep_path) => {
                    // Update source entity's dependents_map
                    if let Ok(mut source_stats) = self.query.get_mut(source_entity) {
                        let dependent_entry = DependentType::Entity(target, modified_path.path.clone());
                        source_stats.dependents_map.remove_dependent(&dep_path, dependent_entry.clone());
                    }
                    
                    // Update target entity's depends_on_map
                    if let Ok(mut target_stats) = self.query.get_mut(target) {
                        target_stats.depends_on_map.remove_dependent(
                            &modified_path.path,
                            DependentType::Entity(source_entity, dep_path.clone())
                        );
                    } // Else: Log error or handle missing entity
                }
            }
        }
    }

    // recursively go over all dependent stats, re-evaluate their values and update their caches.
    fn update(&mut self, target: Entity, path: &StatPath) {
        // Use a set to detect cycles and avoid redundant updates within one cascade
        let mut visited = bevy::utils::HashSet::new();
        self.update_recursive(target, path.path.clone(), &mut visited);
    }

    fn update_recursive(&mut self, target: Entity, path_str: String, visited: &mut HashSet<(Entity, String)>) {
        let key = (target, path_str.clone());
        if visited.contains(&key) {
            return; // Already processed in this update chain
        }
        visited.insert(key);

        // 1. Recalculate the value for target/path_str
        let new_value = self.evaluate_stat_internal(target, &path_str);

        // 2. Update the cache for target/path_str
        if let Ok(mut stats) = self.query.get_mut(target) {
            stats.cached_values.set(path_str.as_str(), new_value);
            
            // 3. Call post_update instead of register_dependencies_for_path
            let path = StatPath::parse(&path_str).unwrap();
            stats.post_update_stat(&path);
        } else {
            error!("Entity {} not found during cache update for path {}", target, path_str);
            return; // Cannot proceed without the entity
        }

        // 4. Process dependents
        let dependents_to_update = {
            let stats = self.query.get(target).unwrap(); // Assume exists after cache update
            
            // Clone the dependents to avoid borrow conflicts
            if let Some(deps) = stats.dependents_map.get_dependents().get(&path_str) {
                deps.clone()
            } else {
                HashMap::new()
            }
        };

        for (dependent_type, _count) in dependents_to_update {
            match dependent_type {
                DependentType::Local(dependent_path) => {
                    // Recursively update the stat on the *same* entity
                    self.update_recursive(target, dependent_path.clone(), visited);
                }
                DependentType::Entity(dependent_entity, dependent_path) => {
                    // Recursively update the stat on the *other* entity
                    self.update_recursive(dependent_entity, dependent_path.clone(), visited);
                }
            }
        }
    }

    // This needs access to the StatType definition and the caches of dependencies
    fn evaluate_stat_internal(&self, target: Entity, path_str: &str) -> f32 {
        let stats = self.query.get(target).expect("Target entity vanished during evaluation"); // Proper error handling needed

        stats.evaluate(&StatPath::parse(path_str).unwrap())
    }

    // handles safe tear-down of a destroyed stat entity. Removes all dependencies from relevant entities
    pub fn remove_stat_entity(&mut self, target_entity: Entity) {
        // Step 1: Clean up all outgoing dependencies (dependencies on other entities)
        // We need to find all cross-entity dependencies this entity has
        let outgoing_dependencies = {
            // Get the stats component for the target entity
            let Ok(stats) = self.query.get(target_entity) else {
                // If the entity doesn't have stats, nothing to do
                return;
            };

            let mut dependencies = Vec::new();
            
            // Go through all paths that this entity depends on
            for (stat_path, dependent_map) in stats.depends_on_map.get_dependents() {
                for (dependent, _weight) in dependent_map {
                    if let DependentType::Entity(source_entity, source_path) = dependent {
                        // This entity depends on another entity
                        dependencies.push((*source_entity, source_path.clone(), stat_path.clone()));
                    }
                }
            }
            
            dependencies
        };
        
        // Remove the outgoing dependencies from other entities' dependents_map
        for (source_entity, source_path, local_path) in outgoing_dependencies {
            if let Ok(mut source_stats) = self.query.get_mut(source_entity) {
                // Remove this entity as a dependent from the source entity
                let dependent_entry = DependentType::Entity(target_entity, local_path);
                source_stats.dependents_map.remove_dependent(&source_path, dependent_entry);
            }
        }
        
        // Step 2: Clean up all incoming dependencies (other entities that depend on this one)
        let incoming_dependencies = {
            // Get the stats component for the target entity
            let Ok(stats) = self.query.get(target_entity) else {
                // If the entity doesn't have stats, nothing to do
                return;
            };

            let mut dependencies = Vec::new();
            
            // Go through all stats that other entities might depend on
            for (stat_path, dependent_map) in stats.dependents_map.get_dependents() {
                for (dependent, _weight) in dependent_map {
                    if let DependentType::Entity(dependent_entity, dependent_path) = dependent {
                        // Another entity depends on this one
                        dependencies.push((*dependent_entity, dependent_path.clone(), stat_path.clone()));
                    }
                }
            }
            
            dependencies
        };
        
        // Remove the incoming dependencies from other entities' depends_on_map
        for (dependent_entity, dependent_path, local_path) in incoming_dependencies {
            if let Ok(mut dependent_stats) = self.query.get_mut(dependent_entity) {
                // Remove the dependency on this entity from the dependent entity
                let source_entry = DependentType::Entity(target_entity, local_path);
                dependent_stats.depends_on_map.remove_dependent(&dependent_path, source_entry);
                
                // Also update the dependent entity's cached values that relied on this entity
                // First get the path with the cache value to update
                let cache_key = dependent_path;
                
                // Then update that value with a default (usually 0.0)
                dependent_stats.cached_values.set(&cache_key, 0.0);
                
                // And propagate the change through the entity's dependency graph
                let stat_path = StatPath::parse(&cache_key).unwrap();
                self.update(dependent_entity, &stat_path);
            }
        }
        
        // The entity will be removed by Bevy's ECS after this function completes,
        // so we don't need to explicitly clean up its own internal maps -
        // those will be dropped when the entity is removed
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
    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType);
    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType);
    fn set(&mut self, path: &StatPath, value: f32);
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32;
    
    // Register dependencies and initialize caches when a stat is first added or updated
    fn register(&self, _path: &StatPath, _stats: &mut Stats) { }
    
    // New method to identify which paths need to be updated when this stat changes
    fn get_affected_paths(&self, path: &StatPath) -> Vec<String> {
        // Default implementation just returns the original path
        vec![path.path.clone()]
    }

    // Called after a stat's cached value has been updated
    // This allows the stat to perform any post-update operations like registering dependencies
    fn post_update(&self, _path: &StatPath, _stats: &Stats) { }
}

// A catch-all for stat types.
enum StatType {
    Flat(Flat),
    Modifiable(Modifiable),
    Complex(Complex),
    Tagged(Tagged),
}

impl StatLike for StatType {
    fn add_modifier(&mut self, path: &StatPath, value: ModifierType) {
        match self {
            StatType::Flat(flat) => flat.add_modifier(path, value),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(path, value),
            StatType::Complex(complex) => complex.add_modifier(path, value),
            StatType::Tagged(tagged) => tagged.add_modifier(path, value),
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ModifierType) {
        match self {
            StatType::Flat(flat) => flat.remove_modifier(path, value),
            StatType::Modifiable(modifiable) => modifiable.remove_modifier(path, value),
            StatType::Complex(complex) => complex.remove_modifier(path, value),
            StatType::Tagged(tagged) => tagged.remove_modifier(path, value),
        }
    }

    fn set(&mut self, path: &StatPath, value: f32) {
        match self {
            StatType::Flat(flat) => flat.set(path, value),
            StatType::Modifiable(_) => {},
            StatType::Complex(_) => {},
            StatType::Tagged(_) => {},
        }
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        match self {
            StatType::Flat(flat) => flat.evaluate(path, stats),
            StatType::Modifiable(modifiable) => modifiable.evaluate(path, stats),
            StatType::Complex(complex) => complex.evaluate(path, stats),
            StatType::Tagged(tagged) => tagged.evaluate(path, stats),
        }
    }
    
    fn register(&self, path: &StatPath, stats: &mut Stats) {
        match self {
            StatType::Flat(_) => {}, // No registration needed
            StatType::Modifiable(_) => {}, // No registration needed
            StatType::Complex(complex) => complex.register(path, stats),
            StatType::Tagged(tagged) => tagged.register(path, stats),
        }
    }
}

// Only has a base value. Cannot have expression modifiers added to it.
struct Flat {
    base: f32,
}

impl StatLike for Flat {
    fn add_modifier(&mut self, _path: &StatPath, value: ModifierType) {
        // For Flat stats, we only handle Literal values
        if let ModifierType::Literal(val) = value {
            self.base += val;
        }
        // Silently ignore Expression modifiers as they're not applicable
    }

    fn remove_modifier(&mut self, _path: &StatPath, value: &ModifierType) {
        // For Flat stats, we only handle Literal values
        if let ModifierType::Literal(val) = value {
            self.base -= val;
        }
        // Silently ignore Expression modifiers as they're not applicable
    }

    fn set(&mut self, _path: &StatPath, value: f32) {
        // Direct setting replaces the base value
        self.base = value;
    }

    fn evaluate(&self, _path: &StatPath, _stats: &Stats) -> f32 {
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
    fn add_modifier(&mut self, _path: &StatPath, value: ModifierType) {
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

    fn remove_modifier(&mut self, _path: &StatPath, value: &ModifierType) {
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

    fn set(&mut self, _path: &StatPath, _value: f32) { return }

    fn evaluate(&self, _path: &StatPath, stats: &Stats) -> f32 {
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
    fn add_modifier(&mut self, path: &StatPath, value: ModifierType) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1]; // Second part of the path is the modifier type (e.g., "Added")
            let config = StatConfig::global().lock().unwrap();
            let default_base = config.get_default_base(part_name);
            let part = self.parts.entry(part_name.clone()).or_insert_with(|| {
                Modifiable {
                    base: default_base,
                    expressions: Vec::new(),
                }
            });
            
            // Add the modifier to the appropriate part
            part.add_modifier(path, value);
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ModifierType) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get_mut(part_name) {
                part.remove_modifier(path, value);
            }
        }
    }

    fn set(&mut self, path: &StatPath, value: f32) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get_mut(part_name) {
                part.set(path, value);
            }
        }
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        // If path specifies a part, evaluate just that part
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get(part_name) {
                return part.evaluate(path, stats);
            }
            
            return 0.0;
        }
        
        // Otherwise, evaluate the total expression
        // First, ensure all parts are cached
        for (part_name, part) in &self.parts {
            let part_path = format!("{}.{}", path.parts[0], part_name);
            let part_value = part.evaluate(&StatPath::parse(&part_path).unwrap(), stats);
            stats.cached_values.set(&part_path, part_value);
        }
        
        // Then evaluate the total expression
        let context = stats.cached_values.context();
        self.total.evaluate(context)
    }

    fn register(&self, path: &StatPath, stats: &mut Stats) { // path is for the Complex stat, e.g., "Life"
        let base_name = &path.parts[0];

        for part_name_in_expr in self.total.compiled.iter_identifiers() { // e.g., "Base", "Added"
            let part_path_str = format!("{}.{}", base_name, part_name_in_expr);
            let part_s_path = StatPath::parse(&part_path_str).unwrap();

            // 1. Ensure the part stat itself (e.g., "Life.Added" as Modifiable) has a definition.
            //    This uses the Stats struct's own method, which will call StatConfig if needed.
            stats.get_or_create_defined_stat(&part_s_path);
            // After this, stats.definitions[&part_s_path.path] exists.

            // 2. Only set initial cache value for the part IF THE PART IS NOT ALREADY CACHED.
            if stats.cached_values.get(&part_path_str).is_err() {
                let stat_config = StatConfig::global().lock().unwrap(); // For default bases
                let initial_part_value = self.parts.get(part_name_in_expr)
                    .map_or_else(
                        || {
                            warn!("Complex stat '{}' has part '{}' in total expr but not in internal 'parts' map. Using default base from config.", path.path, part_name_in_expr);
                            stat_config.get_default_base(part_name_in_expr)
                        },
                        // Evaluate the Modifiable part definition *within* this Complex stat's `self.parts`.
                        |modifiable_part_in_complex| modifiable_part_in_complex.evaluate(&part_s_path, stats)
                    );
                stats.cached_values.set(&part_path_str, initial_part_value);
            }
            
            // 3. Register dependencies
            stats.dependents_map.add_dependent(&part_path_str, DependentType::Local(base_name.clone()));
            stats.depends_on_map.add_dependent(base_name, DependentType::Local(part_path_str.clone()));
        }
        
        // 4. Cache an initial value for the Complex stat itself ("Life")
        //    This uses the part values that might have just been placed in cache by this register call,
        //    OR values that were already in cache (e.g., if a part was modified by StatsInitializer
        //    before this Complex stat was registered).
        if stats.cached_values.get(&path.path).is_err() { // path.path is "Life"
            let total_value = self.evaluate(path, stats); 
            stats.cached_values.set(&path.path, total_value);
        }
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
    fn new(name: &str) -> Self {
        let config = StatConfig::global().lock().unwrap();
        let expr_template = config.get_default_expression(name)
            .unwrap_or_else(|| config.get_default_expression("Default").unwrap())
            .clone();
            
        Self {
            total: Expression::new_unchecked(&expr_template),
            parts: HashMap::new(),
            cached_queries: RwLock::new(HashMap::new()),
        }
    }
    
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
    fn add_modifier(&mut self, path: &StatPath, value: ModifierType) {
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

    fn remove_modifier(&mut self, path: &StatPath, value: &ModifierType) {
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

    fn set(&mut self, path: &StatPath, value: f32) {
        // Tagged stats don't support direct setting
        // Could implement if needed
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
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
    
    fn register(&self, path: &StatPath, stats: &mut Stats) { }

    fn get_affected_paths(&self, path: &StatPath) -> Vec<String> {
        let mut affected_paths = Vec::new();
        
        // If this is a specific part/tag path, find all cached paths that might be affected
        if path.parts.len() >= 3 {
            let base_path = &path.parts[0];
            let part_name = &path.parts[1];
            
            if let Ok(modified_tag) = path.parts[2].parse::<u32>() {
                let affected_queries = self.get_affected_queries();
                
                for (tag, parts) in affected_queries {
                    // Only include tags that match our modified tag
                    if has_matching_tag(modified_tag, tag) {
                        // If the modified part is in the cached queries, add it
                        if parts.contains(part_name) {
                            let part_path = format!("{}.{}.{}", base_path, part_name, tag);
                            affected_paths.push(part_path);
                        }
                        
                        // If the total is cached, add it
                        if parts.contains("_total") {
                            let total_path = format!("{}.{}", base_path, tag);
                            affected_paths.push(total_path);
                        }
                    }
                }
            }
        } else if path.parts.len() == 1 {
            // If this is just the base path, add all cached paths for this stat
            let base_path = &path.parts[0];
            let affected_queries = self.get_affected_queries();
            
            for (tag, parts) in affected_queries {
                for part in parts {
                    if part == "_total" {
                        let total_path = format!("{}.{}", base_path, tag);
                        affected_paths.push(total_path);
                    } else {
                        let part_path = format!("{}.{}.{}", base_path, part, tag);
                        affected_paths.push(part_path);
                    }
                }
            }
        }
        
        // If we didn't find any affected paths, include the original path
        if affected_paths.is_empty() {
            affected_paths.push(path.path.clone());
        }
        
        affected_paths
    }
    
    fn post_update(&self, path: &StatPath, stats: &Stats) {
        if path.parts.len() < 1 {
            return; // Invalid path
        }
        
        let base_type = &path.parts[0]; // e.g., "Damage", "Resistance", etc.
        
        // Get all previously queried combinations from the cache
        let cached_combinations = self.cached_queries.read().unwrap().clone();
        
        // For each previously queried tag
        for (tag, parts) in cached_combinations {
            // If the path had a specific tag, check if the queried tag is affected
            let should_update = if path.parts.len() >= 3 {
                if let Ok(modifier_tag) = path.parts[2].parse::<u32>() {
                    has_matching_tag(modifier_tag, tag)
                } else {
                    // Can't parse the tag, so update conservatively
                    true
                }
            } else {
                // No specific tag in the path, update all
                true
            };
            
            if !should_update {
                continue;
            }
            
            // Update all parts that were previously queried
            for part in &parts {
                // For queries like "Damage.5" (top-level with tag)
                if path.parts.len() == 2 {
                    let total_path = format!("{}.{}", base_type, tag);
                    let total_path_obj = StatPath::parse(&total_path).unwrap();
                    let total_value = self.evaluate(&total_path_obj, stats);
                    stats.cached_values.set(&total_path, total_value);
                }
                
                // For queries like "Damage.Added.5" (specific part with tag)
                let part_path = format!("{}.{}.{}", base_type, part, tag);
                let part_path_obj = StatPath::parse(&part_path).unwrap();
                let part_value = self.evaluate(&part_path_obj, stats);
                stats.cached_values.set(&part_path, part_value);
            }
        }
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
    trigger: Trigger<OnAdd, StatsInitializer>,
    query: Query<&StatsInitializer>,
    mut stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    let entity = trigger.entity();

    let Ok(stats_initializer) = query.get(entity) else {
        return;
    };

    stats_initializer.apply(&mut stat_accessor, &entity);

    commands.entity(entity).remove::<StatsInitializer>();
}

pub fn plugin(app: &mut App) {
    app.add_observer(initialize_stat_entity);
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::system::RunSystemOnce;
    // Assuming your crate items are accessible like this:
    use super::{
        plugin, Expression, Modifiable, ModifierSet, ModifierType, StatAccessor, StatConfig, StatPath, StatType, Stats, StatsInitializer, Tagged // User's new items
    };
    use bevy::prelude::*;
    use bevy::utils::{HashMap, HashSet};

    // Helper to create StatPath for tests more easily
    fn sp(path_str: &str) -> StatPath {
        StatPath::parse(path_str).unwrap()
    }

    // Helper for Literal ModifierType
    fn lit(val: f32) -> ModifierType {
        ModifierType::Literal(val)
    }

    // Helper for Expression ModifierType
    fn expr_mod(val: &str) -> ModifierType {
        ModifierType::Expression(Expression::new(val).unwrap())
    }

    // Tags for testing
    const TAG_NONE: u32 = 0;
    const TAG_FIRE: u32 = 1 << 0; // 1
    // const TAG_AXE: u32 = 1 << 10; // 1024 (Not used in these minimal tests)

    fn setup_app() -> App {
        let mut app = App::new();
        let mut config_guard = StatConfig::global().lock().unwrap();

        // Ensure critical patterns for tests are present in StatConfig
        if !config_guard.stat_type_patterns.contains_key(r"^(Damage|Resistance)$") {
            config_guard.add_pattern(
                r"^(Damage|Resistance)$",
                || StatType::Tagged(Tagged::new("Damage"))
            );
        }
        if !config_guard.stat_type_patterns.contains_key(r"^BonusStat$") {
             config_guard.add_pattern(
                r"^BonusStat$",
                || StatType::Modifiable(Modifiable { base: 0.0, expressions: Vec::new() })
            );
        }
        // Ensure "Power" defaults to Flat or has a pattern for it.
        // If no pattern matches, it defaults to Flat { base: 0.0 }, which is fine.
        drop(config_guard);
        
        app.add_plugins(plugin);

        app.update(); // Apply any changes from setup
        app
    }

    #[test]
    fn test_flat_stat_modification() {
        let mut app = setup_app();

        let initial_modifiers = ModifierSet::new(HashMap::from([
            // "Power" (Flat) defaults to base 0. Adding 50 makes it 50.
            ("Power".to_string(), vec![lit(50.0)]),
        ]));
        let entity = app.world_mut().spawn((
            Stats::default(),
            StatsInitializer { modifier_set: initial_modifiers },
        )).id();

        app.update(); // Run initialize_stat_entity system

        let stats = app.world().get::<Stats>(entity).unwrap();
        assert_eq!(stats.get("Power"), 50.0);

        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.add_modifier_value(entity, &sp("Power"), lit(10.0));
        });
        let stats = app.world().get::<Stats>(entity).unwrap();
        assert_eq!(stats.get("Power"), 60.0); // 50 + 10

        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.remove_modifier_value(entity, &sp("Power"), &lit(20.0));
        });
        let stats = app.world().get::<Stats>(entity).unwrap();
        assert_eq!(stats.get("Power"), 40.0); // 60 - 20
    }

    #[test]
    fn test_modifiable_stat_modification() {
        let mut app = setup_app();

        let initial_modifiers = ModifierSet::new(HashMap::from([
            // "BonusStat" (Modifiable) defaults to base 0.
            ("BonusStat".to_string(), vec![lit(30.0), expr_mod("5*2")]),
        ]));
        let entity = app.world_mut().spawn((
            Stats::default(),
            StatsInitializer { modifier_set: initial_modifiers },
        )).id();
        app.update();

        let stats = app.world().get::<Stats>(entity).unwrap();
        assert_eq!(stats.get("BonusStat"), 40.0); // 0 (base) + 30 (lit) + 10 (expr)

        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.remove_modifier_value(entity, &sp("BonusStat"), &expr_mod("5*2"));
        });
        let stats = app.world().get::<Stats>(entity).unwrap();
        assert_eq!(stats.get("BonusStat"), 30.0);

        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.remove_modifier_value(entity, &sp("BonusStat"), &lit(30.0));
        });
        let stats = app.world().get::<Stats>(entity).unwrap();
        assert_eq!(stats.get("BonusStat"), 0.0);
    }

    #[test]
    fn test_complex_stat_life() {
        let mut app = setup_app();

        // StatConfig for "Life" Complex's internal parts:
        // Base.base=100, Added.base=0, Increased.base=1, More.base=1.
        // We add lit(0.0) to "Life.Added" via initializer to ensure "Life" Complex is instantiated
        // and its default configured parts are used.
        // Internal "Added" part's base becomes 0 (initial from config) + 0 (from Initializer) = 0.
        let initial_modifiers = ModifierSet::new(HashMap::from([
            ("Life.Added".to_string(), vec![lit(100.0)]),
            ("Life.Increased".to_string(), vec![lit(1.0)]),
            ("Life.More".to_string(), vec![lit(1.0)]),
        ]));
        let entity = app.world_mut().spawn((
            Stats::default(),
            StatsInitializer { modifier_set: initial_modifiers },
        )).id();
        // app.update();

        // // Initial Life = 100 (Base part) + 0 (Added part) * (1 + 1 (Incr part)) * (1 + 1 (More part)) = 100.
        // let stats_ro = app.world().get::<Stats>(entity).unwrap();
        // assert_eq!(stats_ro.get("Life"), 100.0, "Initial Life value");

        // let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
        //     // "Life.Added" part's base (0) + 50.0 = 50.0
        //     accessor.add_modifier_value(entity, &sp("Life.Added"), lit(50.0));
        // });
        // // Life = 100 + 50 * (1+1) * (1+1) = 300.0
        // let stats_ro = app.world().get::<Stats>(entity).unwrap();
        // assert_eq!(stats_ro.get("Life"), 300.0, "Life after Added");

        // let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
        //     // "Life.Increased" part's base (1.0) + 0.5 = 1.5
        //     accessor.add_modifier_value(entity, &sp("Life.Increased"), lit(0.5));
        // });
        // // Life = 100 + 50 * (1+1.5) * (1+1) = 350.0
        // let stats_ro = app.world().get::<Stats>(entity).unwrap();
        // assert_eq!(stats_ro.get("Life"), 350.0, "Life after Increased");

        // let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
        //     // accessor.set for a part should overwrite that part's base.
        //     // "Life.Base" part's base (100.0) is set to 200.0.
        //     accessor.set(entity, &sp("Life.Base"), 200.0);
        // });
        // // Life = 200 + 50 * (1+1.5) * (1+1) = 450.0
        // let stats_ro = app.world().get::<Stats>(entity).unwrap();
        // assert_eq!(stats_ro.get("Life"), 450.0, "Life after Set Base part");
    }

    #[test]
    fn test_tagged_stat_damage() {
        let mut app = setup_app();

        // Initialize "Damage" Tagged stat by adding a dummy modifier through Initializer.
        // This ensures the "Damage" StatType::Tagged is created for the entity.
        // Parts (TaggedEntry) default bases from StatConfig: Added=0, Base=0, Increased=1.0, More=1.0.
        let initial_setup_mods = ModifierSet::new(HashMap::from([
            (format!("Damage.Added.{}", TAG_NONE), vec![lit(0.0)]),
        ]));
        let entity = app.world_mut().spawn((
            Stats::default(),
            StatsInitializer { modifier_set: initial_setup_mods.clone() }, // clone if needed again
        )).id();
        app.update(); // Apply initial_setup_mods

        // Now apply actual test modifiers using run_system_once for clarity,
        // though they could also be part of initial_setup_mods.
        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.add_modifier_value(entity, &sp(&format!("Damage.Added.{}", TAG_FIRE)), lit(10.0));
            accessor.add_modifier_value(entity, &sp(&format!("Damage.Increased.{}", TAG_FIRE)), lit(0.2));
            accessor.add_modifier_value(entity, &sp(&format!("Damage.Base.{}", TAG_FIRE)), lit(5.0));
        });
        app.update(); // Process these additions

        let stats_ro = app.world().get::<Stats>(entity).unwrap();
        // Values of parts for TAG_FIRE:
        // Added: 0 (default) + 10 = 10
        // Increased: 1.0 (default) + 0.2 = 1.2
        // Base: 0 (default) + 5 = 5
        // More: 1.0 (default)
        assert_eq!(stats_ro.get(&format!("Damage.Added.{}", TAG_FIRE)), 10.0);
        assert_eq!(stats_ro.get(&format!("Damage.Increased.{}", TAG_FIRE)), 1.2);
        assert_eq!(stats_ro.get(&format!("Damage.Base.{}", TAG_FIRE)), 5.0);
        assert_eq!(stats_ro.get(&format!("Damage.More.{}", TAG_FIRE)), 1.0);

        // Total Damage.FIRE: Added + Base * (1 + Increased) * (1 + More)
        // = 10.0 + 5.0 * (1 + 1.2) * (1 + 1.0) = 10.0 + 22.0 = 32.0
        assert_eq!(stats_ro.get(&format!("Damage.{}", TAG_FIRE)), 32.0);

        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.remove_modifier_value(entity, &sp(&format!("Damage.Added.{}", TAG_FIRE)), &lit(3.0));
        });
        // Damage.Added.TAG_FIRE is now 10.0 - 3.0 = 7.0
        let stats_ro_after_remove = app.world().get::<Stats>(entity).unwrap();
        assert_eq!(stats_ro_after_remove.get(&format!("Damage.Added.{}", TAG_FIRE)), 7.0);
        // Total Damage.FIRE: 7.0 + 22.0 = 29.0
        assert_eq!(stats_ro_after_remove.get(&format!("Damage.{}", TAG_FIRE)), 29.0);
    }

    #[test]
    fn test_source_entity_change() {
        let mut app = setup_app();

        let strength_a_mods = ModifierSet::new(HashMap::from([("Strength".to_string(), vec![lit(20.0)])]));
        let source_a = app.world_mut().spawn((
            Stats::default(),
            StatsInitializer { modifier_set: strength_a_mods },
        )).id();

        let strength_b_mods = ModifierSet::new(HashMap::from([("Strength".to_string(), vec![lit(40.0)])]));
        let source_b = app.world_mut().spawn((
            Stats::default(),
            StatsInitializer { modifier_set: strength_b_mods },
        )).id();
        app.update(); // Initialize source_a and source_b stats

        let dependent_mods = ModifierSet::new(HashMap::from([
            ("BonusStat".to_string(), vec![expr_mod("Strength@Player / 2")]),
        ]));
        let dependent_entity = app.world_mut().spawn((
            Stats::default(),
            StatsInitializer { modifier_set: dependent_mods },
        )).id();
        // Note: `register_source` is not part of StatsInitializer, so it's called separately.
        // `BonusStat` will initially be 0 because "Strength@Player" is 0.
        app.update(); // Initialize dependent_entity BonusStat (Strength@Player will be 0)


        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.register_source(dependent_entity, "Player".to_string(), source_a);
        });
        app.update(); // Allow register_source to trigger updates

        let dependent_stats = app.world().get::<Stats>(dependent_entity).unwrap();
        assert_eq!(dependent_stats.get("BonusStat"), 10.0, "BonusStat with source A"); // 20/2

        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
            accessor.register_source(dependent_entity, "Player".to_string(), source_b);
        });
        app.update();

        let dependent_stats_updated = app.world().get::<Stats>(dependent_entity).unwrap();
        assert_eq!(dependent_stats_updated.get("BonusStat"), 20.0, "BonusStat with source B"); // 40/2

        app.world_mut().entity_mut(source_b).despawn();
        app.update(); // Process despawn

        let _ = app.world_mut().run_system_once(move |mut accessor: StatAccessor| {
             if accessor.query.get(dependent_entity).is_ok() {
                let path_obj = sp("BonusStat");
                accessor.update(dependent_entity, &path_obj); // Force re-evaluation
            }
        });
        app.update();

        let dependent_stats_after_despawn = app.world().get::<Stats>(dependent_entity).unwrap();
        assert_eq!(dependent_stats_after_despawn.get("BonusStat"), 0.0, "BonusStat after source despawn");
    }
}