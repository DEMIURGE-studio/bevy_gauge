use bevy::prelude::*;
use std::collections::HashMap;
use super::prelude::*;

#[derive(Resource, Default)]
pub struct Config {
    stat_types: HashMap<String, String>,
    relationship_types: HashMap<String, ModType>,
    total_expressions: HashMap<String, String>,
}

impl Config {
    /// Register a stat type for a given stat path
    pub fn register_stat_type(&mut self, stat: &str, stat_type: &str) {
        self.stat_types.insert(stat.to_string(), stat_type.to_string());
    }

    /// Register a relationship type (Add/Mul) for a stat path
    pub fn register_relationship_type(&mut self, stat: &str, relationship: ModType) {
        self.relationship_types.insert(stat.to_string(), relationship);
    }

    /// Register the total expression for a stat path
    pub fn register_total_expression(&mut self, stat: &str, expression: &str) {
        self.total_expressions.insert(stat.to_string(), expression.to_string());
    }

    /// Get the stat type for a given path
    pub(crate) fn get_stat_type(&self, path: &StatPath) -> &str {
        self.stat_types
            .get(path.name)
            .map(|s| s.as_str())
            .unwrap_or("Flat") // Default to Flat if not specified
    }
    
    /// Get the relationship type for a given path
    pub(crate) fn get_relationship_type(&self, path: &StatPath) -> ModType {
        // For parts of stats, check if we have a specific relationship type
        if let Some(part) = path.part {
            let key = format!("{}.{}", path.name, part);
            if let Some(rel_type) = self.relationship_types.get(&key) {
                return rel_type.clone();
            }
        }

        // Check for the base stat
        if let Some(rel_type) = self.relationship_types.get(path.name) {
            return rel_type.clone();
        }

        // Default relationship types based on part name
        if let Some(part) = path.part {
            match part {
                "increased" | "reduced" | "added" => ModType::Add,
                "more" | "less" => ModType::Mul,
                _ => ModType::Add, // Default to Add for unknown parts
            }
        } else {
            ModType::Add // Default to Add for base stats
        }
    }
    
    /// Get the total expression for a given path
    pub(crate) fn get_total_expression(&self, path: &StatPath) -> &str {
        self.total_expressions
            .get(path.name)
            .map(|s| s.as_str())
            .unwrap_or("0") // Default to "0" if no expression is found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stat_type_registration() {
        let mut config = Config::default();
        config.register_stat_type("Damage", "Tagged");
        
        let path = StatPath::parse("Damage");
        assert_eq!(config.get_stat_type(&path), "Tagged");
        
        let unknown_path = StatPath::parse("Unknown");
        assert_eq!(config.get_stat_type(&unknown_path), "Flat");
    }

    #[test]
    fn test_relationship_type_inference() {
        let mut config = Config::default();
        
        // Test default relationships based on part names
        assert_eq!(
            config.get_relationship_type(&StatPath::parse("Damage.increased")),
            ModType::Add
        );
        assert_eq!(
            config.get_relationship_type(&StatPath::parse("Damage.more")),
            ModType::Mul
        );

        // Test explicit registration
        config.register_relationship_type("Damage.special", ModType::Mul);
        assert_eq!(
            config.get_relationship_type(&StatPath::parse("Damage.special")),
            ModType::Mul
        );
    }

    #[test]
    fn test_total_expression() {
        let mut config = Config::default();
        config.register_total_expression("Damage", "base * (1 + increased) * more");
        
        let path = StatPath::parse("Damage");
        assert_eq!(
            config.get_total_expression(&path),
            "base * (1 + increased) * more"
        );
        
        let unknown_path = StatPath::parse("Unknown");
        assert_eq!(config.get_total_expression(&unknown_path), "0");
    }
}