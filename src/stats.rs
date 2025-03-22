use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use bevy::prelude::*;
use evalexpr::build_operator_tree;
use log::warn;
use crate::prelude::*;
use crate::value_type::{Expression, StatError, ValueBounds, ValueType};


#[derive(Resource, Debug, Default)]
pub struct StatDependencyRegistry {
    pub dependencies: HashMap<String, HashSet<String>>, // Health <- Strength, Resolve
    pub dependents: HashMap<String, HashSet<String>>, // Resolve -> Health, Strength -> Health
    
    // Pre-calculated global resolution order (most independent to most dependent)
    pub resolution_order: Vec<String>,
    // Track order position for quick lookups
    pub order_map: HashMap<String, usize>,
    // Whether resolution order needs updating
    needs_update: bool,
}

impl StatDependencyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_dependencies(&mut self, stat_name: &str, dependencies: HashSet<String>) {
        if !dependencies.is_empty() {
            // Check if this is a new stat or has new dependencies
            let is_new_stat = !self.dependencies.contains_key(stat_name);
            let has_new_deps = match self.dependencies.get(stat_name) {
                Some(existing_deps) => !dependencies.is_subset(existing_deps),
                None => true
            };

            // Only mark for update if something changed
            if is_new_stat || has_new_deps {
                self.needs_update = true;
            }

            // Store forward dependencies
            self.dependencies.insert(stat_name.to_string(), dependencies.clone());

            // Store reverse dependencies for each dependency
            for dep in &dependencies {
                self.dependents
                    .entry(dep.clone())
                    .or_default()
                    .insert(stat_name.to_string());
            }
        }
    }

    // Get all stats that depend on a given stat
    pub fn get_dependents(&self, stat_name: &str) -> HashSet<String> {
        self.dependents
            .get(stat_name)
            .cloned()
            .unwrap_or_default()
    }

    // Get all dependencies of a stat
    pub fn get_dependencies(&self, stat_name: &str) -> HashSet<String> {
        self.dependencies
            .get(stat_name)
            .cloned()
            .unwrap_or_default()
    }

    // Extract dependencies from an expression and register them
    pub fn register_expression(&mut self, stat_name: &str, expression: &Expression) {
        let deps = expression.extract_dependencies();
        self.register_dependencies(stat_name, deps);
    }

    // Update the global resolution order if needed
    pub fn update_resolution_order(&mut self) {
        if !self.needs_update && !self.resolution_order.is_empty() {
            return; // Already up to date
        }

        // Get all stats in the system
        let all_stats: HashSet<_> = self.dependencies.keys()
            .chain(self.dependents.keys())
            .cloned()
            .collect();

        self.resolution_order = self.topological_sort(&all_stats);

        // Update the order map for quick lookups
        self.order_map.clear();
        for (i, stat) in self.resolution_order.iter().enumerate() {
            self.order_map.insert(stat.clone(), i);
        }

        self.needs_update = false;
    }

    // Get position of a stat in the resolution order
    pub fn get_stat_order(&self, name: &str) -> Option<usize> {
        self.order_map.get(name).cloned()
    }

    // Perform a topological sort on a set of stats
    fn topological_sort(&self, stats: &HashSet<String>) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        // Process independent stats first (stats with no dependencies)
        let independent_stats: Vec<_> = stats.iter()
            .filter(|s| !self.dependencies.contains_key(*s) ||
                self.dependencies.get(*s).map_or(true, |deps| deps.is_empty()))
            .cloned()
            .collect();

        for stat in independent_stats {
            visited.insert(stat.clone()); // Clone before inserting
            result.push(stat);            // Push the original
        }

        // Then process dependent stats
        for stat in stats {
            if !visited.contains(stat) {
                self.depth_first_search(
                    stat,
                    &mut result,
                    &mut visited,
                    &mut temp_visited
                );
            }
        }

        result
    }

    // Helper for topological sort
    fn depth_first_search(
        &self,
        stat: &str,
        result: &mut Vec<String>,
        visited: &mut HashSet<String>,
        temp_visited: &mut HashSet<String>,
    ) {
        // Skip if already processed
        if visited.contains(stat) {
            return;
        }

        // Detect cycles
        if temp_visited.contains(stat) {
            warn!("Circular dependency detected involving stat: {}", stat);
            return;
        }

        // Mark as temporarily visited
        temp_visited.insert(stat.to_string());

        // Process dependencies first
        let dependencies = self.get_dependencies(stat);
        for dep in dependencies {
            self.depth_first_search(&dep, result, visited, temp_visited);
        }

        // Mark as fully visited
        temp_visited.remove(stat);
        visited.insert(stat.to_string());
        result.push(stat.to_string());
    }

    // Get an optimized resolution order for a specific collection
    pub fn get_optimized_order(&self, available_stats: &HashSet<String>) -> Vec<String> {
        // Make sure global order is up to date
        if self.needs_update || self.resolution_order.is_empty() {
            // This is mutable self in an immutable context, so we can't modify
            // Just return a fresh calculation for this specific set
            return self.topological_sort(available_stats);
        }

        // Filter global order to only include stats in this collection
        self.resolution_order.iter()
            .filter(|stat| available_stats.contains(*stat))
            .cloned()
            .collect()
    }

}




#[derive(Debug, Clone, Default)]
pub struct StatInstance {
    pub value: ValueType,
    pub bounds: Option<ValueBounds>
}

impl StatInstance {
    pub fn new(value: ValueType, bounds: Option<ValueBounds>) -> Self {
        Self {
            value,
            bounds
        }
    }
    pub fn from_f32(val: f32) -> Self {
        Self {
            value: ValueType::Literal(val),
            bounds: None
        }
    }
    
    pub fn from_expression(expression: Expression) -> Self {
        Self {
            value: ValueType::Expression(expression),
            bounds: None
        }
    }
}

#[derive(Component, Debug, Default, Clone, DerefMut, Deref)]
// #[require(StatContext)]
pub struct StatCollection {
    #[deref]
    pub stats: HashMap<String, StatInstance>,
    pub pending_stats: HashMap<String, HashSet<String>>, // Key is stat that is hanging, hashset is collection of missing stats
}

impl StatCollection {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            pending_stats: HashMap::new(),
        }
    }

    // Insert a stat and update resolution order if needed
    pub fn insert(&mut self, name: String, instance: StatInstance, registry: &mut StatDependencyRegistry) {
        // If this is an expression, check if all dependencies are available
        if let ValueType::Expression(ref expr) = instance.value {
            let deps = expr.extract_dependencies();

            // Identify missing dependencies
            let missing_deps: HashSet<String> = deps
                .iter()
                .filter(|dep| !self.stats.contains_key(*dep))
                .cloned()
                .collect();

            // If there are missing dependencies, add to pending stats
            if !missing_deps.is_empty() {
                self.pending_stats.insert(name.clone(), missing_deps);
            }

            // Register the expression with the global registry
            registry.register_expression(&name, &expr);
        }

        // Insert the stat
        self.stats.insert(name.clone(), instance);

        // Try to resolve any pending stats
        self.resolve_pending_stats(registry);
    }

    // Get the value of a stat by name, evaluating it if necessary
    pub fn get_str(&self, stat: &str) -> Result<f32, StatError> {
        match self.stats.get(stat) {
            Some(stat_type) => Ok(stat_type.value.evaluate(self)),
            None => Err(StatError::NotFound(stat.to_string())),
        }
    }

    // Get the value of a stat by name, evaluating it if necessary
    pub fn get<S: AsRef<str>>(&self, stat: S) -> Result<f32, StatError> {
        self.get_str(stat.as_ref())
    }

    // Resolve any pending stats when a new stat is added
    fn resolve_pending_stats(&mut self, registry: &StatDependencyRegistry) {
        // Collect stats to process - avoids borrowing issues
        let stats_to_process: Vec<String> = self.stats.keys().cloned().collect();

        for stat_name in stats_to_process {
            // Get stats that depend on this one
            let dependents = registry.get_dependents(&stat_name);

            // Update pending stats that depend on this one
            let mut newly_available = Vec::new();

            for dependent in dependents {
                if let Some(missing_deps) = self.pending_stats.get_mut(&dependent) {
                    // Remove this dependency since it's now available
                    missing_deps.remove(&stat_name);

                    // If no more missing dependencies, mark for resolution
                    if missing_deps.is_empty() {
                        newly_available.push(dependent.clone());
                    }
                }
            }

            // Remove fully resolved pending stats and evaluate them
            for resolved_stat in newly_available {
                self.pending_stats.remove(&resolved_stat);

                // Evaluate the expression if it exists
                if let Some(stat) = self.stats.get(&resolved_stat) {
                    if let ValueType::Expression(_) = stat.value {
                        // Clone to avoid borrowing issues
                        let bounds = stat.bounds.clone();

                        // Get the value from ValueType's evaluate method
                        let value = stat.value.evaluate(self);

                        // Create a new stat instance with the calculated value
                        let new_stat = StatInstance {
                            value: ValueType::Literal(value),
                            bounds,
                        };

                        // Update the collection (outside this loop)
                        self.stats.insert(resolved_stat.clone(), new_stat);
                    }
                }
            }
        }
    }

    // Update all stats based on global resolution order
    pub fn update_all(&mut self, registry: &StatDependencyRegistry) -> Result<(), StatError> {
        // Get all stats in this collection
        let available_stats: HashSet<_> = self.stats.keys().cloned().collect();

        // Get optimized resolution order for this collection
        let order = registry.get_optimized_order(&available_stats);

        // Evaluate each stat in order
        for stat_name in order {
            if let Some(stat) = self.stats.get(&stat_name) {
                if let ValueType::Expression(ref expr) = stat.value {
                    // Evaluate the expression (ValueType has the evaluate method)
                    let value = stat.value.evaluate(self);

                    // Create new stat instance with calculated value
                    let new_stat = StatInstance {
                        value: ValueType::Literal(value),
                        bounds: stat.bounds.clone(),
                    };

                    // Update the collection
                    self.stats.insert(stat_name.clone(), new_stat);
                }
            }
        }

        Ok(())
    }

    // Get a list of hanging stats with their missing dependencies
    pub fn get_hanging_stats(&self) -> &HashMap<String, HashSet<String>> {
        &self.pending_stats
    }

    // Add a batch of stats at once
    pub fn batch_insert(&mut self, stats: Vec<(String, StatInstance)>, registry: &mut StatDependencyRegistry) {
        let mut expressions_to_register = Vec::new();

        for (name, instance) in stats {
            // Register expression dependencies later (after all stats are inserted)
            if let ValueType::Expression(ref expr) = instance.value {
                expressions_to_register.push((name.clone(), expr.clone()));

                let deps = expr.extract_dependencies();

                // Identify missing dependencies
                let missing_deps: HashSet<String> = deps
                    .iter()
                    .filter(|dep| !self.stats.contains_key(*dep))
                    .cloned()
                    .collect();

                // If there are missing dependencies, add to pending stats
                if !missing_deps.is_empty() {
                    self.pending_stats.insert(name.clone(), missing_deps);
                }
            }

            // Insert the stat
            self.stats.insert(name, instance);
        }

        // Register all expressions with the registry
        for (name, expr) in expressions_to_register {
            registry.register_expression(&name, &expr);
        }

        // Update registry resolution order
        registry.update_resolution_order();

        // Resolve pending stats
        self.resolve_pending_stats(registry);
    }
}

fn update_stats(
    stat_entity_query: Query<Entity, Changed<StatContext>>,
    mut commands: Commands,
) {
    for entity in stat_entity_query.iter() {
        // TODO
    }
}


pub(crate) fn plugin(app: &mut App) {
    app.add_systems(AddStatComponent, (
        update_stats,
    ));
}


fn create_test_collection() -> (StatCollection, StatDependencyRegistry) {
    let mut registry = StatDependencyRegistry::new();
    let mut collection = StatCollection::default();

    // Insert basic stats
    collection.insert("health".to_string(), StatInstance::from_f32(100.0), &mut registry);
    collection.insert("armor".to_string(), StatInstance::from_f32(50.0), &mut registry);
    collection.insert("strength".to_string(), StatInstance::from_f32(25.0), &mut registry);
    collection.insert("agility".to_string(), StatInstance::from_f32(30.0), &mut registry);

    // Insert derived stats
    let damage_expr = Expression(build_operator_tree("strength * 2 + agility / 2").unwrap());
    collection.insert("damage".to_string(), StatInstance::from_expression(damage_expr), &mut registry);

    let defense_bonus_expr = Expression(build_operator_tree("defense + 1").unwrap());
    collection.insert("defense_bonus".to_string(), StatInstance::from_expression(defense_bonus_expr), &mut registry);

    let defense_expr = Expression(build_operator_tree("armor + health * 0.1").unwrap());
    collection.insert("defense".to_string(), StatInstance::from_expression(defense_expr), &mut registry);


    // Make sure global resolution order is up to date
    registry.update_resolution_order();

    (collection, registry)
}

#[test]
fn test_literal_values() {
    let (collection, _registry) = create_test_collection();
    assert_eq!(collection.get("health").unwrap(), 100.0);
    assert_eq!(collection.get("armor").unwrap(), 50.0);
    assert_eq!(collection.get("strength").unwrap(), 25.0);
    assert_eq!(collection.get("agility").unwrap(), 30.0);
    assert_eq!(collection.get("damage").unwrap(), 65.0);
    assert_eq!(collection.get("defense").unwrap(), 60.0);
    assert_eq!(collection.get("defense_bonus").unwrap(), 61.0);
}

#[test]
fn test_resolution_order() {
    let (collection, registry) = create_test_collection();

    // Get the resolution order
    let available_stats: HashSet<_> = collection.stats.keys().cloned().collect();
    let order = registry.get_optimized_order(&available_stats);

    // Verify basic stats come before derived stats
    assert!(order.iter().position(|s| s == "health").unwrap() < order.iter().position(|s| s == "defense").unwrap());
    assert!(order.iter().position(|s| s == "armor").unwrap() < order.iter().position(|s| s == "defense").unwrap());

    // Verify multi-level dependencies are properly ordered
    assert!(order.iter().position(|s| s == "defense").unwrap() < order.iter().position(|s| s == "defense_bonus").unwrap());

    // Print the resolution order for debugging
    println!("Resolution order: {:?}", order);
}

#[test]
fn test_hanging_stats() {
    let mut registry = StatDependencyRegistry::new();
    let mut collection = StatCollection::default();

    // Add just some stats, leaving dependencies unresolved
    collection.insert("health".to_string(), StatInstance::from_f32(100.0), &mut registry);
    collection.insert("strength".to_string(), StatInstance::from_f32(25.0), &mut registry);

    // Add a stat that depends on an unavailable stat
    let missing_dep_expr = Expression(build_operator_tree("missing_stat * 2").unwrap());
    collection.insert("derived_with_missing".to_string(), StatInstance::from_expression(missing_dep_expr), &mut registry);

    // Add a stat with multiple missing dependencies
    let multi_missing_expr = Expression(build_operator_tree("missing_stat_1 + missing_stat_2").unwrap());
    collection.insert("multi_missing".to_string(), StatInstance::from_expression(multi_missing_expr), &mut registry);

    // Verify hanging stats are tracked correctly
    let hanging = collection.get_hanging_stats();
    assert_eq!(hanging.len(), 2);
    assert!(hanging.contains_key("derived_with_missing"));
    assert!(hanging.contains_key("multi_missing"));

    // Verify missing dependencies are tracked correctly
    assert!(hanging.get("derived_with_missing").unwrap().contains("missing_stat"));
    assert_eq!(hanging.get("multi_missing").unwrap().len(), 2);

    // // Convert to the original format for compatibility
    // let hanging_by_deps = get_hanging_stats_by_dependencies(&collection);
    // assert_eq!(hanging_by_deps.len(), 3); // Should have 3 unique dependency sets
}

#[test]
fn test_resolving_dependencies() {
    let mut registry = StatDependencyRegistry::new();
    let mut collection = StatCollection::default();

    // Add a base stat
    collection.insert("base".to_string(), StatInstance::from_f32(10.0), &mut registry);

    // Add a stat that depends on a missing stat
    let dependent_expr = Expression(build_operator_tree("base + missing").unwrap());
    collection.insert("dependent".to_string(), StatInstance::from_expression(dependent_expr), &mut registry);

    // Verify it's hanging
    assert_eq!(collection.get_hanging_stats().len(), 1);

    // Now add the missing dependency
    collection.insert("missing".to_string(), StatInstance::from_f32(5.0), &mut registry);

    // Verify dependency is resolved
    assert_eq!(collection.get_hanging_stats().len(), 0);

    // Verify value is calculated correctly
    assert_eq!(collection.get("dependent").unwrap(), 15.0);
}
