use crate::modifiers::{IntermediateModifierValue, ModifierCollectionRefs, ModifierInstance, ModifierStorageType, ModifierStorage, ModifierValue, ModifierValueTotal, StatUpdatedEvent, BitMaskedStatModifierStorage};
use std::collections::{HashMap, HashSet};
use bevy::prelude::*;
// use crate::modifiers::{Intermediate};
use crate::prelude::AttributeInstance;
use crate::resource::ResourceInstance;

#[derive(Debug)]
pub enum StatError {
    BadOpp(String),
    NotFound(String),
}

#[derive(Debug, Clone, Default)]
pub struct StatInstance {
    // Dependencies and dependents now use strings instead of ValueTags
    pub dependencies: HashSet<String>,
    pub dependents: HashSet<String>,

    pub modifier_collection: ModifierStorage,
    
    pub stat: StatType,
}

impl StatInstance {
    
    pub fn add_replace_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        self.modifier_collection.add_or_replace_modifier(modifier, modifier_entity);
    }
    
    pub fn remove_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        self.modifier_collection.remove_modifier(modifier, modifier_entity);
    }
    
    pub fn new(stat_type: StatType, modifier_storage: ModifierStorage) -> Self {
        Self {
            dependents: HashSet::new(),
            dependencies: HashSet::new(),
            
            modifier_collection: modifier_storage,
            
            stat: stat_type
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum StatType {
    Attribute(AttributeInstance),
    Resource(ResourceInstance),
    // Intermediate(Intermediate),
    #[default]
    Empty,
}

impl StatType {
    pub fn get_value(&self) -> f32 {
        match self {
            StatType::Attribute(attr) => attr.get_value_f32(),
            StatType::Resource(resource) => resource.current,
            // StatType::Intermediate(intermediate) => {0.0}
            // Assuming Intermediate now has a get_total method
            StatType::Empty => 0.0,
        }
    }
    
    pub fn get_dependencies(&self) -> Option<HashSet<String>> {
        let x = match self {
            StatType::Attribute(attr) => { attr.value().extract_dependencies() },
            StatType::Resource(resource) => {resource.bounds.extract_bound_dependencies()},
            // StatType::Intermediate(intermediate) => { None }, // TODO NEED TO UPDATE TO UPDATE MODIFIERS BASED ON CHANGING STATS
            StatType::Empty => { None },
        };
        x
    }
}



#[derive(Component, Debug, Default, Clone, DerefMut, Deref)]
#[require(ModifierCollectionRefs)]
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

    pub fn insert(&mut self, tag: &str, mut stat_instance: StatInstance) {
        let tag_string = tag.to_string();

        let dependencies = stat_instance.stat.get_dependencies();
        // If this is an expression, check if all dependencies are available
        match stat_instance.stat {
            StatType::Attribute(ref attribute) => {
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

                // let stat_instance = self.stats.entry(tag_string.clone()).or_insert(Default::default());
                stat_instance.dependencies = dependencies.clone().unwrap_or(HashSet::new());
                stat_instance.dependents = dependents;
                self.stats.insert(tag_string.clone(), stat_instance);
            }
            StatType::Resource(resource) => {
                let mut stat_instance = StatInstance::default();
                stat_instance.stat = StatType::Resource(resource.clone());
                self.stats.insert(tag_string.clone(), stat_instance);
            }
            // StatType::Intermediate(intermediate) => {
            //     let mut stat_instance = StatInstance::default();
            //     stat_instance.stat = StatType::Intermediate(intermediate.clone());
            //     self.stats.insert(tag_string.clone(), stat_instance);
            // }
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

        // Two-step process to avoid borrowing issues:
        // 1. Clone the StatValue from the attribute (if it exists)
        let mut stat_value = if let Some(stat_instance) = self.stats.get(stat) {
            if let StatType::Attribute(attribute) = &stat_instance.stat {
                Some(attribute.value().clone())
            } else {
                None
            }
        } else {
            None
        };

        // 2. If we have a stat value, update it with the current stat collection
        if let Some(ref mut value) = stat_value {
            value.set_value_with_context(self);

            // 3. Now place the updated value back in the attribute
            if let Some(stat_instance) = self.stats.get_mut(stat) {
                if let StatType::Attribute(attribute) = &mut stat_instance.stat {
                    *attribute.value_mut() = value.clone();
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
    
    pub fn add_replace_modifier(&mut self, target: &str, modifier: &ModifierInstance, modifier_entity: Entity) {
        if let Some(stat) = self.get_mut(target) {
            stat.add_replace_modifier(modifier, modifier_entity);
        }
    }
    
    pub fn remove_modifier(&mut self, target: &str, modifier: &ModifierInstance, modifier_entity: Entity) {
        if let Some(stat) = self.get_mut(target) {
            stat.remove_modifier(modifier, modifier_entity);
        }
    }
}

pub fn on_modifier_change(
    trigger: Trigger<StatUpdatedEvent>,
    mut query: Query<&mut StatCollection>
) {
    let Ok(mut stats) = query.get_mut(trigger.target()) else { todo!() };
    let x = stats.get("ATTRIBUTE").unwrap();
    stats.recalculate(&trigger.stat_name);
}


#[cfg(test)]
mod stat_tests {
    use super::*;
    use crate::prelude::AttributeInstance;
    use crate::resource::ResourceInstance;
    use crate::value_type::*;
    use evalexpr::build_operator_tree;

    // Helper function to create an attribute with a literal value
    fn create_literal_attribute(value: f32) -> AttributeInstance {
        let stat_value = StatValue::from_f32(value);
        AttributeInstance::new(stat_value)
    }

    // Helper function to create an attribute with an expression
    fn create_expression_attribute(expr_str: &str) -> AttributeInstance {
        let node = build_operator_tree(expr_str).unwrap();
        let expression = Expression::new(node);
        let stat_value = StatValue::from_expression(expression);
        AttributeInstance::new(stat_value)
    }

    #[test]
    fn test_literal_stat_value() {
        let mut stats = StatCollection::new();

        // Insert a literal stat
        let attr = create_literal_attribute(42.0);
        stats.insert("health", StatInstance::new(StatType::Attribute(attr), ModifierStorage::default()));

        // Check value
        assert_eq!(stats.get("health").unwrap(), 42.0);
    }

    #[test]
    fn test_expression_stat_value() {
        let mut stats = StatCollection::new();

        // Insert a literal stat first
        let base_attr = create_literal_attribute(10.0);
        stats.insert("base", StatInstance::new(StatType::Attribute(base_attr), ModifierStorage::default()));

        // Insert an expression that references the first stat
        let expr_attr = create_expression_attribute("base * 2");
        stats.insert("derived", StatInstance::new(StatType::Attribute(expr_attr), ModifierStorage::default()));

        // Check values
        assert_eq!(stats.get("base").unwrap(), 10.0);
        assert_eq!(stats.get("derived").unwrap(), 20.0);
    }

    #[test]
    fn test_expression_chain() {
        let mut stats = StatCollection::new();

        // Create a chain of stats
        let attr1 = create_literal_attribute(5.0);
        stats.insert("a", StatInstance::new(StatType::Attribute(attr1), ModifierStorage::default()));

        let attr2 = create_expression_attribute("a + 10");
        stats.insert("b", StatInstance::new(StatType::Attribute(attr2), ModifierStorage::default()));

        let attr3 = create_expression_attribute("b * 2");
        stats.insert("c", StatInstance::new(StatType::Attribute(attr3), ModifierStorage::default()));

        // Check values
        assert_eq!(stats.get("a").unwrap(), 5.0);
        assert_eq!(stats.get("b").unwrap(), 15.0);
        assert_eq!(stats.get("c").unwrap(), 30.0);
    }

    #[test]
    fn test_update_propagation() {
        let mut stats = StatCollection::new();

        // Create base stat
        let attr1 = create_literal_attribute(5.0);
        stats.insert("base", StatInstance::new(StatType::Attribute(attr1), ModifierStorage::default()));

        // Create derived stat
        let attr2 = create_expression_attribute("base * 3");
        stats.insert("derived", StatInstance::new(StatType::Attribute(attr2), ModifierStorage::default()));

        // Initial check
        assert_eq!(stats.get("base").unwrap(), 5.0);
        assert_eq!(stats.get("derived").unwrap(), 15.0);

        // Update base stat
        let new_attr = create_literal_attribute(10.0);
        stats.insert("base", StatInstance::new(StatType::Attribute(new_attr), ModifierStorage::default()));

        // Check that derived value updated
        assert_eq!(stats.get("base").unwrap(), 10.0);
        assert_eq!(stats.get("derived").unwrap(), 30.0);
    }

    #[test]
    fn test_circular_reference_safety() {
        let mut stats = StatCollection::new();

        // Create two stats that reference each other
        let attr1 = create_expression_attribute("b + 5");
        stats.insert("a", StatInstance::new(StatType::Attribute(attr1), ModifierStorage::default()));

        let attr2 = create_expression_attribute("a + 3");
        stats.insert("b", StatInstance::new(StatType::Attribute(attr2), ModifierStorage::default()));

        // This should not crash, even with circular references
        // Values might not be meaningful, but the system should be stable
        let a_val = stats.get("a").unwrap_or(-1.0);
        let b_val = stats.get("b").unwrap_or(-1.0);

        // Just verify we got some values without crashing
        assert!(a_val >= 0.0);
        assert!(b_val >= 0.0);
    }

    #[test]
    fn test_complex_expression() {
        let mut stats = StatCollection::new();

        // Set up base stats
        stats.insert("str", StatInstance::new(StatType::Attribute(create_literal_attribute(16.0)), ModifierStorage::default()));
        stats.insert("dex", StatInstance::new(StatType::Attribute(create_literal_attribute(14.0)), ModifierStorage::default()));
        stats.insert("con", StatInstance::new(StatType::Attribute(create_literal_attribute(12.0)), ModifierStorage::default()));

        // Create a more complex expression
        let formula = "floor((str + dex + con) / 6)";
        let attr = create_expression_attribute(formula);
        stats.insert("bonus", StatInstance::new(StatType::Attribute(attr), ModifierStorage::default()));

        // Check result
        assert_eq!(stats.get("bonus").unwrap(), 7.0); // (16 + 14 + 12) / 6 = 7
    }

    #[test]
    fn test_resource_stat() {
        let mut stats = StatCollection::new();

        // Create max health
        let max_health_attr = create_literal_attribute(100.0);
        stats.insert("max_health", StatInstance::new(StatType::Attribute(max_health_attr), ModifierStorage::default()));

        // Create health resource
        let mut resource = ResourceInstance {current: 80.0, bounds: ValueBounds::new(None, None)};
        resource.bounds.max = Some(ValueType::Expression(Expression::new(
            build_operator_tree("max_health").unwrap()
        )));

        stats.insert("health", StatInstance::new(StatType::Resource(resource), ModifierStorage::default()));

        // Check values
        assert_eq!(stats.get("max_health").unwrap(), 100.0);
        assert_eq!(stats.get("health").unwrap(), 80.0);
    }

    #[test]
    fn test_bounds_checking() {
        let mut stats = StatCollection::new();

        // Create a stat with bounds
        let mut stat_value = StatValue::from_f32(50.0);
        stat_value.set_bounds(Some(ValueBounds::new(
            Some(ValueType::Literal(0.0)),
            Some(ValueType::Literal(100.0))
        )));

        let attr = AttributeInstance::new(stat_value);
        stats.insert("bounded", StatInstance::new(StatType::Attribute(attr), ModifierStorage::default()));

        // Check value
        assert_eq!(stats.get("bounded").unwrap(), 50.0);

        // Update to below minimum
        let mut below_min = StatValue::from_f32(-10.0);
        below_min.set_bounds(Some(ValueBounds::new(
            Some(ValueType::Literal(0.0)),
            Some(ValueType::Literal(100.0))
        )));

        let attr_below = AttributeInstance::new(below_min);
        stats.insert("bounded", StatInstance::new(StatType::Attribute(attr_below), ModifierStorage::default()));

        // Should be clamped to minimum
        assert_eq!(stats.get("bounded").unwrap(), 0.0);

        // Update to above maximum
        let mut above_max = StatValue::from_f32(150.0);
        above_max.set_bounds(Some(ValueBounds::new(
            Some(ValueType::Literal(0.0)),
            Some(ValueType::Literal(100.0))
        )));

        let attr_above = AttributeInstance::new(above_max);
        stats.insert("bounded", StatInstance::new(StatType::Attribute(attr_above), ModifierStorage::default()));

        // Should be clamped to maximum
        assert_eq!(stats.get("bounded").unwrap(), 100.0);
    }

    #[test]
    fn test_dynamic_bounds() {
        let mut stats = StatCollection::new();

        // Create base stats
        stats.insert("min_bound", StatInstance::new(StatType::Attribute(create_literal_attribute(10.0)), ModifierStorage::default()));
        stats.insert("max_bound", StatInstance::new(StatType::Attribute(create_literal_attribute(30.0)), ModifierStorage::default()));

        // Create a stat with dynamic bounds
        let mut stat_value = StatValue::from_f32(20.0);
        stat_value.set_bounds(Some(ValueBounds::new(
            Some(ValueType::Expression(Expression::new(
                build_operator_tree("min_bound").unwrap()
            ))),
            Some(ValueType::Expression(Expression::new(
                build_operator_tree("max_bound").unwrap()
            )))
        )));

        let attr = AttributeInstance::new(stat_value);
        stats.insert("dynamic_bounded", StatInstance::new(StatType::Attribute(attr), ModifierStorage::default()));

        // Check initial value
        assert_eq!(stats.get("dynamic_bounded").unwrap(), 20.0);

        // Update bound
        stats.insert("min_bound", StatInstance::new(StatType::Attribute(create_literal_attribute(25.0)), ModifierStorage::default()));

        // Value should be updated to minimum
        // Note: This might need manual recalculation depending on your implementation
        stats.recalculate("dynamic_bounded");
        assert_eq!(stats.get("dynamic_bounded").unwrap(), 25.0);
    }

    #[test]
    fn test_pending_stats_resolution() {
        let mut stats = StatCollection::new();

        // Insert an expression that depends on a non-existent stat
        let expr_attr = create_expression_attribute("missing_stat + 5");
        stats.insert("dependent", StatInstance::new(StatType::Attribute(expr_attr), ModifierStorage::default()));

        // Check that it's in pending stats
        assert!(stats.get_hanging_attributes().contains_key("dependent"));

        // Now add the missing stat
        let base_attr = create_literal_attribute(10.0);
        stats.insert("missing_stat", StatInstance::new(StatType::Attribute(base_attr), ModifierStorage::default()));

        // Check that dependent is resolved and has the correct value
        assert!(!stats.get_hanging_attributes().contains_key("dependent"));
        assert_eq!(stats.get("dependent").unwrap(), 15.0);
    }

    #[test]
    fn test_multiple_dependencies() {
        let mut stats = StatCollection::new();

        // Create multiple base stats
        stats.insert("str", StatInstance::new(StatType::Attribute(create_literal_attribute(10.0)), ModifierStorage::default()));
        stats.insert("dex", StatInstance::new(StatType::Attribute(create_literal_attribute(12.0)), ModifierStorage::default()));
        stats.insert("con", StatInstance::new(StatType::Attribute(create_literal_attribute(14.0)), ModifierStorage::default()));

        // Create derived stats
        let attr1 = create_expression_attribute("str + dex");
        stats.insert("reflex", StatInstance::new(StatType::Attribute(attr1), ModifierStorage::default()));

        let attr2 = create_expression_attribute("str + con");
        stats.insert("fortitude", StatInstance::new(StatType::Attribute(attr2), ModifierStorage::default()));

        // Check values
        assert_eq!(stats.get("reflex").unwrap(), 22.0);
        assert_eq!(stats.get("fortitude").unwrap(), 24.0);

        // Update base stat and check both derived stats update
        stats.insert("str", StatInstance::new(StatType::Attribute(create_literal_attribute(20.0)), ModifierStorage::default()));

        assert_eq!(stats.get("reflex").unwrap(), 32.0);
        assert_eq!(stats.get("fortitude").unwrap(), 34.0);
    }

    #[test]
    fn test_recalculate_all() {
        let mut stats = StatCollection::new();

        // Create a chain of stats
        stats.insert("a", StatInstance::new(StatType::Attribute(create_literal_attribute(5.0)), ModifierStorage::default()));
        stats.insert("b", StatInstance::new(StatType::Attribute(create_expression_attribute("a * 2")), ModifierStorage::default()));
        stats.insert("c", StatInstance::new(StatType::Attribute(create_expression_attribute("b + 10")), ModifierStorage::default()));

        // Manually change a value without using insert
        // This simulates a direct change that wouldn't trigger recalculation
        if let Some(stat) = stats.stats.get_mut("a") {
            if let StatType::Attribute(ref mut attr) = stat.stat {
                attr.value_mut().set_value(10.0);
            }
        }

        // Values are stale without recalculation
        assert_eq!(stats.get("a").unwrap(), 10.0); // Updated directly
        assert_eq!(stats.get("b").unwrap(), 10.0); // Still using old value of a
        assert_eq!(stats.get("c").unwrap(), 20.0); // Still using old value of b

        // Recalculate all stats
        stats.recalculate_all();

        // Check updated values
        assert_eq!(stats.get("a").unwrap(), 10.0);
        assert_eq!(stats.get("b").unwrap(), 20.0); // Now updated
        assert_eq!(stats.get("c").unwrap(), 30.0); // Now updated
    }
}
