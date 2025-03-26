use std::collections::{HashMap, HashSet};
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::*;
use evalexpr::{
    ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use crate::modifiers::{ModifierInstance, ModifierValue, ModifierValueTotal};
use crate::prelude::AttributeInstance;
use crate::resource::ResourceInstance;
use crate::tags::TagRegistry;
use crate::value_type::*;

#[derive(Debug)]
pub enum StatError {
    BadOpp(String),
    NotFound(String),
}



#[derive(Debug, Clone, PartialEq, Default)]
pub struct Intermediate {
    // tag ID to (entities with modifiers for this tag, total modifier value)
    pub tags: HashMap<u32, (EntityHashSet, ModifierValueTotal)>,
    // entity to set of tag IDs it affects
    pub modifiers: EntityHashMap<HashSet<u32>>,
}

#[derive(Debug, Clone, Default, Deref, DerefMut)]
pub struct StatInstance {
    // Dependencies and dependents now use strings instead of ValueTags
    pub dependencies: HashSet<String>,
    pub dependents: HashSet<String>,

    #[deref]
    pub stat: StatType,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum StatType {
    Attribute(AttributeInstance),
    Resource(ResourceInstance),
    Intermediate(Intermediate),
    #[default]
    Empty,
}

impl StatType {
    pub fn get_value(&self) -> f32 {
        match self {
            StatType::Attribute(attr) => attr.get_value_f32(),
            StatType::Resource(resource) => resource.current,
            StatType::Intermediate(intermediate) =>
            // Assuming Intermediate now has a get_total method
                intermediate.get_total(),
            StatType::Empty => 0.0,
        }
    }
    
    pub fn get_dependencies(&self) -> Option<HashSet<String>> {
        match self {
            StatType::Attribute(attr) => {attr.value().extract_dependencies()}
            StatType::Resource(resource) => {resource.bounds.extract_bound_dependencies()}
            StatType::Intermediate(intermediate) => { None } // TODO NEED TO UPDATE TO UPDATE MODIFIERS BASED ON CHANGING STATS
            StatType::Empty => { None }
        }
    }
}


// Implementation for Intermediate
impl Intermediate {
    // Helper method to get total value
    pub fn get_total(&self) -> f32 {
        // Sum up all modifier values
        self.tags.values()
            .map(|(_, modifier_value)| modifier_value.get_total())
            .sum()
    }

    pub fn add_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        for (tag, &mut (ref mut vset, ref mut modifier_value)) in self.tags.iter_mut() {
            // Check if this modifier qualifies for this tag using bitwise AND
            if tag & modifier.source_tag > 0 {  // Changed from > 1 to > 0
                vset.insert(modifier_entity);
                *modifier_value += &modifier.value;
                self.modifiers.entry(modifier_entity).or_insert(HashSet::new()).insert(*tag);
            }
        }
    }

    pub fn remove_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        if let Some(target_tags) = self.modifiers.get(&modifier_entity) {
            let tags_to_remove: Vec<u32> = target_tags.iter().copied().collect();

            for target_tag in tags_to_remove {
                if let Some(&mut (ref mut map, ref mut modifier_total)) = self.tags.get_mut(&target_tag) {
                    map.remove(&modifier_entity);
                    *modifier_total -= &modifier.value;

                    if map.is_empty() {
                        self.tags.remove(&target_tag);
                    }
                }
            }
        }

        self.modifiers.remove(&modifier_entity);
    }
}



#[derive(Component, Debug, Default, Clone, DerefMut, Deref)]
pub struct StatCollection {
    #[deref]
    pub stats: HashMap<String, StatInstance>,

    pub pending_stats: HashMap<String, HashSet<String>>, // Key is attribute that is hanging, hashset is collection of missing attributes
}

impl StatCollection {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            pending_stats: HashMap::new(),
        }
    }

    pub fn insert(&mut self, tag: &str, stat_type: StatType) {
        let tag_string = tag.to_string();

        let dependencies = stat_type.get_dependencies();
        // If this is an expression, check if all dependencies are available
        match &stat_type {
            StatType::Attribute(attribute) => {
                let dependents: HashSet<String> = self
                    .stats
                    .iter()
                    .filter(|(_, val)| val.dependencies.contains(&tag_string))
                    .map(|(key, _)| key.clone())
                    .collect();
                
                
                if let Some(ref dependencies) = dependencies {
                    let missing_deps: HashSet<String> = dependencies
                        .iter()
                        .filter(|&dep| !self.stats.contains_key(dep))
                        .cloned()
                        .collect();


                    if !missing_deps.is_empty() {
                        self.pending_stats.insert(tag_string.clone(), missing_deps);
                    }

                    for dependency in dependencies.clone() {
                        self.stats.entry(dependency).and_modify(|stat| {
                            stat.dependents.insert(tag_string.clone());
                        });
                    }
                }


                // If there are missing dependencies, add to pending attributes

                let stat_instance = self.stats.entry(tag_string.clone()).or_insert(Default::default());
                stat_instance.dependencies = dependencies.clone().unwrap_or(HashSet::new());
                stat_instance.dependents = dependents;
                stat_instance.stat = stat_type;
            }
            StatType::Resource(resource) => {
                let mut stat_instance = StatInstance::default();
                stat_instance.stat = StatType::Resource(resource.clone());
                self.stats.insert(tag_string.clone(), stat_instance);
            }
            StatType::Intermediate(intermediate) => {
                let mut stat_instance = StatInstance::default();
                stat_instance.stat = StatType::Intermediate(intermediate.clone());
                self.stats.insert(tag_string.clone(), stat_instance);
            }
            _ => {}
        }

        self.recalculate(&tag_string);
        self.resolve_pending_stats();
        // Mark cache as dirty since stats changed
    }

    // Resolve any pending attributes when a new attribute is added
    fn resolve_pending_stats(&mut self) {
        // Create a queue of stats that might be resolvable now
        let mut to_check: Vec<String> = Vec::new();

        // First pass: identify stats that might be resolvable now
        let pending_keys: Vec<String> = self.pending_stats.keys().cloned().collect();
        for tag in pending_keys {
            let mut dependencies_met = true;

            // Check if all dependencies for this stat are now available
            if let Some(missing_deps) = self.pending_stats.get(&tag) {
                for dep in missing_deps {
                    if !self.stats.contains_key(dep) {
                        dependencies_met = false;
                        break;
                    }
                }
            }

            // If all dependencies are met, add to the resolution queue
            if dependencies_met {
                to_check.push(tag);
            }
        }

        // Process the queue
        while let Some(tag) = to_check.pop() {
            // Remove this tag from pending stats
            if self.pending_stats.remove(&tag).is_some() {
                // Now check if any stats that were waiting for this one can be resolved
                let mut new_resolvable = Vec::new();

                for (pending_tag, pending_deps) in &mut self.pending_stats {
                    // Remove this tag from the missing dependencies
                    pending_deps.remove(&tag);

                    // If all dependencies are now met, add to the resolution queue
                    if pending_deps.is_empty() {
                        new_resolvable.push(pending_tag.clone());
                    }
                }

                // Add the newly resolvable stats to the queue
                to_check.extend(new_resolvable);
            }
        }
    }

    // Add a batch of attributes at once with tree-walking resolution
    // pub fn batch_insert(&mut self, attributes: Vec<(&str, AttributeInstance)>) {
    //     // Insert non-expression attributes first (they have no dependencies)
    //     let mut expressions = Vec::new();
    // 
    //     for (tag, attr) in attributes {
    //         match &attr.value {
    //             ValueType::Literal(_) => {
    //                 // Literal values can be inserted immediately
    //                 self.insert(tag, StatType::Attribute(attr));
    //             }
    //             ValueType::Expression(_) => {
    //                 // Save expression attributes for dependency resolution
    //                 expressions.push((tag, attr));
    //             }
    //         }
    //     }
    // 
    //     // Now try to resolve expressions in passes until we make no more progress
    //     let mut remaining = expressions;
    //     let mut progress = true;
    // 
    //     while progress && !remaining.is_empty() {
    //         progress = false;
    //         let mut next_remaining = Vec::new();
    // 
    //         for (tag, attr) in remaining {
    //             if let ValueType::Expression(expr) = &attr.value {
    //                 let deps = expr.extract_dependencies();
    // 
    //                 // Check if all dependencies are available
    //                 let all_deps_available = deps.iter().all(|dep| self.stats.contains_key(dep));
    // 
    //                 if all_deps_available {
    //                     // All dependencies are available, insert this attribute
    //                     self.insert(tag, StatType::Attribute(attr));
    //                     progress = true;
    //                 } else {
    //                     // Some dependencies are still missing, keep for next pass
    //                     next_remaining.push((tag, attr));
    //                 }
    //             }
    //         }
    // 
    //         remaining = next_remaining;
    //     }
    // 
    //     // If we still have remaining expressions, they have circular dependencies
    //     // Just insert them anyway and let resolve_pending_stats handle them
    //     for (tag, attr) in remaining {
    //         self.insert(tag, StatType::Attribute(attr));
    //     }
    // }

    // Recalculate a stat and its dependents using tree-walking
    pub fn recalculate(&mut self, tag: &str) {
        // Simple set to track which stats we've processed
        let mut processed = HashSet::new();

        // Start from the given tag and walk outward to dependents
        self.tree_walk_calculate(tag, &mut processed);
    }

    fn tree_walk_calculate(&mut self, start_tag: &str, processed: &mut HashSet<String>) {
        // Skip if already processed
        if processed.contains(start_tag) {
            return;
        }

        // Mark as processed to avoid cycles
        processed.insert(start_tag.to_string());

        // First collect dependents to avoid borrow issues during recursion
        let dependents = if let Some(stat) = self.stats.get(start_tag) {
            stat.dependents.iter().cloned().collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        // Update this stat's value
        self.update_dependent_stat(start_tag);

        // Process dependents without holding mutable borrow
        for dependent in dependents {
            self.tree_walk_calculate(&dependent, processed);
        }
    }

    // Get a stat value by string
    pub fn get(&mut self, stat: &str) -> Result<f32, StatError> {
        match self.stats.get(stat) {
            Some(stat_instance) => Ok(stat_instance.stat.get_value()),
            None => Err(StatError::NotFound(stat.to_string())),
        }
    }

    // Update a dependent stat's value
    pub fn update_dependent_stat(&mut self, stat: &str) {
        // 1. First check if we need to update this stat and extract necessary info
        let mut needs_update = false;
        let mut variable_names = Vec::new();

        if let Some(stat_instance) = self.stats.get(stat) {
            if let StatType::Attribute(attribute) = &stat_instance.stat {
                if let ValueType::Expression(expression) = &attribute.value {
                    needs_update = true;
                    // Collect variable names before any mutable borrows
                    variable_names.extend(
                        expression
                            .iter_variable_identifiers()
                            .map(|s| s.to_string()),
                    );
                }
            }
        }

        if !needs_update {
            return;
        }

        // 2. Collect all variable values BEFORE any mutable borrows
        let mut variable_values = HashMap::new();

        // Use thread-local to track stack for cycle detection
        thread_local! {
            static EVAL_STACK: std::cell::RefCell<HashSet<String>> =
                std::cell::RefCell::new(HashSet::new());
        }

        for var_name in &variable_names {
            let is_cyclic = EVAL_STACK.with(|stack| stack.borrow().contains(var_name));

            let val = if is_cyclic {
                0.0 // Break cycles
            } else {
                // Add to stack to detect cycles
                EVAL_STACK.with(|stack| stack.borrow_mut().insert(var_name.clone()));

                // Get value safely without recursive mutable borrowing
                let result = self.get(var_name).unwrap_or(0.0);

                // Remove from stack
                EVAL_STACK.with(|stack| stack.borrow_mut().remove(var_name));

                result as f64
            };

            variable_values.insert(var_name.clone(), val);
        }

        // 3. NOW we can mutably borrow to update the expression
        if let Some(stat_instance) = self.stats.get_mut(stat) {
            if let StatType::Attribute(attribute) = &mut stat_instance.stat {
                if let ValueType::Expression(expression) = &mut attribute.value {
                    // Create context with all our pre-collected variable values
                    let mut context = HashMapContext::new();
                    for (name, value) in variable_values {
                        context
                            .set_value(name, EvalValue::from_float(value))
                            .unwrap();
                    }

                    // Evaluate expression with the prepared context
                    expression.cached_value = expression
                        .eval_with_context_mut(&mut context)
                        .unwrap_or(EvalValue::from_float(0.0))
                        .as_number()
                        .unwrap_or(0.0) as f32;
                }
            }
        }
    }

    // Recalculate all stats using tree-walking
    pub fn recalculate_all(&mut self) {
        // Find all stats with no dependencies (roots of the tree)
        let root_stats: Vec<String> = self
            .stats
            .keys()
            .filter(|tag| {
                self.stats
                    .get(*tag)
                    .map_or(true, |instance| instance.dependencies.is_empty())
            })
            .cloned()
            .collect();

        // Process each root stat first
        let mut processed = HashSet::new();
        for root in root_stats {
            self.tree_walk_calculate(&root, &mut processed);
        }

        // Then process any remaining stats that weren't reached
        let all_stats: Vec<String> = self.stats.keys().cloned().collect();
        for tag in all_stats {
            if !processed.contains(&tag) {
                self.tree_walk_calculate(&tag, &mut processed);
            }
        }
    }

    // Get a list of hanging attributes with their missing dependencies
    pub fn get_hanging_attributes(&self) -> &HashMap<String, HashSet<String>> {
        &self.pending_stats
    }
}
