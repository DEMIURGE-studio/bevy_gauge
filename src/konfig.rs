use once_cell::sync::Lazy;
use std::sync::{RwLock, Mutex};
use std::collections::HashMap;
use super::prelude::*;
use regex::Regex;
use serial_test::serial;

// The actual Konfig data, protected by an RwLock for concurrent reads.
pub static KONFIG_DATA: Lazy<RwLock<Konfig>> = Lazy::new(|| {
    RwLock::new(Konfig::new_internal())
});

// A Mutex to serialize all attempts to acquire a write lock on KONFIG_DATA.
pub static KONFIG_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| {
    Mutex::new(())
});

pub struct Konfig {
    // Stat Types
    stat_types_map: HashMap<String, String>,
    stat_types_regex_rules: Vec<(Regex, String)>,
    stat_types_default: String,

    // Relationship Types
    relationship_types_map: HashMap<String, ModType>,
    relationship_types_regex_rules: Vec<(Regex, ModType)>,
    relationship_types_default: ModType,

    // Total Expressions
    total_expressions_map: HashMap<String, String>,
    total_expressions_regex_rules: Vec<(Regex, String)>,
    total_expressions_default: String,

    // TagSet Resolvers
    tag_set_resolvers: HashMap<String, Box<dyn TagSet + Send + Sync>>,
    default_tag_set_resolver: Option<Box<dyn TagSet + Send + Sync>>,
}

impl Konfig {
    fn new_internal() -> Self {
        Self {
            // Stat Types
            stat_types_map: HashMap::new(),
            stat_types_regex_rules: Vec::new(),
            stat_types_default: "Modifiable".to_string(),

            // Relationship Types
            relationship_types_map: HashMap::new(),
            relationship_types_regex_rules: Vec::new(),
            relationship_types_default: ModType::Add,

            // Total Expressions
            total_expressions_map: HashMap::new(),
            total_expressions_regex_rules: Vec::new(),
            total_expressions_default: "0".to_string(),

            // TagSet Resolvers
            tag_set_resolvers: HashMap::new(),
            default_tag_set_resolver: None,
        }
    }

    // --- Stat Type Methods ---
    pub fn get_stat_type(path_key: &str) -> String {
        let reader = KONFIG_DATA.read().unwrap();
        if let Some(value) = reader.stat_types_map.get(path_key) {
            return value.clone();
        }
        for (regex, value) in &reader.stat_types_regex_rules {
            if regex.is_match(path_key) {
                return value.clone();
            }
        }
        reader.stat_types_default.clone()
    }

    pub fn register_stat_type(stat: &str, stat_type: &str) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.stat_types_map.insert(stat.to_string(), stat_type.to_string());
    }

    pub fn register_stat_type_regex(pattern: &str, value: &str) -> Result<(), regex::Error> {
        let regex = Regex::new(pattern)?;
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.stat_types_regex_rules.push((regex, value.to_string()));
        Ok(())
    }

    pub fn set_stat_type_default(default_value: &str) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.stat_types_default = default_value.to_string();
    }

    // --- Relationship Type Methods ---
    pub fn get_relationship_type(path_key: &str) -> ModType {
        let reader = KONFIG_DATA.read().unwrap();
        if let Some(value) = reader.relationship_types_map.get(path_key) {
            return value.clone();
        }
        for (regex, value) in &reader.relationship_types_regex_rules {
            if regex.is_match(path_key) {
                return value.clone();
            }
        }
        reader.relationship_types_default.clone()
    }

    pub fn register_relationship_type(stat_path_part: &str, relationship: ModType) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.relationship_types_map.insert(stat_path_part.to_string(), relationship);
    }

    pub fn register_relationship_type_regex(pattern: &str, value: ModType) -> Result<(), regex::Error> {
        let regex = Regex::new(pattern)?;
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.relationship_types_regex_rules.push((regex, value));
        Ok(())
    }

    pub fn set_relationship_type_default(default_value: ModType) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.relationship_types_default = default_value;
    }

    // --- Total Expression Methods ---
    pub fn get_total_expression(path_key: &str) -> String {
        let reader = KONFIG_DATA.read().unwrap();
        if let Some(value) = reader.total_expressions_map.get(path_key) {
            return value.clone();
        }
        for (regex, value) in &reader.total_expressions_regex_rules {
            if regex.is_match(path_key) {
                return value.clone();
            }
        }
        reader.total_expressions_default.clone()
    }

    pub fn register_total_expression(stat: &str, expression: &str) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.total_expressions_map.insert(stat.to_string(), expression.to_string());
    }

    pub fn register_total_expression_regex(pattern: &str, value: &str) -> Result<(), regex::Error> {
        let regex = Regex::new(pattern)?;
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.total_expressions_regex_rules.push((regex, value.to_string()));
        Ok(())
    }

    pub fn set_total_expression_default(default_value: &str) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.total_expressions_default = default_value.to_string();
    }
    
    // --- TagSet Resolver Methods ---
    pub fn register_tag_set(stat_base_name: &str, resolver: Box<dyn TagSet + Send + Sync>) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.tag_set_resolvers.insert(stat_base_name.to_string(), resolver);
    }

    pub fn set_default_tag_set(resolver: Box<dyn TagSet + Send + Sync>) {
        let _guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut writer = KONFIG_DATA.write().unwrap();
        writer.default_tag_set_resolver = Some(resolver);
    }
    
    // Internal helper to get a TagSet resolver. This will be used by StatPath::parse.
    // This method itself doesn't need to be public API of Konfig.
    pub(crate) fn internal_get_tag_resolver_for_stat_name(&self, stat_name: &str) -> Option<&(dyn TagSet + Send + Sync)> {
        self.tag_set_resolvers.get(stat_name)
            .map(|boxed_resolver| boxed_resolver.as_ref()) // Deref Box to &dyn TagSet
            .or_else(|| self.default_tag_set_resolver.as_deref())
    }
    
    // --- Test Utility ---
    pub fn reset_for_test() {
        let _write_serialization_guard = KONFIG_WRITE_LOCK.lock().unwrap();
        let mut konfig_writer = KONFIG_DATA.write().unwrap();
        *konfig_writer = Konfig::new_internal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock TagSet for testing Konfig's TagSet registration
    struct MockTagSet;
    impl TagSet for MockTagSet {
        fn match_tag(&self, _tag_str: &str) -> u32 { 0 }
        fn tag_category_for_bit(&self, _tag_bit: u32) -> u32 { 0 }
        fn tag_category_for_group(&self, _group_tag: u32) -> u32 { 0 }
        fn all_defined_groups(&self) -> &'static [u32] { &[] }
        // build_permissive_tag, permissive_tag_from_str, build_permissive_mask can use defaults or be mocked
    }

    #[test]
    #[serial]
    fn test_konfig_access_and_registration() {
        Konfig::reset_for_test();

        Konfig::register_stat_type("Health", "Complex");
        Konfig::register_relationship_type("Strength", ModType::Mul);
        Konfig::register_total_expression("Mana", "base * (1 + increased)");

        assert_eq!(Konfig::get_stat_type("Health"), "Complex");
        assert_eq!(Konfig::get_relationship_type("Strength"), ModType::Mul);
        assert_eq!(Konfig::get_total_expression("Mana"), "base * (1 + increased)");
        assert_eq!(Konfig::get_stat_type("Dexterity"), "Modifiable"); // Checks default
    }

    #[test]
    #[serial]
    fn test_stat_type_regex_and_default() {
        Konfig::reset_for_test();
        Konfig::set_stat_type_default("CustomDefault");
        Konfig::register_stat_type_regex(r"\.\d+$", "Complex").unwrap();
        Konfig::register_stat_type_regex(r"(?i)(current)", "Flat").unwrap();
        Konfig::register_stat_type_regex(r"(?i)(base|added|increased)", "Complex").unwrap();
        Konfig::register_stat_type("CurrentBananas", "Complex");

        assert_eq!(Konfig::get_stat_type("CurrentLife"), "Flat"); // Regex match
        assert_eq!(Konfig::get_stat_type("CurrentBananas"), "Complex"); // Exact match
        assert_eq!(Konfig::get_stat_type("SomeOtherStat"), "CustomDefault"); // Default match
    }

    #[test]
    #[serial]
    fn test_tag_set_registration_and_retrieval() {
        Konfig::reset_for_test();
        
        let specific_resolver = Box::new(MockTagSet);
        // We can't directly compare Box<dyn Trait>, so we test by seeing if SOME resolver is returned.
        // A more thorough test would involve this resolver being used by StatPath::parse later.
        Konfig::register_tag_set("Damage", specific_resolver);

        let default_resolver = Box::new(MockTagSet);
        Konfig::set_default_tag_set(default_resolver);

        let reader = KONFIG_DATA.read().unwrap(); // Access internal data for test verification
        assert!(reader.internal_get_tag_resolver_for_stat_name("Damage").is_some(), "Damage specific resolver should exist");
        assert!(reader.internal_get_tag_resolver_for_stat_name("UnknownStat").is_some(), "Default resolver should be used for UnknownStat");
    }
} 