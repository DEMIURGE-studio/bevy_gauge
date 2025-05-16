use once_cell::sync::Lazy;
use std::sync::RwLock;
use std::collections::HashMap;
use super::prelude::*;

pub struct Konfig {
    stat_types: HashMap<String, String>,
    relationship_types: HashMap<String, ModType>,
    total_expressions: HashMap<String, String>,
}

impl Konfig {
    fn new() -> Self {
        Self {
            stat_types: HashMap::new(),
            relationship_types: HashMap::new(),
            total_expressions: HashMap::new(),
        }
    }

    pub fn get_stat_type(&self, path: &StatPath) -> &str {
        self.stat_types
            .get(path.name)
            .map(|s| s.as_str())
            .unwrap_or("Modifiable") // Default to Modifiable if not specified
    }

    pub fn get_relationship_type(&self, path: &StatPath) -> ModType {
        self.relationship_types
            .get(path.name)
            .unwrap_or(&ModType::Add)
            .clone()
    }
    
    pub fn get_total_expression(&self, path: &StatPath) -> &str {
        self.total_expressions
            .get(path.name)
            .map(|s| s.as_str())
            .unwrap_or("0")
    }

    pub fn register_stat_type(&mut self, stat: &str, stat_type: &str) {
        self.stat_types.insert(stat.to_string(), stat_type.to_string());
    }

    pub fn register_relationship_type(&mut self, stat: &str, relationship: ModType) {
        self.relationship_types.insert(stat.to_string(), relationship);
    }
    
    pub fn register_total_expression(&mut self, stat: &str, expression: &str) {
        self.total_expressions.insert(stat.to_string(), expression.to_string());
    }
}

pub static KONFIG: Lazy<RwLock<Konfig>> = Lazy::new(|| {
    RwLock::new(Konfig::new())
});

// Example usage (can be removed or moved to tests)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_konfig_access() {
        let konfig = KONFIG.read().unwrap();
        let stat_type = konfig.get_stat_type(&StatPath::parse("Damage"));
        assert_eq!(stat_type, "Modifiable");
    }
} 