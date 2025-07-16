use once_cell::sync::Lazy;
use std::sync::{RwLock, Mutex};
use std::collections::HashMap;
use super::prelude::*;
use regex::Regex;
use crate::tags; // Ensure process_tag is accessible

// The actual Konfig data, protected by an RwLock for concurrent reads.
pub static KONFIG_DATA: Lazy<RwLock<Konfig>> = Lazy::new(|| {
    RwLock::new(Konfig::new_internal())
});

// A Mutex to serialize all attempts to acquire a write lock on KONFIG_DATA.
pub static KONFIG_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| {
    Mutex::new(())
});

// Private helper function within the konfig.rs module
// Extracts the base stat name (e.g., "Damage" from "Damage.part.{TAG}@Target")
fn get_base_stat_name_from_path(full_path: &str) -> &str {
    // First, split by '@' to handle potential target alias, take the part before it.
    let path_without_target = full_path.split('@').next().unwrap_or(full_path);
    // Then, split by '.' to get the first segment, which is the base stat name.
    // If no '.', the whole string is considered the base name.
    path_without_target.split('.').next().unwrap_or(path_without_target)
}

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
    
    /// A one-stop shop function that processes a path string to resolve string-based tags
    /// into their numerical representations.
    ///
    /// It automatically determines the correct `TagSet` resolver based on the stat's base name
    /// (e.g., "Damage" from "Damage.type.{FIRE}@Player") by looking it up in the global `Konfig`.
    /// If a resolver is found, `crate::tags::process_tag` is called.
    ///
    /// If no string tag patterns (`.{...}` or `{...}`) are detected in the `path_str`
    /// that `process_tag` would act upon, or if no suitable `TagSet` resolver is found
    /// for the stat name, the original `path_str` is returned as a `String`.
    ///
    /// # Arguments
    /// * `path_str`: The stat path string, potentially containing string-based tags.
    ///
    /// # Returns
    /// A `String` which is either the processed path with numerical tags or the original path.
    pub fn process_path(path_str: &str) -> String {
        // Perform a check for patterns that `process_tag` specifically looks for.
        // These are ".{tag_expression}" or the entire main path being "{tag_expression}".
        let has_potential_string_tag = path_str.contains(".{") ||
                                     (path_str.starts_with('{') && path_str.ends_with('}'));

        if !has_potential_string_tag {
            return path_str.to_string();
        }

        let base_name = get_base_stat_name_from_path(path_str);
        
        // Acquire a read lock to access the Konfig instance
        let konfig_instance_guard = KONFIG_DATA.read().unwrap();
        
        if let Some(resolver) = konfig_instance_guard.internal_get_tag_resolver_for_stat_name(base_name) {
            // Call process_tag from the tags module
            tags::process_tag(path_str, resolver)
        } else {
            // No specific or default resolver found for this stat name.
            // Since process_tag requires a resolver to do its work,
            // and we didn't find one, return the original path string.
            // (process_tag itself would also return the original if its patterns aren't met,
            // but here the issue is the lack of a resolver).
            // Optionally, one might log a warning here if a string tag pattern was detected
            // but no resolver was configured for the base_name.
            // Example: log::warn!("Path '{}' may contain string tags but no TagSet resolver was found for base name '{}'", path_str, base_name);
            path_str.to_string()
        }
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
    use serial_test::serial;

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

        Konfig::register_stat_type("Life", "Complex");
        Konfig::register_relationship_type("Strength", ModType::Mul);
        Konfig::register_total_expression("Mana", "base * (1 + increased)");

        assert_eq!(Konfig::get_stat_type("Life"), "Complex");
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

        assert_eq!(Konfig::get_stat_type("$[Life.current]"), "Flat"); // Regex match
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

        {
            let reader = KONFIG_DATA.read().unwrap(); // Access internal data for test verification
            assert!(reader.internal_get_tag_resolver_for_stat_name("Damage").is_some(), "Damage specific resolver should exist");
            assert!(reader.internal_get_tag_resolver_for_stat_name("UnknownStat").is_some(), "Default resolver should be used for UnknownStat");
        } // `reader` is dropped here, releasing the read lock on KONFIG_DATA

        // Test the new fully_process_path_tags function
        // Setup a mock resolver that converts "FIRE" to 1, "COLD" to 2
        struct PathProcessingTagSet;
        impl TagSet for PathProcessingTagSet {
            fn match_tag(&self, tag_str: &str) -> u32 {
                match tag_str {
                    "FIRE" => 1,
                    "COLD" => 2,
                    _ => 0, // Unknown tags map to 0 for this simple mock
                }
            }
            fn build_permissive_mask(&self, tag_expression_str: &str) -> u32 {
                // Simplified for test: assume single tag or direct number
                if tag_expression_str == "FIRE" { 1 }
                else if tag_expression_str == "COLD" { 2 }
                else if tag_expression_str == "FIRE|COLD" { 3 } // simple OR
                else { tag_expression_str.parse().unwrap_or(u32::MAX) } // if it's a number, use it
            }
            // Other TagSet methods can be defaulted or minimally implemented
            fn tag_category_for_bit(&self, _tag_bit: u32) -> u32 { 0 }
            fn tag_category_for_group(&self, _group_tag: u32) -> u32 { 0 }
            fn all_defined_groups(&self) -> &'static [u32] { &[] }
        }
        Konfig::register_tag_set("TestStat", Box::new(PathProcessingTagSet));

        assert_eq!(Konfig::process_path("TestStat.part.{FIRE}"), "TestStat.part.1");
        assert_eq!(Konfig::process_path("TestStat.part.{COLD}@Player"), "TestStat.part.2@Player");
        assert_eq!(Konfig::process_path("TestStat.part.{FIRE|COLD}"), "TestStat.part.3");
        assert_eq!(Konfig::process_path("TestStat.part.{99}"), "TestStat.part.{99}"); // Numeric, unchanged by resolver logic
        assert_eq!(Konfig::process_path("TestStat.part.ActualNumericalTag"), "TestStat.part.ActualNumericalTag"); // No braces, unchanged
        //assert_eq!(Konfig::process_path("AnotherStat.part.{FIRE}"), "AnotherStat.part.0"); // Corrected: Was "AnotherStat.part.{FIRE}"
        //assert_eq!(Konfig::process_path("{FIRE}"), "0"); // Corrected: Was "3"
        
        // Test with a default resolver that can process "{FIRE}"
        Konfig::set_default_tag_set(Box::new(PathProcessingTagSet));
        assert_eq!(Konfig::process_path("AnyStat.value.{FIRE}"), "AnyStat.value.1"); // Uses default resolver
        assert_eq!(Konfig::process_path("{FIRE}"), "1"); // Now base_name is "{FIRE}", but default applies to resolve "FIRE"
                                                                   // This highlights that get_base_stat_name_from_path needs to be robust
                                                                   // or we accept that "{TAG}" implies using the default resolver.
                                                                   // The current get_base_stat_name_from_path would make base_name "{FIRE}".
                                                                   // The default resolver logic in internal_get_tag_resolver_for_stat_name doesn't care about the passed stat_name if specific not found.
    }
} 