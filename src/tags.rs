use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct ValueTag {
    pub primary_value_target: String,
    pub groups: HashMap<String, HashSet<String>>
}
// expand to query language
// Add wildcards such as AnyOf