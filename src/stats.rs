use crate::modifiers::ModifierValueTotal;
use crate::prelude::AttributeInstance;
use crate::resource::ResourceInstance;
use crate::tags::ValueTag;
use crate::value_type::{StatError, ValueType};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum StatInstance {
    Attribute(AttributeInstance),
    Resource(ResourceInstance),
    Intermediate(ModifierValueTotal),
}

impl StatInstance {
    pub fn get_value(&self, stat_collection: &StatCollection) -> f32 {
        match self {
            StatInstance::Attribute(attr) => {
                let val = attr.value.evaluate(stat_collection);
                val
            }
            StatInstance::Resource(resource) => resource.current,
            StatInstance::Intermediate(intermediate) => intermediate.get_total(),
        }
    }
}

#[derive(Component, Debug, Default, Clone, DerefMut, Deref)]
pub struct StatCollection {
    #[deref]
    pub stats: HashMap<ValueTag, StatInstance>,

    pub stat_dependencies: HashMap<ValueTag, HashSet<ValueTag>>,
    pub stat_dependents: HashMap<ValueTag, HashSet<ValueTag>>,

    pub pending_stats: HashMap<ValueTag, HashSet<ValueTag>>, // Key is attribute that is hanging, hashset is collection of missing attributes

    pub value_tag_cache: HashMap<ValueTag, Vec<(ValueTag, Entity)>>,
    pub value_tag_cache_dirty: bool,
}

impl StatCollection {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            stat_dependencies: Default::default(),
            stat_dependents: Default::default(),
            pending_stats: HashMap::new(),
            value_tag_cache: HashMap::new(),
            value_tag_cache_dirty: true,
        }
    }

    // Insert a attribute and update resolution order if needed

    pub fn insert(&mut self, tag: ValueTag, instance: StatInstance) {
        // If this is an expression, check if all dependencies are available
        match instance {
            StatInstance::Attribute(attribute) => {
                if let ValueType::Expression(expr) = &attribute.value {
                    let deps = expr.extract_dependencies();

                    // Identify missing dependencies
                    let missing_deps: HashSet<ValueTag> = deps
                        .iter()
                        .filter(|&dep| !self.stats.contains_key(dep))
                        .cloned()
                        .collect();


                    // If there are missing dependencies, add to pending attributes
                    if !missing_deps.is_empty() {
                        self.pending_stats.insert(tag.clone(), missing_deps);
                    }
                }

                self.stats.insert(tag, StatInstance::Attribute(attribute));

                self.resolve_pending_stats();
            }
            StatInstance::Resource(resource) => {
                self.stats.insert(tag.clone(), StatInstance::Resource(resource));
            }
            StatInstance::Intermediate(intermediate) => {
                self.stats.insert(tag.clone(), StatInstance::Intermediate(intermediate));
            }
        }

        // Mark cache as dirty since stats changed
        self.value_tag_cache_dirty = true;
    }

    // Resolve any pending attributes when a new attribute is added
    fn resolve_pending_stats(&mut self) {
        // Create a queue of stats that might be resolvable now
        let mut to_check: Vec<ValueTag> = Vec::new();

        // First pass: identify stats that might be resolvable now
        let pending_keys: Vec<ValueTag> = self.pending_stats.keys().cloned().collect();
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
            if let Some(deps) = self.pending_stats.remove(&tag) {
                // Store its dependencies for dependency tracking
                self.stat_dependencies.insert(tag.clone(), deps.clone());

                // Register this tag as a dependent of each of its dependencies
                for dep in deps {
                    self.stat_dependents
                        .entry(dep)
                        .or_default()
                        .insert(tag.clone());
                }

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

        // Handle any remaining pending stats (circular dependencies)
        if !self.pending_stats.is_empty() {
            for (tag, _) in self.pending_stats.drain() {
                if let Some(StatInstance::Attribute(_)) = self.stats.get(&tag) {
                    // Replace with a literal 0 value to break the cycle
                    let zero_attr = AttributeInstance::from_f32(0.0);
                    self.stats.insert(tag.clone(), StatInstance::Attribute(zero_attr));

                    // Remove this tag from dependency tracking
                    self.stat_dependencies.remove(&tag);
                }
            }
        }
    }

    // Add a batch of attributes at once with tree-walking resolution
    pub fn batch_insert(&mut self, attributes: Vec<(ValueTag, AttributeInstance)>) {
        // Insert non-expression attributes first (they have no dependencies)
        let mut expressions = Vec::new();

        for (tag, attr) in attributes {
            match &attr.value {
                ValueType::Literal(_) => {
                    // Literal values can be inserted immediately
                    self.insert(tag, StatInstance::Attribute(attr));
                },
                ValueType::Expression(_) => {
                    // Save expression attributes for dependency resolution
                    expressions.push((tag, attr));
                }
            }
        }

        // Now try to resolve expressions in passes until we make no more progress
        let mut remaining = expressions;
        let mut progress = true;

        while progress && !remaining.is_empty() {
            progress = false;
            let mut next_remaining = Vec::new();

            for (tag, attr) in remaining {
                if let ValueType::Expression(expr) = &attr.value {
                    let deps = expr.extract_dependencies();

                    // Check if all dependencies are available
                    let all_deps_available = deps.iter()
                        .all(|dep| self.stats.contains_key(dep));

                    if all_deps_available {
                        // All dependencies are available, insert this attribute
                        self.insert(tag, StatInstance::Attribute(attr));
                        progress = true;
                    } else {
                        // Some dependencies are still missing, keep for next pass
                        next_remaining.push((tag, attr));
                    }
                }
            }

            remaining = next_remaining;
        }

        // If we still have remaining expressions, they have circular dependencies
        // Just insert them anyway and let resolve_pending_stats handle them
        for (tag, attr) in remaining {
            self.insert(tag, StatInstance::Attribute(attr));
        }
    }

    // Recalculate a stat and its dependents using tree-walking
    pub fn recalculate(&mut self, tag: &ValueTag) {
        // Simple set to track which stats we've processed
        let mut processed = HashSet::new();

        // Start from the given tag and walk outward to dependents
        self.tree_walk_calculate(tag, &mut processed);
    }

    // Helper to walk the dependency tree and calculate values
    fn tree_walk_calculate(&mut self, start_tag: &ValueTag, processed: &mut HashSet<ValueTag>) {
        // Skip if already processed
        if processed.contains(start_tag) {
            return;
        }

        // Mark as processed to avoid cycles
        processed.insert(start_tag.clone());

        // Process this stat (force evaluation)
        let _ = self.get_str(start_tag);

        // Recursively process all dependents
        if let Some(dependents) = self.stat_dependents.get(start_tag).cloned() {
            for dependent in dependents {
                self.tree_walk_calculate(&dependent, processed);
            }
        }
    }

    // Recalculate all stats using tree-walking
    pub fn recalculate_all(&mut self) {
        // First, ensure all dependencies are properly tracked
        self.update_dependency_tracking();

        // Find all stats with no dependencies (roots of the tree)
        let root_stats: Vec<ValueTag> = self.stats.keys()
            .filter(|tag| {
                self.stat_dependencies.get(*tag).map_or(true, |deps| deps.is_empty())
            })
            .cloned()
            .collect();

        // Process each root stat first
        let mut processed = HashSet::new();
        for root in root_stats {
            self.tree_walk_calculate(&root, &mut processed);
        }

        // Then process any remaining stats that weren't reached
        let all_stats: Vec<ValueTag> = self.stats.keys().cloned().collect();
        for tag in all_stats {
            if !processed.contains(&tag) {
                self.tree_walk_calculate(&tag, &mut processed);
            }
        }
    }

    // Update dependency tracking info for all stats
    fn update_dependency_tracking(&mut self) {
        // Clear existing dependency tracking
        self.stat_dependencies.clear();
        self.stat_dependents.clear();

        // Rebuild dependency tracking
        for (tag, stat) in &self.stats {
            if let StatInstance::Attribute(attr) = stat {
                if let ValueType::Expression(expr) = &attr.value {
                    let deps = expr.extract_dependencies();

                    if !deps.is_empty() {
                        // Store dependencies
                        self.stat_dependencies.insert(tag.clone(), deps.clone());

                        // Update dependents
                        for dep in &deps {
                            self.stat_dependents
                                .entry(dep.clone())
                                .or_default()
                                .insert(tag.clone());
                        }
                    }
                }
            }
        }
    }

    // Get the value of a attribute by name, evaluating it if necessary
    pub fn get_str(&self, stat: &ValueTag) -> Result<f32, StatError> {
        match self.stats.get(stat) {
            Some(stat) => Ok(stat.get_value(self)),
            None => Err(StatError::NotFound(stat.stringify())),
        }
    }

    // Get the value of a attribute by name, evaluating it if necessary
    pub fn get<S: AsRef<str>>(&self, stat: S) -> Result<f32, StatError> {
        let parse_result = ValueTag::parse(stat.as_ref());
        if let Ok(parsed_tag) = parse_result {
            self.get_str(&parsed_tag)
        } else {
            Err(StatError::NotFound(stat.as_ref().to_string()))
        }
    }

    pub fn mark_dirty(&mut self) {
        self.value_tag_cache_dirty = true;
    }

    /// Check if the cache is dirty
    pub fn is_dirty(&self) -> bool {
        self.value_tag_cache_dirty
    }

    /// Get all modifier entities that qualify for a given tag
    pub fn get_qualifying_modifiers(&self, tag: &ValueTag) -> Option<&Vec<(ValueTag, Entity)>> {
        self.value_tag_cache.get(tag)
    }

    /// Rebuild the cache by finding all qualifying modifiers for each tag
    pub fn rebuild(
        &mut self,
        stat_tags: &[ValueTag],
        modifiers: &[(ValueTag, Entity)],
    ) {
        self.value_tag_cache.clear();

        // For each tag in the stat collection
        for tag in stat_tags {
            let mut qualifying_modifiers = Vec::new();

            // Find all modifiers that qualify for this tag
            for (modifier_tag, entity) in modifiers {
                if modifier_tag.qualifies_for(tag) {
                    qualifying_modifiers.push((modifier_tag.clone(), *entity));
                }
            }

            // Store the qualifying modifiers if any
            if !qualifying_modifiers.is_empty() {
                self.value_tag_cache.insert(tag.clone(), qualifying_modifiers);
            }
        }

        self.value_tag_cache_dirty = false;
    }

    // Get a list of hanging attributes with their missing dependencies
    pub fn get_hanging_attributes(&self) -> &HashMap<ValueTag, HashSet<ValueTag>> {
        &self.pending_stats
    }

}

#[cfg(test)]
mod stat_tests {
    use super::*;
    use crate::tags::{ValueTag, TagGroup};
    use crate::value_type::{ValueType, Expression};
    use evalexpr::build_operator_tree;

    // Helper to create a simple attribute
    fn create_attribute(value: f32) -> AttributeInstance {
        AttributeInstance::from_f32(value)
    }

    // Helper to create an expression attribute
    fn create_expression_attribute(expr_str: &str) -> AttributeInstance {
        let node = build_operator_tree(expr_str).unwrap();
        AttributeInstance::from_expression(Expression(node))
    }

    #[test]
    fn test_basic_stat_insertion_and_retrieval() {
        let mut stats = StatCollection::new();

        // Create and insert a simple stat
        let tag = ValueTag::parse("Health").unwrap();
        let attr = create_attribute(100.0);
        stats.insert(tag.clone(), StatInstance::Attribute(attr));

        // Retrieve and check the value
        assert_eq!(stats.get_str(&tag).unwrap(), 100.0);
        assert_eq!(stats.get("Health").unwrap(), 100.0);

        // Check a non-existent stat
        assert!(stats.get("Nonexistent").is_err());
    }

    #[test]
    fn test_tag_with_groups() {
        let mut stats = StatCollection::new();

        // Create a tag with groups
        let fire_tag = ValueTag::parse("Damage(elemental[\"fire\"])").unwrap();
        let ice_tag = ValueTag::parse("Damage(elemental[\"ice\"])").unwrap();

        // Insert stats with these tags
        stats.insert(fire_tag.clone(), StatInstance::Attribute(create_attribute(30.0)));
        stats.insert(ice_tag.clone(), StatInstance::Attribute(create_attribute(25.0)));

        // Retrieve and check values
        assert_eq!(stats.get_str(&fire_tag).unwrap(), 30.0);
        assert_eq!(stats.get_str(&ice_tag).unwrap(), 25.0);

        // Check using string form
        assert_eq!(stats.get("Damage(elemental[\"fire\"])").unwrap(), 30.0);
        assert_eq!(stats.get("Damage(elemental[\"ice\"])").unwrap(), 25.0);
    }

    #[test]
    fn test_simple_dependency_resolution() {
        let mut stats = StatCollection::new();

        // Add a base stat first
        let base_tag = ValueTag::parse("Strength").unwrap();
        stats.insert(base_tag.clone(), StatInstance::Attribute(create_attribute(10.0)));

        // Add a stat that depends on the base stat
        let derived_tag = ValueTag::parse("DamageBonus").unwrap();
        let expr = create_expression_attribute("Strength * 0.5");
        stats.insert(derived_tag.clone(), StatInstance::Attribute(expr));

        // Check the derived stat's value
        assert_eq!(stats.get_str(&derived_tag).unwrap(), 5.0);

        // Update the base stat
        stats.insert(base_tag.clone(), StatInstance::Attribute(create_attribute(20.0)));

        // Check the derived stat's value again - should be updated
        assert_eq!(stats.get_str(&derived_tag).unwrap(), 10.0);
    }

    #[test]
    fn test_pending_stat_resolution() {
        let mut stats = StatCollection::new();

        // Add a derived stat first (with dependency not yet met)
        let derived_tag = ValueTag::parse("Attack").unwrap();
        let expr = create_expression_attribute("BaseAttack + StrengthBonus");
        stats.insert(derived_tag.clone(), StatInstance::Attribute(expr));

        // Check that it's in the pending stats
        assert!(stats.pending_stats.contains_key(&derived_tag));

        // Add the first dependency
        let base_attack_tag = ValueTag::parse("BaseAttack").unwrap();
        stats.insert(base_attack_tag.clone(), StatInstance::Attribute(create_attribute(20.0)));

        // Should still be pending
        assert!(stats.pending_stats.contains_key(&derived_tag));

        // Add the second dependency
        let str_bonus_tag = ValueTag::parse("StrengthBonus").unwrap();
        stats.insert(str_bonus_tag.clone(), StatInstance::Attribute(create_attribute(5.0)));

        // Now it should be resolved
        assert!(!stats.pending_stats.contains_key(&derived_tag));

        // Check the value
        assert_eq!(stats.get_str(&derived_tag).unwrap(), 25.0);
    }

    #[test]
    fn test_batch_insertion() {
        let mut stats = StatCollection::new();

        // Create a batch of attributes with dependencies
        let attributes = vec![
            (ValueTag::parse("BaseAttack").unwrap(), create_attribute(20.0)),
            (ValueTag::parse("Strength").unwrap(), create_attribute(10.0)),
            (ValueTag::parse("StrengthBonus").unwrap(), create_expression_attribute("Strength * 0.5")),
            (ValueTag::parse("Attack").unwrap(), create_expression_attribute("BaseAttack + StrengthBonus")),
        ];

        // Insert them all at once
        stats.batch_insert(attributes);

        // Check that all dependencies were properly resolved
        assert!(stats.pending_stats.is_empty());

        // Check the computed values
        assert_eq!(stats.get("BaseAttack").unwrap(), 20.0);
        assert_eq!(stats.get("Strength").unwrap(), 10.0);
        assert_eq!(stats.get("StrengthBonus").unwrap(), 5.0);
        assert_eq!(stats.get("Attack").unwrap(), 25.0);
    }

    #[test]
    fn test_recalculation() {
        let mut stats = StatCollection::new();

        // Create a chain of dependent stats
        stats.insert(ValueTag::parse("Base").unwrap(), StatInstance::Attribute(create_attribute(10.0)));
        stats.insert(ValueTag::parse("Level1").unwrap(), StatInstance::Attribute(create_expression_attribute("Base * 2")));
        stats.insert(ValueTag::parse("Level2").unwrap(), StatInstance::Attribute(create_expression_attribute("Level1 + 5")));
        stats.insert(ValueTag::parse("Level3").unwrap(), StatInstance::Attribute(create_expression_attribute("Level2 * 1.5")));

        // Check initial values
        assert_eq!(stats.get("Base").unwrap(), 10.0);
        assert_eq!(stats.get("Level1").unwrap(), 20.0);
        assert_eq!(stats.get("Level2").unwrap(), 25.0);
        assert_eq!(stats.get("Level3").unwrap(), 37.5);

        // Update the base value
        stats.insert(ValueTag::parse("Base").unwrap(), StatInstance::Attribute(create_attribute(20.0)));

        // Recalculate the chain
        stats.recalculate(&ValueTag::parse("Base").unwrap());

        // Check updated values
        assert_eq!(stats.get("Base").unwrap(), 20.0);
        assert_eq!(stats.get("Level1").unwrap(), 40.0);
        assert_eq!(stats.get("Level2").unwrap(), 45.0);
        assert_eq!(stats.get("Level3").unwrap(), 67.5);
    }

    #[test]
    fn test_recalculate_all() {
        let mut stats = StatCollection::new();

        // Create multiple dependency chains
        stats.insert(ValueTag::parse("Strength").unwrap(), StatInstance::Attribute(create_attribute(10.0)));
        stats.insert(ValueTag::parse("Dexterity").unwrap(), StatInstance::Attribute(create_attribute(15.0)));

        stats.insert(ValueTag::parse("StrBonus").unwrap(), StatInstance::Attribute(create_expression_attribute("Strength * 0.5")));
        stats.insert(ValueTag::parse("DexBonus").unwrap(), StatInstance::Attribute(create_expression_attribute("Dexterity * 0.3")));

        stats.insert(ValueTag::parse("Attack").unwrap(), StatInstance::Attribute(create_expression_attribute("StrBonus * 2 + 10")));
        stats.insert(ValueTag::parse("Defense").unwrap(), StatInstance::Attribute(create_expression_attribute("DexBonus * 3 + 5")));

        // Check initial values
        assert_eq!(stats.get("Attack").unwrap(), 20.0); // 10 * 0.5 * 2 + 10 = 20
        assert_eq!(stats.get("Defense").unwrap(), 18.5); // 15 * 0.3 * 3 + 5 = 18.5

        // Update base stats
        stats.insert(ValueTag::parse("Strength").unwrap(), StatInstance::Attribute(create_attribute(20.0)));
        stats.insert(ValueTag::parse("Dexterity").unwrap(), StatInstance::Attribute(create_attribute(25.0)));

        // Recalculate everything
        stats.recalculate_all();

        // Check updated values
        assert_eq!(stats.get("Attack").unwrap(), 30.0); // 20 * 0.5 * 2 + 10 = 30
        assert_eq!(stats.get("Defense").unwrap(), 27.5); // 25 * 0.3 * 3 + 5 = 27.5
    }

    #[test]
    fn test_circular_dependencies() {
        let mut stats = StatCollection::new();

        // Create a circular dependency between A, B, and C
        // A depends on B, B depends on C, C depends on A
        stats.insert(ValueTag::parse("A").unwrap(), StatInstance::Attribute(create_expression_attribute("B + 5")));
        stats.insert(ValueTag::parse("B").unwrap(), StatInstance::Attribute(create_expression_attribute("C * 2")));
        stats.insert(ValueTag::parse("C").unwrap(), StatInstance::Attribute(create_expression_attribute("A / 2")));

        // With our tree-walking approach, one of these should be set to 0
        // Check if all values are calculated
        let a_value = stats.get("A").unwrap_or(999.0); // Use a default that would be obvious if not set
        let b_value = stats.get("B").unwrap_or(999.0);
        let c_value = stats.get("C").unwrap_or(999.0);

        // Verify that we've handled the circular dependency by checking:
        // 1. At least one value should be 0 (to break the cycle)
        // 2. The other values should be calculated based on that
        if a_value == 0.0 {
            assert_eq!(b_value, 0.0); // B = C * 2, and C would be 0 
            assert_eq!(c_value, 0.0); // C = A / 2 = 0
        } else if b_value == 0.0 {
            assert_eq!(a_value, 5.0); // A = B + 5 = 0 + 5
            assert_eq!(c_value, 2.5); // C = A / 2 = 5 / 2
        } else if c_value == 0.0 {
            assert_eq!(b_value, 0.0); // B = C * 2 = 0
            assert_eq!(a_value, 5.0); // A = B + 5 = 0 + 5
        } else {
            // All three can't have non-zero values in a true circular dependency
            assert!(false, "Circular dependency not properly handled");
        }

        // Now let's break the circular dependency by setting a concrete value
        stats.insert(ValueTag::parse("A").unwrap(), StatInstance::Attribute(create_attribute(10.0)));

        // Recalculate
        stats.recalculate_all();

        // Check values - now they should be deterministic
        assert_eq!(stats.get("A").unwrap(), 10.0);
        assert_eq!(stats.get("C").unwrap(), 5.0);   // C = A / 2 = 10 / 2
        assert_eq!(stats.get("B").unwrap(), 10.0);  // B = C * 2 = 5 * 2
    }

    #[test]
    fn test_tag_qualification_caching() {
        let mut stats = StatCollection::new();

        // Add some stats with tags
        let fire_tag = ValueTag::parse("Damage(elemental[\"fire\"])").unwrap();
        let ice_tag = ValueTag::parse("Damage(elemental[\"ice\"])").unwrap();

        stats.insert(fire_tag.clone(), StatInstance::Attribute(create_attribute(30.0)));
        stats.insert(ice_tag.clone(), StatInstance::Attribute(create_attribute(25.0)));

        // Set up some modifier tags and entities
        let generic_mod_tag = ValueTag::parse("Damage").unwrap();
        let fire_mod_tag = ValueTag::parse("Damage(elemental[\"fire\"])").unwrap();
        let ice_mod_tag = ValueTag::parse("Damage(elemental[\"ice\"])").unwrap();
        let wrong_tag = ValueTag::parse("Health").unwrap();

        let modifiers = vec![
            (generic_mod_tag.clone(), Entity::from_raw(1)),
            (fire_mod_tag.clone(), Entity::from_raw(2)),
            (ice_mod_tag.clone(), Entity::from_raw(3)),
            (wrong_tag.clone(), Entity::from_raw(4)),
        ];

        // Rebuild the cache
        stats.rebuild(&[fire_tag.clone(), ice_tag.clone()], &modifiers);

        // Check that the cache contains the right modifiers for each tag
        let fire_mods = stats.get_qualifying_modifiers(&fire_tag).unwrap();
        let ice_mods = stats.get_qualifying_modifiers(&ice_tag).unwrap();

        // For fire damage, both generic and fire mods should apply
        assert_eq!(fire_mods.len(), 2);
        assert!(fire_mods.iter().any(|(tag, _)| tag == &generic_mod_tag));
        assert!(fire_mods.iter().any(|(tag, _)| tag == &fire_mod_tag));
        assert!(!fire_mods.iter().any(|(tag, _)| tag == &ice_mod_tag));

        // For ice damage, both generic and ice mods should apply
        assert_eq!(ice_mods.len(), 2);
        assert!(ice_mods.iter().any(|(tag, _)| tag == &generic_mod_tag));
        assert!(!ice_mods.iter().any(|(tag, _)| tag == &fire_mod_tag));
        assert!(ice_mods.iter().any(|(tag, _)| tag == &ice_mod_tag));

        // Test marking the cache dirty and rebuilding
        stats.mark_dirty();
        assert!(stats.is_dirty());

        // Remove some modifiers
        let reduced_modifiers = vec![
            (generic_mod_tag.clone(), Entity::from_raw(1)),
            (fire_mod_tag.clone(), Entity::from_raw(2)),
        ];

        // Rebuild the cache
        stats.rebuild(&[fire_tag.clone(), ice_tag.clone()], &reduced_modifiers);
        assert!(!stats.is_dirty());

        // Check the updated cache
        let fire_mods = stats.get_qualifying_modifiers(&fire_tag).unwrap();
        let ice_mods = stats.get_qualifying_modifiers(&ice_tag).unwrap();

        // Fire should still have both modifiers
        assert_eq!(fire_mods.len(), 2);

        // Ice should now only have the generic modifier
        assert_eq!(ice_mods.len(), 1);
        assert!(ice_mods.iter().any(|(tag, _)| tag == &generic_mod_tag));
    }

    #[test]
    fn test_complex_dependency_tree() {
        let mut stats = StatCollection::new();

        // Create a complex dependency tree
        stats.insert(ValueTag::parse("BaseDamage").unwrap(), StatInstance::Attribute(create_attribute(10.0)));
        stats.insert(ValueTag::parse("BaseStrength").unwrap(), StatInstance::Attribute(create_attribute(20.0)));
        stats.insert(ValueTag::parse("BaseCritical").unwrap(), StatInstance::Attribute(create_attribute(5.0)));

        stats.insert(ValueTag::parse("StrengthBonus").unwrap(), StatInstance::Attribute(create_expression_attribute("BaseStrength * 0.1")));
        stats.insert(ValueTag::parse("DamageMultiplier").unwrap(), StatInstance::Attribute(create_expression_attribute("1 + StrengthBonus")));
        stats.insert(ValueTag::parse("CriticalChance").unwrap(), StatInstance::Attribute(create_expression_attribute("BaseCritical + StrengthBonus * 0.5")));
        stats.insert(ValueTag::parse("CriticalMultiplier").unwrap(), StatInstance::Attribute(create_expression_attribute("1.5 + BaseCritical * 0.1")));

        stats.insert(ValueTag::parse("DamageBase").unwrap(), StatInstance::Attribute(create_expression_attribute("BaseDamage * DamageMultiplier")));
        stats.insert(ValueTag::parse("DamageCritical").unwrap(), StatInstance::Attribute(create_expression_attribute("DamageBase * CriticalMultiplier * CriticalChance * 0.01")));
        stats.insert(ValueTag::parse("TotalDamage").unwrap(), StatInstance::Attribute(create_expression_attribute("DamageBase + DamageCritical")));

        // Check initial values
        let total_damage = stats.get("TotalDamage").unwrap();
        assert!(total_damage > 0.0); // Just make sure it calculated something reasonable

        // Now modify a base stat
        stats.insert(ValueTag::parse("BaseStrength").unwrap(), StatInstance::Attribute(create_attribute(40.0)));

        // Recalculate just that stat
        stats.recalculate(&ValueTag::parse("BaseStrength").unwrap());

        // Ensure the change propagated through the tree
        let new_total_damage = stats.get("TotalDamage").unwrap();
        assert!(new_total_damage > total_damage); // Damage should increase with strength
    }
}
