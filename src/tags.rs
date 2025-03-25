use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use crate::modifiers::ModifierInstance;
use crate::stats::StatCollection;

/// Represents a group of values within a tag
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagGroup {
    /// Represents all possible values in this group
    All,
    /// Represents a specific set of values
    AnyOf(HashSet<String>),
}

impl TagGroup {
    pub fn to_any_of(&self) -> Option<&HashSet<String>> {
        match self {
            TagGroup::AnyOf(set) => Some(set),
            _ => None,
        }
    }

    pub fn is_all(&self) -> bool {
        matches!(self, TagGroup::All)
    }

    /// Check if this tag group is compatible with another tag group
    pub fn is_compatible_with(&self, other: &TagGroup) -> bool {
        match (self, other) {
            // Both All - automatic match
            (TagGroup::All, TagGroup::All) => true,

            // Self is All - matches any specific values in other
            (TagGroup::All, TagGroup::AnyOf(_)) => true,

            // Other is All - matches any specific values in self
            (TagGroup::AnyOf(_), TagGroup::All) => true,

            // Both have specific values - need at least one value in common
            (TagGroup::AnyOf(self_values), TagGroup::AnyOf(other_values)) => {
                self_values.intersection(other_values).next().is_some()
            }
        }
    }
}

/// A tag that identifies a specific value or attribute
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ValueTag {
    /// The primary value this tag targets (e.g., "Damage")
    pub primary_value_target: String,

    /// Optional groups that qualify this tag (e.g., elemental type, weapon type)
    pub groups: Option<HashMap<String, TagGroup>>,

    /// Cache of string representation for efficient comparison
    cached_string: Option<String>,

    /// Cache of hash value for efficient comparison
    cached_hash: Option<u64>,
}

impl ValueTag {
    /// Create a new tag with a primary target and optional groups
    pub fn new(primary_value_target: String, groups: Option<HashMap<String, TagGroup>>) -> Self {
        ValueTag {
            primary_value_target,
            groups,
            cached_string: None,
            cached_hash: None,
        }
    }

    /// Add a group that represents all values of a particular type
    pub fn add_all_group(&mut self, name: String) -> &mut Self {
        // Clear caches since we're modifying the tag
        self.cached_string = None;
        self.cached_hash = None;

        if let Some(ref mut groups) = self.groups {
            groups.insert(name, TagGroup::All);
        } else {
            let mut groups = HashMap::new();
            groups.insert(name, TagGroup::All);
            self.groups = Some(groups);
        }

        self
    }

    /// Add a group with specific allowed values
    pub fn add_any_of_group(&mut self, name: String, values: HashSet<String>) -> &mut Self {
        // Clear caches since we're modifying the tag
        self.cached_string = None;
        self.cached_hash = None;

        if let Some(ref mut groups) = self.groups {
            groups.insert(name, TagGroup::AnyOf(values));
        } else {
            let mut groups = HashMap::new();
            groups.insert(name, TagGroup::AnyOf(values));
            self.groups = Some(groups);
        }

        self
    }

    /// Convert the tag to a string representation
    pub fn stringify(&self) -> String {
        // Use cached version if available
        if let Some(ref cached) = self.cached_string {
            return cached.clone();
        }

        // Otherwise, calculate the string
        let mut result = format!("{}", self.primary_value_target);

        // Sort the groups by key for deterministic output
        if let Some(ref groups) = self.groups {
            result.push_str("(");
            let mut sorted_groups: Vec<(&String, &TagGroup)> = groups.iter().collect();
            sorted_groups.sort_by(|a, b| a.0.cmp(b.0));

            let mut first = true;
            for (group_name, group) in sorted_groups {
                if !first {
                    result.push_str(" ");  // Add space between groups
                }
                first = false;
                match group {
                    TagGroup::All => {
                        result.push_str(&format!("{}", group_name));
                    }
                    TagGroup::AnyOf(values) => {
                        // Sort values for deterministic output
                        let mut sorted_values: Vec<&String> = values.iter().collect();
                        sorted_values.sort();

                        let values_str = sorted_values.iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>()
                            .join(" ");

                        result.push_str(&format!("{}[\"{}\"]", group_name, values_str));
                    }
                }
            }
            result.push_str(")");
        }

        // Cache the result
        let result_clone = result.clone();
        let mut mutable_self = self.clone();
        mutable_self.cached_string = Some(result_clone);

        result
    }

    /// Parse a tag from a string representation
    pub fn parse(s: &str) -> Result<Self, String> {
        // Find the primary_value_target (everything up to the first '(' or the entire string)
        let primary_end = s.find('(').unwrap_or(s.len());
        let primary_value_target = s[..primary_end].to_string();

        let mut groups = HashMap::new();

        // If we have groups, parse them
        if primary_end < s.len() {
            // Ensure the string ends with ')'
            if !s.ends_with(')') {
                return Err("Missing closing parenthesis for groups".to_string());
            }

            // Extract the groups part (remove the outer parentheses)
            let groups_str = &s[primary_end + 1..s.len() - 1];

            // Split the groups_str into individual group definitions
            let mut current_pos = 0;
            let mut group_start = 0;
            let mut in_bracket = false;
            let mut in_quotes = false;

            while current_pos < groups_str.len() {
                let c = groups_str.chars().nth(current_pos).unwrap();

                match c {
                    '[' => in_bracket = true,
                    ']' => in_bracket = false,
                    '"' => in_quotes = !in_quotes,
                    ' ' if !in_bracket && !in_quotes => {
                        // We found a space separator between groups
                        if current_pos > group_start {
                            let group_def = &groups_str[group_start..current_pos];
                            parse_group(group_def, &mut groups)?;
                            group_start = current_pos + 1;
                        } else {
                            group_start = current_pos + 1;
                        }
                    },
                    _ => {}
                }

                current_pos += 1;
            }

            // Process the last group
            if group_start < groups_str.len() {
                let group_def = &groups_str[group_start..];
                parse_group(group_def, &mut groups)?;
            }
        }

        // Create a new optimized tag with cache
        let groups_option = if groups.is_empty() { None } else { Some(groups) };
        let mut tag = ValueTag {
            primary_value_target,
            groups: groups_option,
            cached_string: Some(s.to_string()),
            cached_hash: None,
        };

        // Pre-compute the hash
        tag.compute_hash();

        Ok(tag)
    }

    // Helper method to compute and cache the hash value
    fn compute_hash(&mut self) -> u64 {
        // If we have a cached hash, return it
        if let Some(hash) = self.cached_hash {
            return hash;
        }

        // Make sure we have a cached string
        if self.cached_string.is_none() {
            self.cached_string = Some(self.stringify());
        }

        // Compute the hash from the string
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        // We can unwrap safely because we just made sure the cached_string is Some
        self.cached_string.as_ref().unwrap().hash(&mut hasher);
        let hash = hasher.finish();

        // Cache and return the hash
        self.cached_hash = Some(hash);
        hash
    }

    /// Check if this tag qualifies to affect another tag
    /// Returns true if this tag qualifies to affect `target_tag`
    pub fn qualifies_for(&self, target_tag: &ValueTag) -> bool {
        // Primary value must match exactly
        if self.primary_value_target != target_tag.primary_value_target {
            return false;
        }

        // A tag with no groups is a "universal" tag that applies to anything
        // with the same primary value
        if self.groups.is_none() {
            return true;
        }

        // If we have groups but target doesn't, we're too specific
        if target_tag.groups.is_none() {
            return false;
        }

        let our_groups = self.groups.as_ref().unwrap();
        let target_groups = target_tag.groups.as_ref().unwrap();

        // For each of our groups, check compatibility with the target
        for (group_name, our_group) in our_groups {
            // If target doesn't have this group at all, no match
            if !target_groups.contains_key(group_name) {
                // In your requirements, you mentioned that a modifier without weapon_size
                // would still qualify for a target with weapon_size["one_handed"],
                // so we allow missing groups in the target
                continue;
            }

            let target_group = target_groups.get(group_name).unwrap();

            // Check if our group is compatible with the target's group
            if !our_group.is_compatible_with(target_group) {
                return false;
            }
        }

        // All checks passed, this tag qualifies
        true
    }

    /// Get all values for a specific group
    pub fn get_group_values(&self, group_name: &str) -> Option<HashSet<String>> {
        if let Some(groups) = &self.groups {
            if let Some(group) = groups.get(group_name) {
                match group {
                    TagGroup::All => Some(HashSet::new()), // Empty set means "all"
                    TagGroup::AnyOf(values) => Some(values.clone()),
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if this tag has a specific value in a group
    pub fn has_value(&self, group_name: &str, value: &str) -> bool {
        if let Some(values) = self.get_group_values(group_name) {
            if values.is_empty() {
                // Empty set means All - matches anything
                true
            } else {
                values.contains(value)
            }
        } else {
            false
        }
    }
}

impl Hash for ValueTag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // If we have a cached hash, use it
        if let Some(hash) = self.cached_hash {
            state.write_u64(hash);
            return;
        }

        // Otherwise, make sure we have a cached string
        let string = if let Some(ref s) = self.cached_string {
            s
        } else {
            // For &self methods, we can't modify self directly
            // But we can create a temporary that does the same hash operation
            let string = self.stringify();
            string.hash(state);
            return;
        };

        // Hash the string
        string.hash(state);
    }
}

// Helper function to parse a single group definition
fn parse_group(group_def: &str, groups: &mut HashMap<String, TagGroup>) -> Result<(), String> {
    // Check if it's an All group or AnyOf group
    if let Some(bracket_pos) = group_def.find('[') {
        // AnyOf group
        let group_name = group_def[..bracket_pos].to_string();

        // Extract values from quotes
        let values_start = group_def[bracket_pos..].find('"')
            .ok_or("Missing values opening quote")? + bracket_pos;
        let values_end = group_def[values_start + 1..].rfind('"')
            .ok_or("Missing values closing quote")? + values_start + 1;

        let values_str = &group_def[values_start + 1..values_end];
        let values: HashSet<String> = values_str.split_whitespace()
            .map(String::from)
            .collect();

        groups.insert(group_name, TagGroup::AnyOf(values));
    } else {
        // All group
        let group_name = group_def.to_string();
        groups.insert(group_name, TagGroup::All);
    }

    Ok(())
}

// System to update tag qualification caches
pub fn update_tag_caches(
    mut query: Query<(Entity, &mut StatCollection)>,
    all_modifiers: Query<(Entity, &ModifierInstance)>,
) {
    for (entity, mut stat_collection) in query.iter_mut() {
        if !stat_collection.is_dirty() {
            continue;
        }

        // Extract all stat tags
        let stat_tags: Vec<ValueTag> = stat_collection.stats.keys().cloned().collect();

        // Get all modifiers and their tags
        let modifiers: Vec<(ValueTag, Entity)> = all_modifiers.iter()
            .filter_map(|(modifier_entity, modifier)| {
                // Only include modifiers targeting this entity
                if let Some(target_context) = &modifier.target_context {
                    if target_context.entity == Some(entity) {
                        return Some((modifier.definition.tag.clone(), modifier_entity));
                    }
                }
                None
            })
            .collect();

        // Rebuild the cache
        stat_collection.rebuild(&stat_tags, &modifiers);
    }
}

// Event sent when modifiers change and caches need updating
#[derive(Event)]
pub struct ModifiersChangedEvent {
    pub target_entity: Entity,
}

// System to handle modifier change events
pub fn handle_modifier_events(
    mut events: EventReader<ModifiersChangedEvent>,
    mut stat_collections: Query<(Entity, &mut StatCollection)>,
) {
    for event in events.read() {
        for (entity, mut stat_collection) in stat_collections.iter_mut() {
            if entity == event.target_entity {
                stat_collection.mark_dirty();
            }
        }
    }
}

// Plugin to add all tag system components
pub fn tag_system_plugin(app: &mut App) {
    app
        .add_event::<ModifiersChangedEvent>()
        .add_systems(Update, update_tag_caches)
        .add_systems(Update, handle_modifier_events);
}

#[cfg(test)]
mod tag_tests {
    use super::*;

    #[test]
    fn test_tag_parsing() {
        // Test basic tag
        let tag1 = ValueTag::parse("Damage").unwrap();
        assert_eq!(tag1.primary_value_target, "Damage");
        assert!(tag1.groups.is_none());

        // Test tag with groups
        let tag2 = ValueTag::parse("Damage(elemental[\"fire\"] weapon_type[\"sword\"])").unwrap();
        assert_eq!(tag2.primary_value_target, "Damage");
        assert!(tag2.groups.is_some());

        let groups = tag2.groups.unwrap();
        assert_eq!(groups.len(), 2);
        assert!(groups.contains_key("elemental"));
        assert!(groups.contains_key("weapon_type"));

        if let TagGroup::AnyOf(values) = &groups["elemental"] {
            assert_eq!(values.len(), 1);
            assert!(values.contains("fire"));
        } else {
            panic!("Expected AnyOf group");
        }

        if let TagGroup::AnyOf(values) = &groups["weapon_type"] {
            assert_eq!(values.len(), 1);
            assert!(values.contains("sword"));
        } else {
            panic!("Expected AnyOf group");
        }

        // Test tag with multiple values in a group
        let tag3 = ValueTag::parse("Damage(weapon_type[\"sword axe\"])").unwrap();
        let groups = tag3.groups.unwrap();

        if let TagGroup::AnyOf(values) = &groups["weapon_type"] {
            assert_eq!(values.len(), 2);
            assert!(values.contains("sword"));
            assert!(values.contains("axe"));
        } else {
            panic!("Expected AnyOf group");
        }

        // Test tag with All group
        let tag4 = ValueTag::parse("Damage(elemental)").unwrap();
        let groups = tag4.groups.unwrap();

        assert!(matches!(groups["elemental"], TagGroup::All));
    }

    #[test]
    fn test_tag_qualification() {
        // Create a fire damage tag
        let fire_tag = ValueTag::parse("Damage(elemental[\"fire\"])").unwrap();

        // Create various modifier tags
        let fire_mod = ValueTag::parse("Damage(elemental[\"fire\"])").unwrap();
        let ice_mod = ValueTag::parse("Damage(elemental[\"ice\"])").unwrap();
        let elemental_mod = ValueTag::parse("Damage(elemental)").unwrap();
        let all_damage_mod = ValueTag::parse("Damage").unwrap();

        // Test qualification logic
        assert!(fire_mod.qualifies_for(&fire_tag)); // Fire qualifies for fire
        assert!(!ice_mod.qualifies_for(&fire_tag)); // Ice doesn't qualify for fire
        assert!(elemental_mod.qualifies_for(&fire_tag)); // General elemental qualifies for fire
        assert!(all_damage_mod.qualifies_for(&fire_tag)); // All damage qualifies for fire

        // Test complex cases
        let complex_tag = ValueTag::parse("Damage(elemental[\"fire\"] ability_type[\"attack\"] weapon_type[\"axe sword\"])").unwrap();

        // Create various complex modifiers
        let exact_mod = ValueTag::parse("Damage(elemental[\"fire\"] ability_type[\"attack\"] weapon_size[\"one_handed\"] weapon_type[\"axe\"])").unwrap();
        let wrong_element_mod = ValueTag::parse("Damage(elemental[\"ice\"] ability_type[\"attack\"] weapon_size[\"one_handed\"] weapon_type[\"axe\"])").unwrap();
        let missing_size_mod = ValueTag::parse("Damage(elemental[\"fire\"] ability_type[\"attack\"] weapon_type[\"axe\"])").unwrap();
        let another_mod = ValueTag::parse("Damage(elemental[\"fire\"] ability_type[\"spell\"])").unwrap();
        let generic_element_mod = ValueTag::parse("Damage(elemental ability_type[\"attack\"] weapon_size[\"one_handed\"] weapon_type[\"axe\"])").unwrap();
        
        
        

        // Test qualification logic
        assert!(exact_mod.qualifies_for(&complex_tag)); // Exact match qualifies
        assert!(!wrong_element_mod.qualifies_for(&complex_tag)); // Wrong element doesn't qualify
        assert!(missing_size_mod.qualifies_for(&complex_tag)); // Missing size still qualifies
        assert!(!another_mod.qualifies_for(&complex_tag));
        assert!(generic_element_mod.qualifies_for(&complex_tag)); // Generic element qualifies
    }
    
}
