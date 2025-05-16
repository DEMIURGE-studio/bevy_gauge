use once_cell::sync::Lazy;
use std::sync::{RwLock, Mutex};
use std::collections::HashMap;
use super::prelude::*;

// The actual Konfig data, protected by an RwLock for concurrent reads.
pub static KONFIG_DATA: Lazy<RwLock<Konfig>> = Lazy::new(|| {
    RwLock::new(Konfig::new_internal())
});

// A Mutex to serialize all attempts to acquire a write lock on KONFIG_DATA.
pub static KONFIG_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| {
    Mutex::new(())
});

pub struct Konfig {
    stat_types: HashMap<String, String>,
    relationship_types: HashMap<String, ModType>,
    total_expressions: HashMap<String, String>,
}

impl Konfig {
    // Renamed to new_internal to avoid conflict with a potential public new if ever needed.
    fn new_internal() -> Self {
        Self {
            stat_types: HashMap::new(),
            relationship_types: HashMap::new(),
            total_expressions: HashMap::new(),
        }
    }

    // Internal methods for direct mutation, only called when write lock is held.
    fn internal_register_stat_type(&mut self, stat: &str, stat_type: &str) {
        self.stat_types.insert(stat.to_string(), stat_type.to_string());
    }

    fn internal_register_relationship_type(&mut self, stat: &str, relationship: ModType) {
        self.relationship_types.insert(stat.to_string(), relationship);
    }
    
    fn internal_register_total_expression(&mut self, stat: &str, expression: &str) {
        self.total_expressions.insert(stat.to_string(), expression.to_string());
    }

    // Public read accessors - these use the KONFIG_DATA RwLock directly.
    // Note: These are now associated functions, not methods on &self.
    pub fn get_stat_type(path: &str) -> String {
        KONFIG_DATA.read().unwrap()
            .stat_types
            .get(path)
            .map(|s| s.clone())
            .unwrap_or_else(|| "Modifiable".to_string()) 
    }

    pub fn get_relationship_type(path: &str) -> ModType {
        KONFIG_DATA.read().unwrap()
            .relationship_types
            .get(path)
            .unwrap_or(&ModType::Add) // Default to Add if not specified
            .clone()
    }
    
    pub fn get_total_expression(path: &str) -> String {
        KONFIG_DATA.read().unwrap()
            .total_expressions
            .get(path)
            .map(|s| s.clone())
            .unwrap_or_else(|| "0".to_string()) // Default to "0" if not specified
    }

    // Public write accessors - these use the KONFIG_WRITE_LOCK first, then KONFIG_DATA's write lock.
    pub fn register_stat_type(stat: &str, stat_type: &str) {
        let _write_serialization_guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut konfig_writer = KONFIG_DATA.write().unwrap();
        konfig_writer.internal_register_stat_type(stat, stat_type);
    }

    pub fn register_relationship_type(stat: &str, relationship: ModType) {
        let _write_serialization_guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut konfig_writer = KONFIG_DATA.write().unwrap();
        konfig_writer.internal_register_relationship_type(stat, relationship);
    }
    
    pub fn register_total_expression(stat: &str, expression: &str) {
        let _write_serialization_guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut konfig_writer = KONFIG_DATA.write().unwrap();
        konfig_writer.internal_register_total_expression(stat, expression);
    }
    
    pub fn reset_for_test() {
        let _write_serialization_guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut konfig_writer = KONFIG_DATA.write().unwrap();
        *konfig_writer = Konfig::new_internal();
    }
}

// Removed the old static KONFIG and the direct impl Default for Konfig as it's handled internally now.

#[cfg(test)]
mod tests {
    use super::*; // Imports Konfig (struct and its static methods), ModType, StatPath

    #[test]
    fn test_konfig_access_and_registration() {
        Konfig::reset_for_test();

        Konfig::register_stat_type("Health", "Vitality");
        Konfig::register_relationship_type("Strength", ModType::Mul);
        Konfig::register_total_expression("Mana", "BaseMana + Intellect * 5");

        assert_eq!(Konfig::get_stat_type("Health"), "Vitality");
        assert_eq!(Konfig::get_relationship_type("Strength"), ModType::Mul);
        assert_eq!(Konfig::get_total_expression("Mana"), "BaseMana + Intellect * 5");
        assert_eq!(Konfig::get_stat_type("Damage"), "Modifiable");
    }
} 