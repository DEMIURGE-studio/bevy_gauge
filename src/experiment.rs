use std::{cell::SyncUnsafeCell, sync::{Arc, RwLock}};

use bevy::{ecs::system::SystemParam, prelude::*, utils::{HashMap, HashSet}};
use evalexpr::{Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};

// Internal mutability used for cached_values and dependents_map because those should
// be able to be safely updated due to their values being derived from internal values.
// We actively update the values whenever a modifier changes, ensuring the cache is
// always up to date. 
#[derive(Component)]
struct Stats {
    definitions: HashMap<String, StatType>,
    cached_values: SyncContext,
    dependents_map: SyncDependents,
    sources: HashMap<String, Entity>,
}

impl Stats {
    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType) {

    }
    
    fn remove_modifier(&mut self, path: &StatPath, modifier: ModifierType) {

    }

    fn set(&mut self, path: &StatPath, value: f32) {
        // get the stat entry
        // if there is no stat at that address, make it a StatType::Flat and set its base
    }

    fn register_dependent(&mut self, dependent: &DependentType) {

    }

    fn get(&self, path: &StatPath) -> f32 {
        self.cached_values.get(&path.path)
    }

    fn evaluate(&self, path: &StatPath) -> f32 {
        let Some(stat) = self.definitions.get(&path.path) else {
            return 0.0;
        };

        stat.evaluate(path, self)
    }

    fn get_dependents(&self) -> HashMap<String, HashMap<DependentType, u32>> {
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
}

// Holds the memoized values for stats in the form of a HashMapContext, which can be
// used to evaluate stat values. When a stats value changes, it's cached value must be
// actively updated anywhere it exists.
struct SyncContext(SyncUnsafeCell<HashMapContext>);

impl SyncContext {
    fn new() -> Self {
        Self(SyncUnsafeCell::new(HashMapContext::new()))
    }

    fn get(&self, path: &str) -> f32 {
        unsafe {
            if let Some(stat_value) = (*self.0.get()).get_value(path.into()) {
                return stat_value.as_float().unwrap_or(0.0) as f32;
            }
        }
        0.0
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
struct SyncDependents(Arc<RwLock<HashMap<String, HashMap<DependentType, u32>>>>);

impl SyncDependents {
    fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
   
    fn add_dependent(&self, path: &str, dependent: DependentType) {
        if let Ok(mut graph) = self.0.write() {
            let entry = graph.entry(path.to_string()).or_insert_with(HashMap::new);
            *entry.entry(dependent).or_insert(0) += 1;
        }
    }
    
    fn remove_dependent(&self, path: &str, dependent: DependentType) {
        let Ok(mut graph) = self.0.write() else {
            return;
        };

        let Some(dependents) = graph.get_mut(path) else {
            return;
        };
        
        if let Some(weight) = dependents.get_mut(&dependent) {
            *weight -= 1;
            if *weight == 0 {
                dependents.remove(&dependent);
            }
        }
        if dependents.is_empty() {
            graph.remove(path);
        }
    }
    
    fn get_stat_dependents(&self, path: &str) -> Vec<DependentType> {
        if let Ok(graph) = self.0.read() {
            graph.get(path)
                .map(|dependents| dependents.keys().cloned().collect())
                .unwrap_or_else(Vec::new)
        } else {
            Vec::new()
        }
    }
    
    // TODO do we need this clone?
    fn get_dependents(&self) -> HashMap<String, HashMap<DependentType, u32>> {
        if let Ok(graph) = self.0.read() {
            graph.clone()
        } else {
            HashMap::new()
        }
    }
}

// Since stats are just strings, we need some way of knowing how a stat is configured based
// on its name alone. For instance, when we add "Life.Added" we need to know that that's a 
// StatType::Modifiable, where "Damage.Added.LIGHTNING" is a StatType::Tagged
#[derive(Resource)]
struct StatConfig {

}

// Stats cannot handle cross-entity stat updates, so all stat value changes must be done
// through the StatAccessor so it can keep everything up to date. 
#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    query: Query<'w, 's, &'static mut Stats>,
    config: Res<'w, StatConfig>,
}

impl StatAccessor<'_, '_> {
    // Handle all value changing, cache updating, and dependency registration.
    fn add_modifier_value(&mut self, target: Entity, path: &StatPath, modifier: ModifierType) {
        let stats = self.query.get(target).unwrap();

        if let ModifierType::Expression(expression) = &modifier {
            let dependencies = stats.get_dependencies(expression);
            self.add_dependents(target, path, dependencies);
        }

        let mut stats = self.query.get_mut(target).unwrap();
        stats.add_modifier(path, modifier);

        self.update(target, path);
    }
    
    // add_modifier_value but in reverse.
    fn remove_modifier_value(&mut self, target: Entity, path: &StatPath, modifier: ModifierType) {
        let stats = self.query.get(target).unwrap();

        if let ModifierType::Expression(expression) = &modifier {
            let dependencies = stats.get_dependencies(expression);
            self.remove_dependents(target, path, dependencies);
        }

        let mut stats = self.query.get_mut(target).unwrap();
        stats.remove_modifier(path, modifier);

        self.update(target, path);
    }

    fn set(&mut self, target: Entity, path: &StatPath, value: f32) {
        let Ok(mut stats) = self.query.get_mut(target) else {
            return;
        };

        stats.set(path, value);
        self.update(target, path);
    }

    // Registers a source. For instance target: item_entity, source_type: "EquippedTo", source: equipped_to_entity.
    // When a new source is registered the old cached values for that source must be updated. For instance, if 
    // we change who the sword is equipped to, we must update the "Strength@EquippedTo" cached value for the sword
    // and all dependent stat values (via the update function)
    fn register_source(&mut self, target: Entity, source_type: String, source: Entity) {

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
                    if let Ok(stats) = self.query.get(target) {
                        let mut map = stats.dependents_map.0.write().unwrap(); // Handle potential poisoning
                        let dependents = map.entry(dep_path).or_default();
                        *dependents.entry(modified_dependent.clone()).or_insert(0) += 1;
                    } else {
                        error!("Target entity {} not found for adding local dependency", target);
                    }
                }
                // The modified stat depends on a stat (dep_path) on *another* entity (source_entity)
                DependentType::Entity(source_entity, dep_path) => {
                    // We need to tell the *source* entity that the *target* entity's stat depends on it.
                    if let Ok(source_stats) = self.query.get(source_entity) {
                         let dependent_entry = DependentType::Entity(target, modified_path.path.clone());
                         let mut map = source_stats.dependents_map.0.write().unwrap(); // Handle potential poisoning
                         let dependents = map.entry(dep_path).or_default();
                        *dependents.entry(dependent_entry).or_insert(0) += 1;
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
                    if let Ok(stats) = self.query.get_mut(target) {
                        let mut map = stats.dependents_map.0.write().unwrap();
                        if let Some(dependents) = map.get_mut(&dep_path) {
                            if let Some(count) = dependents.get_mut(&modified_dependent) {
                                *count -= 1;
                                if *count == 0 {
                                    dependents.remove(&modified_dependent);
                                }
                            }
                            if dependents.is_empty() {
                                map.remove(&dep_path);
                            }
                        }
                    } // Else: Log error or handle missing entity
                }
                DependentType::Entity(source_entity, dep_path) => {
                    if let Ok(source_stats) = self.query.get_mut(source_entity) {
                        let dependent_entry = DependentType::Entity(target, modified_path.path.clone());
                        let mut map = source_stats.dependents_map.0.write().unwrap();
                        if let Some(dependents) = map.get_mut(&dep_path) {
                            if let Some(count) = dependents.get_mut(&dependent_entry) {
                                *count -= 1;
                                if *count == 0 {
                                    dependents.remove(&dependent_entry);
                                }
                            }
                            if dependents.is_empty() {
                                map.remove(&dep_path);
                            }
                        }
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

    // Helper function for recursion
    fn update_recursive(&mut self, target: Entity, path_str: String, visited: &mut bevy::utils::HashSet<(Entity, String)>) {
        let key = (target, path_str.clone());
        if visited.contains(&key) {
            return; // Already processed in this update chain
        }
        visited.insert(key);

        // 1. Recalculate the value for target/path_str
        let new_value = self.evaluate_stat_internal(target, &path_str); // Need this helper

        // 2. Update the cache for target/path_str
        if let Ok(stats) = self.query.get(target) {
            stats.cached_values.set(path_str.as_str(), new_value.into());
        } else {
            error!("Entity {} not found during cache update for path {}", target, path_str);
            return; // Cannot proceed without the entity
        }

        let stats = self.query.get(target).unwrap(); // Assume exists after cache update check
        let dependents = stats.get_dependents();
        let Some(dependents_to_update) = dependents.get(&path_str) else {
            return;
        };

        for (dependent_type, _count) in dependents_to_update {
            match dependent_type {
                DependentType::Local(dependent_path) => {
                    // Recursively update the stat on the *same* entity
                    self.update_recursive(target, dependent_path.clone(), visited);
                }
                DependentType::Entity(dependent_entity, dependent_path) => {
                    // Recursively update the stat on the *other* entity
                    self.update_recursive(*dependent_entity, dependent_path.clone(), visited);
                }
            }
        }
    }

    // This needs access to the StatType definition and the caches of dependencies
    fn evaluate_stat_internal(&self, target: Entity, path_str: &str) -> f32 {
        let stats = self.query.get(target).expect("Target entity vanished during evaluation"); // Proper error handling needed

        stats.evaluate(&StatPath::parse(path_str).unwrap())
    }

    // TODO handle safe tear-down of a destroyed stat entity. Should remove all dependencies from relevant entities
    pub fn remove_stat_entity(&mut self, target_entity: Entity) {

    }
}

// Modifiers can be flat (literal) values or expressions.
enum ModifierType {
    Literal(f32),
    Expression(Expression),
}

// Expressions store their definition and use evalexpr to calculate stat values.
struct Expression {
    definition: String,
    compiled: Node<DefaultNumericTypes>,
}

impl Expression {
    fn new(expression: &str) -> Self {
        Self {
            definition: expression.to_string(),
            compiled: evalexpr::build_operator_tree(expression).unwrap(),
        }
    }

    pub(crate) fn evaluate(&self, context: &HashMapContext) -> f32 {
        self.compiled
            .eval_with_context(context)
            .unwrap_or(Value::Float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)] // Added derives
struct StatPath {
    owner: Option<String>, // Name of the source (e.g., "EquippedTo", "Parent")
    path: String,          // The full original string (e.g., "Strength@EquippedTo")
    local_path: String,    // Path relative to the owner (e.g., "Strength")
    parts: Vec<String>,    // Local path split by '.' (e.g., ["Damage", "Added", "12"]), where 12 is a u32 representing arbitrary tags.
}

impl StatPath {
    // Simplified parser, needs more robust error handling
    fn parse<S: AsRef<str>>(path_str_ref: S) -> Result<Self, String> { // Return Result
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
           return Err(format!("Invalid stat path format: {}", path_str));
        }


        Ok(Self {
            owner,
            path: path_str.to_string(),
            local_path: local_path_str.to_string(),
            parts,
        })
    }
}

impl<T: Into<String>> From<T> for StatPath where T: AsRef<str> { // Generic From based on AsRef<str>
     fn from(value: T) -> Self {
         Self::parse(value.as_ref()).expect("Failed to parse stat path")
     }
}

// A common interface for adding modifiers, removing modifiers, etc. Should gracefully handle
// the unique problems that face specific stats. Not entirely sure what "on_update" is doing, 
// but there needs to be a built-in way for something like a Compound stat to add its part variants
// to the cache and its total variant as a dependent of its part variants (i.e., when "Life" is added
// we add "Life" as a dependent of "Life.Added", "Life.Increased", etc., AND we add "Life.Added" and
// the other variants to the cache.)
trait StatLike {
    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats);
    fn remove_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats);
    fn set(&mut self, path: &StatPath, value: f32, stats: &Stats);
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32;
    
    // Register dependencies and initialize caches when a stat is first added or updated
    fn register(&self, path: &StatPath, stats: &mut Stats) { }
    
    // Unregister dependencies when a stat is removed
    fn unregister(&self, path: &StatPath, stats: &mut Stats) { }
}

// A catch-all for stat types.
enum StatType {
    Flat(Flat),
    Modifiable(Modifiable),
    Complex(Complex),
    Tagged(Tagged),
}

impl StatLike for StatType {
    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats) {
        match self {
            StatType::Flat(flat) => flat.add_modifier(path, value, stats),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(path, value, stats),
            StatType::Complex(complex) => complex.add_modifier(path, value, stats),
            StatType::Tagged(tagged) => tagged.add_modifier(path, value, stats),
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats) {
        match self {
            StatType::Flat(flat) => flat.remove_modifier(path, value, stats),
            StatType::Modifiable(modifiable) => modifiable.remove_modifier(path, value, stats),
            StatType::Complex(complex) => complex.remove_modifier(path, value, stats),
            StatType::Tagged(tagged) => tagged.remove_modifier(path, value, stats),
        }
    }

    fn set(&mut self, path: &StatPath, value: f32, stats: &Stats) {
        match self {
            StatType::Flat(flat) => flat.set(path, value, stats),
            StatType::Modifiable(modifiable) => {},
            StatType::Complex(complex) => {},
            StatType::Tagged(tagged) => {},
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
    
    fn unregister(&self, path: &StatPath, stats: &mut Stats) {
        match self {
            StatType::Flat(_) => {}, // No unregistration needed
            StatType::Modifiable(_) => {}, // No unregistration needed
            StatType::Complex(complex) => complex.unregister(path, stats),
            StatType::Tagged(tagged) => tagged.unregister(path, stats),
        }
    }
}

// Only has a base value. Cannot have expression modifiers added to it.
struct Flat {
    base: f32,
}

impl StatLike for Flat {
    fn add_modifier(&mut self, _path: &StatPath, value: ModifierType, _stats: &Stats) {
        // For Flat stats, we only handle Literal values
        if let ModifierType::Literal(val) = value {
            self.base += val;
        }
        // Silently ignore Expression modifiers as they're not applicable
    }

    fn remove_modifier(&mut self, _path: &StatPath, value: ModifierType, _stats: &Stats) {
        // For Flat stats, we only handle Literal values
        if let ModifierType::Literal(val) = value {
            self.base -= val;
        }
        // Silently ignore Expression modifiers as they're not applicable
    }

    fn set(&mut self, _path: &StatPath, value: f32, _stats: &Stats) {
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
    fn add_modifier(&mut self, _path: &StatPath, value: ModifierType, _stats: &Stats) {
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

    fn remove_modifier(&mut self, _path: &StatPath, value: ModifierType, _stats: &Stats) {
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

    fn set(&mut self, _path: &StatPath, _value: f32, _stats: &Stats) { return }

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
    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1]; // Second part of the path is the modifier type (e.g., "Added")
            let part = self.parts.entry(part_name.clone()).or_insert_with(|| {
                // Create default Modifiable with appropriate base value
                // For multipliers, default to 1.0, for additive values, default to 0.0
                let default_base = if part_name == "Added" { 0.0 } else { 1.0 };
                Modifiable {
                    base: default_base,
                    expressions: Vec::new(),
                }
            });
            
            // Add the modifier to the appropriate part
            part.add_modifier(path, value, stats);
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get_mut(part_name) {
                part.remove_modifier(path, value, stats);
            }
        }
    }

    fn set(&mut self, path: &StatPath, value: f32, stats: &Stats) {
        // Get the appropriate part based on the path
        if path.parts.len() > 1 {
            let part_name = &path.parts[1];
            if let Some(part) = self.parts.get_mut(part_name) {
                part.set(path, value, stats);
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

    fn register(&self, path: &StatPath, stats: &mut Stats) {
        // Get the base name (e.g., "Life" from "Life" or "Life.Added")
        let base_name = path.parts[0].clone();
        
        for part_name in self.total.compiled.iter_identifiers() {
            let part_path = format!("{}.{}", base_name, part_name);
            let path_obj = StatPath::parse(&part_path).unwrap();
            
            // Get existing part or use default value
            let part_value = if let Some(part) = self.parts.get(part_name) {
                part.evaluate(&path_obj, stats)
            } else {
                // Default values: 0.0 for Added, 1.0 for multipliers
                if part_name == "Added" { 0.0 } else { 1.0 }
            };
            
            // Cache the part value
            stats.cached_values.set(&part_path, part_value);
            
            // 2. Register the base stat as dependent on each part
            let dependent = DependentType::Local(base_name.clone());
            stats.dependents_map.add_dependent(&part_path, dependent);
        }
        
        // 3. Evaluate and cache the total
        let total_value = self.evaluate(path, stats);
        stats.cached_values.set(&base_name, total_value);
    }
    
    fn unregister(&self, path: &StatPath, stats: &mut Stats) {
        let base_name = path.parts[0].clone();
        
        // Unregister the base stat as a dependent of each part
        for (part_name, _) in &self.parts {
            let part_path = format!("{}.{}", base_name, part_name);
            let dependent = DependentType::Local(base_name.clone());
            stats.dependents_map.remove_dependent(&part_path, dependent);
        }
    }
}

struct TaggedEntry {
    parts: HashMap<u32, Modifiable>,
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
struct Tagged {
    total: Expression,
    parts: HashMap<String, TaggedEntry>,
    // Track which paths have been queried for each tag using a HashSet
    queried_combinations: SyncUnsafeCell<HashMap<u32, HashSet<String>>>,
}

impl Tagged {
    fn new() -> Self {
        Self {
            total: Expression::new("Added + Base * Increased * More"),
            parts: HashMap::new(),
            queried_combinations: SyncUnsafeCell::new(HashMap::new()),
        }
    }
    
    fn update_cached_tags(&self, part_name: &str, tag: u32, stats: &Stats) {
        unsafe {
            let combinations = &mut *self.queried_combinations.get();
            
            // If this tag has been queried before
            if let Some(paths) = combinations.get(&tag) {
                for path_str in paths {
                    // Re-evaluate the path with this tag
                    let path = StatPath::parse(path_str).unwrap();
                    let new_value = self.evaluate(&path, stats);
                    
                    // Update the cached value in Stats only
                    stats.cached_values.set(path_str, new_value);
                }
            }
        }
    }
    
    fn get_or_evaluate_tag(&self, base: &str, part: &str, tag: u32, stats: &Stats) -> f32 {
        let path_str = format!("{}.{}.{}", base, part, tag);
        
        // Check if already cached in Stats
        let cached_value = stats.cached_values.get(&path_str);
        if cached_value != 0.0 {
            return cached_value;
        }
        
        // Evaluate the tag
        let part_entry = match self.parts.get(part) {
            Some(entry) => entry,
            None => return if part == "Added" { 0.0 } else { 1.0 }, // Default
        };
        
        let tag_entry = match part_entry.parts.get(&tag) {
            Some(entry) => entry,
            None => return if part == "Added" { 0.0 } else { 1.0 }, // Default
        };
        
        // Create a temporary path for evaluation
        let temp_path = StatPath::parse(&path_str).unwrap();
        let value = tag_entry.evaluate(&temp_path, stats);
        
        // Cache the result in Stats
        stats.cached_values.set(&path_str, value);
        
        // Remember that this tag was queried
        unsafe {
            let combinations = &mut *self.queried_combinations.get();
            let tag_paths = combinations.entry(tag).or_insert_with(HashSet::new);
            tag_paths.insert(path_str);
        }
        
        value
    }
}

impl StatLike for Tagged {
    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats) {
        // Extract tag from path (assuming format like "Damage.Added.FIRE" where FIRE is a u32)
        if path.parts.len() < 3 {
            return; // Invalid path for tagged stat
        }
        
        let part_name = &path.parts[1]; // e.g., "Added"
        let tag_str = &path.parts[2]; // e.g., "14"
        let tag: u32 = match tag_str.parse() {
            Ok(num) => num,
            Err(_) => {
                error!("Invalid tag format: {}", tag_str);
                return;
            }
        };
        
        // Get or create the appropriate part type
        let part_entry = self.parts.entry(part_name.clone())
            .or_insert_with(|| TaggedEntry { parts: HashMap::new() });
        
        // Get or create the specific tag entry
        let tag_entry = part_entry.parts.entry(tag)
            .or_insert_with(|| Modifiable { 
                base: if part_name == "Added" { 0.0 } else { 1.0 },
                expressions: Vec::new()
            });
            
        // Add the modifier to the specific tag entry
        tag_entry.add_modifier(path, value, stats);
        
        // Update all cached values for this tag
        self.update_cached_tags(part_name, tag, stats);
    }
    
    fn remove_modifier(&mut self, path: &StatPath, value: ModifierType, stats: &Stats) {
        // Similar to add_modifier but removing
        if path.parts.len() < 3 {
            return; // Invalid path for tagged stat
        }
        
        let part_name = &path.parts[1];
        let tag_str = &path.parts[2];
        let tag: u32 = match tag_str.parse() {
            Ok(num) => num,
            Err(_) => {
                error!("Invalid tag format: {}", tag_str);
                return;
            }
        };
        
        // Find the part
        let Some(part_entry) = self.parts.get_mut(part_name) else {
            return; // Part doesn't exist
        };
        
        // Find the tag
        let Some(tag_entry) = part_entry.parts.get_mut(&tag) else {
            return; // Tag doesn't exist
        };
        
        // Remove the modifier
        tag_entry.remove_modifier(path, value, stats);
        
        // Clean up empty entries
        if tag_entry.expressions.is_empty() && tag_entry.base == (if part_name == "Added" { 0.0 } else { 1.0 }) {
            part_entry.parts.remove(&tag);
        }
        
        if part_entry.parts.is_empty() {
            self.parts.remove(part_name);
        }
        
        // Update all cached values for this tag
        self.update_cached_tags(part_name, tag, stats);
    }
    
    fn set(&mut self, path: &StatPath, value: f32, stats: &Stats) {
        // For direct setting of tag values
        if path.parts.len() < 3 {
            return; // Invalid path for tagged stat
        }
        
        let part_name = &path.parts[1];
        let tag_str = &path.parts[2];
        let tag: u32 = match tag_str.parse() {
            Ok(num) => num,
            Err(_) => {
                error!("Invalid tag format: {}", tag_str);
                return;
            }
        };
        
        // Get or create the appropriate part type
        let part_entry = self.parts.entry(part_name.clone())
            .or_insert_with(|| TaggedEntry { parts: HashMap::new() });
        
        // Get or create the specific tag entry
        let tag_entry = part_entry.parts.entry(tag)
            .or_insert_with(|| Modifiable { 
                base: if part_name == "Added" { 0.0 } else { 1.0 },
                expressions: Vec::new()
            });
        
        // Set the base value directly
        tag_entry.base = value;
        
        // Update cached values
        self.update_cached_tags(part_name, tag, stats);
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        // Must have at least a tag (e.g., "Damage.FIRE" or "Damage.Added.FIRE")
        if path.parts.len() < 2 {
            return 0.0; // Tagged stats require a tag
        }
        
        // Case: "Damage.FIRE" (base with tag)
        if path.parts.len() == 2 {
            let base_name = &path.parts[0];
            let tag_str = &path.parts[1];
            let tag: u32 = match tag_str.parse() {
                Ok(num) => num,
                Err(_) => {
                    error!("Invalid tag format: {}", tag_str);
                    return 0.0;
                }
            };
            
            // We need to evaluate all parts for this tag and apply the formula
            let context = stats.cached_values.context();
            
            // Ensure all parts for this tag are cached
            for part_name in self.total.compiled.iter_identifiers() {
                let part_path = format!("{}.{}.{}", base_name, part_name, tag);
                let value = self.get_or_evaluate_tag(base_name, part_name, tag, stats);
                stats.cached_values.set(&part_path, value);
            }
            
            // Evaluate the total formula for this tag
            if let Ok(val) = self.total.compiled.eval_with_context(context) {
                if let Ok(float_val) = val.as_float() {
                    return float_val as f32;
                }
            }
            
            return 0.0;
        }
        
        // Case: "Damage.Added.FIRE" (specific part with tag)
        if path.parts.len() == 3 {
            let part_name = &path.parts[1];
            let tag_str = &path.parts[2];
            let tag: u32 = match tag_str.parse() {
                Ok(num) => num,
                Err(_) => {
                    error!("Invalid tag format: {}", tag_str);
                    return if part_name == "Added" { 0.0 } else { 1.0 }; // Default value
                }
            };
            
            return self.get_or_evaluate_tag(&path.parts[0], part_name, tag, stats);
        }
        
        0.0 // Fallback
    }
    
    fn register(&self, path: &StatPath, stats: &mut Stats) {
        // For tagged stats, we don't pre-register all possible tags
        // Instead, we register dependencies as they're evaluated
        
        // The base path (e.g., "Damage") depends on its components
        if path.parts.len() == 1 {
            // Register the formula parts
            for part_name in self.total.compiled.iter_identifiers() {
                // We'll register individual tag dependencies when they're first queried
                // This avoids having to register all possible tags
            }
        }
        
        // If a specific tag path (e.g., "Damage.Added.FIRE"), register it
        if path.parts.len() == 3 {
            let tag_str = &path.parts[2];
            let tag: u32 = match tag_str.parse() {
                Ok(num) => num,
                Err(_) => return,
            };
            
            // The base+tag (e.g., "Damage.FIRE") depends on its components
            let base_name = &path.parts[0];
            let part_name = &path.parts[1];
            let base_tag_path = format!("{}.{}", base_name, tag_str);
            
            // Register this component as a dependency for the base+tag
            let part_tag_path = format!("{}.{}.{}", base_name, part_name, tag_str);
            let dependent = DependentType::Local(base_tag_path);
            stats.dependents_map.add_dependent(&part_tag_path, dependent);
            
            // Cache the initial value
            if let Some(part_entry) = self.parts.get(part_name) {
                if let Some(tag_entry) = part_entry.parts.get(&tag) {
                    let temp_path = StatPath::parse(&part_tag_path).unwrap();
                    let value = tag_entry.evaluate(&temp_path, stats);
                    stats.cached_values.set(&part_tag_path, value);
                    
                    // Remember that this tag was queried
                    unsafe {
                        let combinations = &mut *self.queried_combinations.get();
                        let tag_paths = combinations.entry(tag).or_insert_with(HashSet::new);
                        tag_paths.insert(part_tag_path);
                    }
                }
            }
        }
    }
    
    fn unregister(&self, path: &StatPath, stats: &mut Stats) {
        // Similar to register but removing dependencies
        if path.parts.len() == 3 {
            let tag_str = &path.parts[2];
            let tag: u32 = match tag_str.parse() {
                Ok(num) => num,
                Err(_) => return,
            };
            
            let base_name = &path.parts[0];
            let part_name = &path.parts[1];
            let base_tag_path = format!("{}.{}", base_name, tag_str);
            let part_tag_path = format!("{}.{}.{}", base_name, part_name, tag_str);
            
            // Unregister dependency
            let dependent = DependentType::Local(base_tag_path);
            stats.dependents_map.remove_dependent(&part_tag_path, dependent);
            
            // Remove from cache
            unsafe {
                let combinations = &mut *self.queried_combinations.get();
                if let Some(tag_paths) = combinations.get_mut(&tag) {
                    tag_paths.remove(&part_tag_path);
                    if tag_paths.is_empty() {
                        combinations.remove(&tag);
                    }
                }
            }
        }
    }
}