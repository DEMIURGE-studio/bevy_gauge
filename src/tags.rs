use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};


// String to tag ID mapping
#[derive(Resource, Debug, Default, Clone)]
pub struct TagRegistry {
    // Maps string tag names to u32 IDs
    pub string_to_id: HashMap<String, HashMap<String, u32>>,
    // Maps u32 IDs to string tag names
    pub id_to_string: HashMap<String, HashMap<u32, String>>,
    // Next available ID
    next_id: HashMap<String, u32>,
}

impl TagRegistry {
    pub fn new() -> Self {
        Self {
            string_to_id: HashMap::new(),
            id_to_string: HashMap::new(),
            next_id: HashMap::new(),
        }
    }

    // Register a primary tag type (e.g., "DAMAGE", "WEAPON")
    pub fn register_primary_type(&mut self, primary_type: &str) {
        let primary_type = primary_type.to_lowercase();
        // Create entries in the registries if they don't exist
        self.string_to_id.entry(primary_type.clone()).or_insert(HashMap::new());
        self.id_to_string.entry(primary_type.clone()).or_insert(HashMap::new());
        self.next_id.entry(primary_type.clone()).or_insert(0);
    }

    // Register a subtype under a primary type (e.g., "FIRE" under "DAMAGE")
    pub fn register_subtype(&mut self, primary_type: &str, subtype: &str) -> u32 {
        let primary_type = &primary_type.to_lowercase();
        let subtype = &subtype.to_lowercase();
        
        // Make sure the primary type exists
        self.register_primary_type(primary_type);

        // Check if this subtype already exists
        if let Some(existing_id) = self.get_id(primary_type, subtype) {
            return existing_id;
        }

        // Get the next bit position for this primary type
        let id_counter = self.next_id.get_mut(primary_type).unwrap();
        let id = 1 << *id_counter;
        *id_counter += 1;

        // Register the tag
        self.string_to_id.get_mut(primary_type).unwrap().insert(subtype.to_string(), id);
        self.id_to_string.get_mut(primary_type).unwrap().insert(id, subtype.to_string());

        id
    }

    // Register a tag (compatibility method)
    pub fn register_tag(&mut self, primary_group: &str, tag: &str) -> u32 {
        let primary_group = &primary_group.to_lowercase();
        let tag = &tag.to_lowercase();
        
        // Check if this tag already exists
        if let Some(existing_id) = self.get_id(primary_group, tag) {
            return existing_id;
        }

        // Ensure primary group exists
        self.register_primary_type(primary_group);

        // Get the next ID for this primary group
        let id_counter = self.next_id.get_mut(primary_group).unwrap();
        let id = 1 << *id_counter;
        *id_counter += 1;

        // Register the tag
        self.string_to_id.get_mut(primary_group).unwrap().insert(tag.to_string(), id);
        self.id_to_string.get_mut(primary_group).unwrap().insert(id, tag.to_string());

        id
    }

    // Get a tag ID
    pub fn get_id(&self, primary_group: &str, tag: &str) -> Option<u32> {
        self.string_to_id.get(&primary_group.to_lowercase())?.get(&tag.to_lowercase()).copied()
    }

    // Get a tag name
    pub fn get_tag(&self, primary_group: &str, id: u32) -> Option<&String> {
        self.id_to_string.get(&primary_group.to_lowercase())?.get(&id)
    }

    // Check if one tag qualifies for another (bitwise AND check)
    pub fn tag_qualifies_for(&self, primary_group: &str, modifier_tag_id: u32, target_tag_id: u32) -> bool {
        modifier_tag_id & target_tag_id > 0
    }
}

#[cfg(test)]
mod tag_registry_tests {
    use super::*;

    #[test]
    fn test_register_primary_type() {
        let mut registry = TagRegistry::new();

        // Register primary types
        registry.register_primary_type("DAMAGE");
        registry.register_primary_type("WEAPON");

        // Verify the structures are initialized
        assert!(registry.string_to_id.contains_key("damage"));
        assert!(registry.string_to_id.contains_key("weapon"));

    }

    #[test]
    fn test_register_subtype() {
        let mut registry = TagRegistry::new();

        // Register subtypes
        let fire_id = registry.register_subtype("DAMAGE", "FIRE");
        let cold_id = registry.register_subtype("DAMAGE", "COLD");
        let lightning_id = registry.register_subtype("DAMAGE", "LIGHTNING");

        let sword_id = registry.register_subtype("WEAPON", "SWORD");
        let axe_id = registry.register_subtype("WEAPON", "AXE");

        // Verify ID values with bit patterns
        assert_eq!(fire_id, 1);           // 2^0 = 1 (binary: 001)
        assert_eq!(cold_id, 2);           // 2^1 = 2 (binary: 010)
        assert_eq!(lightning_id, 4);      // 2^2 = 4 (binary: 100)

        assert_eq!(sword_id, 1);          // 2^0 = 1 (binary: 001)
        assert_eq!(axe_id, 2);            // 2^1 = 2 (binary: 010)

        // Verify tag lookup
        assert_eq!(registry.get_id("DAMAGE", "FIRE"), Some(fire_id));
        assert_eq!(registry.get_id("DAMAGE", "COLD"), Some(cold_id));
        assert_eq!(registry.get_tag("DAMAGE", fire_id), Some(&"fire".to_string()));
        assert_eq!(registry.get_tag("WEAPON", sword_id), Some(&"sword".to_string()));

        // Non-existent tags should return None
        assert_eq!(registry.get_id("DAMAGE", "NONEXISTENT"), None);
        assert_eq!(registry.get_tag("DAMAGE", 32), None);
    }

    #[test]
    fn test_register_tag() {
        let mut registry = TagRegistry::new();

        // Register tags using the regular method
        let fire_id = registry.register_tag("DAMAGE", "FIRE");
        let cold_id = registry.register_tag("DAMAGE", "COLD");

        // Verify they have the expected bit patterns
        assert_eq!(fire_id, 1);  // 2^0 = 1
        assert_eq!(cold_id, 2);  // 2^1 = 2

        // Registering again should return the same ID
        let fire_id_again = registry.register_tag("DAMAGE", "FIRE");
        assert_eq!(fire_id, fire_id_again);
    }

    #[test]
    fn test_tag_qualification() {
        let mut registry = TagRegistry::new();

        // Register various tags
        let fire_id = registry.register_subtype("DAMAGE", "FIRE");    // 001
        let cold_id = registry.register_subtype("DAMAGE", "COLD");    // 010
        let lightning_id = registry.register_subtype("DAMAGE", "LIGHTNING"); // 100

        // Manual registration of "compound" tags
        let elemental_id = fire_id | cold_id | lightning_id;  // 111 (combines fire, cold, lightning)
        let fire_cold_id = fire_id | cold_id;                // 011 (combines fire and cold)

        // Fire should qualify for elemental
        assert!(registry.tag_qualifies_for("DAMAGE", fire_id, elemental_id));

        // Fire should qualify for fire_cold
        assert!(registry.tag_qualifies_for("DAMAGE", fire_id, fire_cold_id));

        // Fire should NOT qualify for cold
        assert!(!registry.tag_qualifies_for("DAMAGE", fire_id, cold_id));

        // Elemental should qualify for fire
        assert!(registry.tag_qualifies_for("DAMAGE", elemental_id, fire_id));

        // Compound tags should qualify for their components
        assert!(registry.tag_qualifies_for("DAMAGE", fire_cold_id, fire_id));
        assert!(registry.tag_qualifies_for("DAMAGE", fire_cold_id, cold_id));
        assert!(!registry.tag_qualifies_for("DAMAGE", fire_cold_id, lightning_id));
    }

    #[test]
    fn test_different_primary_groups() {
        let mut registry = TagRegistry::new();

        // Register similar subtypes in different primary groups
        let fire_id = registry.register_subtype("DAMAGE", "FIRE");
        let sword_id = registry.register_subtype("WEAPON", "SWORD");

        // Both should be the first bit in their respective groups
        assert_eq!(fire_id, 1);
        assert_eq!(sword_id, 1);

        // They should be retrievable independently
        assert_eq!(registry.get_id("DAMAGE", "FIRE"), Some(fire_id));
        assert_eq!(registry.get_id("WEAPON", "SWORD"), Some(sword_id));

        // They should not interfere with each other
        assert_eq!(registry.get_id("DAMAGE", "SWORD"), None);
        assert_eq!(registry.get_id("WEAPON", "FIRE"), None);
    }

    #[test]
    fn test_compound_tags_manual() {
        let mut registry = TagRegistry::new();

        // Register subtypes
        let fire_id = registry.register_subtype("DAMAGE", "FIRE");
        let cold_id = registry.register_subtype("DAMAGE", "COLD");
        let lightning_id = registry.register_subtype("DAMAGE", "LIGHTNING");

        // Create compound tags manually by OR-ing the subtypes
        let elemental_id = fire_id | cold_id | lightning_id;

        // Verify the bit patterns
        assert_eq!(fire_id, 1);       // 001
        assert_eq!(cold_id, 2);       // 010
        assert_eq!(lightning_id, 4);  // 100
        assert_eq!(elemental_id, 7);  // 111

        // Manually register the compound tag (this would be part of a register_compound_tag method)
        registry.string_to_id.get_mut("damage").unwrap().insert("elemental".to_string(), elemental_id);
        registry.id_to_string.get_mut("damage").unwrap().insert(elemental_id, "elemental".to_string());

        // Verify lookup
        assert_eq!(registry.get_id("DAMAGE", "ELEMENTAL"), Some(elemental_id));
        assert_eq!(registry.get_tag("DAMAGE", elemental_id), Some(&"elemental".to_string()));

        // Test qualification
        assert!(registry.tag_qualifies_for("DAMAGE", fire_id, elemental_id));
        assert!(registry.tag_qualifies_for("DAMAGE", cold_id, elemental_id));
        assert!(registry.tag_qualifies_for("DAMAGE", lightning_id, elemental_id));
        assert!(registry.tag_qualifies_for("DAMAGE", elemental_id, fire_id));
    }

    #[test]
    fn test_integration_with_modifier_system() {
        let mut registry = TagRegistry::new();

        // Register tags
        let fire_id = registry.register_subtype("DAMAGE", "FIRE");
        let cold_id = registry.register_subtype("DAMAGE", "COLD");
        let elemental_id = fire_id | cold_id;

        // This would simulate a system where:
        // - A stat has the "FIRE" tag
        // - A modifier has the "ELEMENTAL" tag
        // - The modifier should apply to the stat because FIRE is part of ELEMENTAL

        let stat_tag = fire_id;
        let modifier_tag = elemental_id;

        // The modifier should qualify for the stat
        assert!(registry.tag_qualifies_for("DAMAGE", modifier_tag, stat_tag));

        // This would allow code like:
        // if registry.tag_qualifies_for(primary_type, modifier.tag, stat.tag) {
        //     apply_modifier(modifier, stat);
        // }
    }
}