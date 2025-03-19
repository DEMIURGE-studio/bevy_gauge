use std::collections::{HashMap, HashSet};
use crate::effects::{ModifierValue};


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TagValue {
    String(String),
    Multiple(Vec<String>),
    Any,
}

#[derive(Debug, Clone)]
pub struct TagQuery {
    category: String,
    attributes: HashMap<String, TagValue>,
}

#[derive(Debug)]
pub struct ModifierRegistry {
    modifiers: Vec<(TagQuery, ModifierValue)>,
}

impl TagQuery {
    pub fn new(category: &str) -> Self {
        Self {
            category: category.to_string(),
            attributes: HashMap::new(),
        }
    }

    pub fn with_attr(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), TagValue::String(value.to_string()));
        self
    }

    pub fn with_attrs(mut self, key: &str, values: Vec<&str>) -> Self {
        let values = values.into_iter().map(|s| s.to_string()).collect();
        self.attributes.insert(key.to_string(), TagValue::Multiple(values));
        self
    }

    pub fn with_any(mut self, key: &str) -> Self {
        self.attributes.insert(key.to_string(), TagValue::Any);
        self
    }

    pub fn matches(&self, other: &TagQuery) -> bool {
        // First check category match
        if self.category != other.category {
            return false;
        }

        // Check all attributes in the query match
        for (key, query_value) in &self.attributes {
            match other.attributes.get(key) {
                Some(target_value) => {
                    if !value_matches(query_value, target_value) {
                        return false;
                    }
                }
                None => {
                    // Target doesn't have this attribute
                    if !matches!(query_value, TagValue::Any) {
                        return false;
                    }
                }
            }
        }

        true
    }
}

fn value_matches(query: &TagValue, target: &TagValue) -> bool {
    match (query, target) {
        // Any matches anything
        (TagValue::Any, _) => true,

        // String matches only the same string
        (TagValue::String(q), TagValue::String(t)) => q == t,

        // Multiple matches if any value matches
        (TagValue::Multiple(values), TagValue::String(t)) => values.contains(t),

        // String matches if it's in the multiple set
        (TagValue::String(q), TagValue::Multiple(values)) => values.contains(q),

        // Multiple and Multiple match if they have any common elements
        (TagValue::Multiple(q_values), TagValue::Multiple(t_values)) => {
            q_values.iter().any(|q| t_values.contains(q))
        }

        // All other combinations don't match
        _ => false,
    }
}

impl ModifierRegistry {
    pub fn new() -> Self {
        Self {
            modifiers: Vec::new(),
        }
    }

    pub fn register(&mut self, query: TagQuery, modifier: ModifierValue) {
        self.modifiers.push((query, modifier));
    }

    pub fn query(&self, tag_query: &TagQuery) -> ModifierValue {
        let mut result = ModifierValue::default();
        let mut matched_indices = HashSet::new();

        for (idx, (query, modifier)) in self.modifiers.iter().enumerate() {
            if query.matches(tag_query) && matched_indices.insert(idx) {
                result.flat += modifier.flat;
                result.increased += modifier.increased;
                result.more *= modifier.more;
            }
        }

        result
    }
}
