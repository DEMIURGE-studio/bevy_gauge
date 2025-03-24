use std::collections::{HashMap, HashSet};
use std::hash::Hash;


#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ValueTag {
    pub primary_value_target: String,
    pub groups: Option<HashMap<String, TagGroup>>
}

impl ValueTag {
    pub fn stringify(&self) -> String {
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
        
        result
    }


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

        Ok(ValueTag {
            primary_value_target,
            groups: Some(groups)
        })
    }




    pub fn new(primary_value_target: String, groups: Option<HashMap<String, TagGroup>>) -> Self {
        ValueTag {
            primary_value_target,
            groups,
        }
    }

    pub fn add_all_group(&mut self, name: String) -> &mut Self {
        if let Some(ref mut groups) = self.groups {
            groups.insert(name, TagGroup::All);
        } else {
            let mut groups = HashMap::new();
            groups.insert(name, TagGroup::All);
            self.groups = Some(groups);
        }
        self
    }

    pub fn add_any_of_group(&mut self, name: String, values: HashSet<String>) -> &mut Self {
        if let Some(ref mut groups) = self.groups {
            groups.insert(name, TagGroup::AnyOf(values));
        } else {
            let mut groups = HashMap::new();
            groups.insert(name, TagGroup::AnyOf(values));
            self.groups = Some(groups);
        }
        self
    }
}

// Implementation for TagGroup to support conversions and manipulations
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

impl Hash for ValueTag {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.stringify().hash(state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagGroup {
    All,
    AnyOf(HashSet<String>),
}

// expand to query language
// Add wildcards such as AnyOf


#[cfg(test)]
mod tag_tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_stringify_empty() {
        let tag = ValueTag {
            primary_value_target: "stat".to_string(),
            groups: None,
        };

        assert_eq!(tag.stringify(), "stat");
    }

    #[test]
    fn test_stringify_with_all_group() {
        let mut tag = ValueTag {
            primary_value_target: "damage".to_string(),
            groups: None,
        };

        tag.add_all_group(String::from("physical"));

        assert_eq!(tag.stringify(), "damage(physical)");
    }

    #[test]
    fn test_stringify_with_any_of_group() {
        let mut tag = ValueTag {
            primary_value_target: "resist".to_string(),
            groups: None,
        };

        let mut values = HashSet::new();
        values.insert("fire".to_string());
        values.insert("ice".to_string());

        tag.add_any_of_group(String::from("element"), values);

        // Note: Values are sorted alphabetically for deterministic output
        assert_eq!(tag.stringify(), "resist(element[\"fire ice\"])");
    }

    #[test]
    fn test_stringify_complex() {
        let mut tag = ValueTag {
            primary_value_target: "bonus".to_string(),
            groups: None,
        };

        tag.add_all_group("elemental".to_string());

        let mut values = HashSet::new();
        values.insert("sword".to_string());
        values.insert("axe".to_string());
        values.insert("mace".to_string());

        tag.add_any_of_group(String::from("weapon"), values);

        // Note: Groups are sorted by name and values are sorted alphabetically
        assert_eq!(tag.stringify(), "bonus(elemental weapon[\"axe mace sword\"])");
    }

    #[test]
    fn test_parse_simple() {
        let s = "damage(physical)";
        let tag = ValueTag::parse(s).unwrap();

        assert_eq!(tag.primary_value_target, "damage");
        if let Some(ref groups) = tag.groups {

            assert_eq!(groups.len(), 1);
            assert!(groups.get("physical").unwrap().is_all());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_parse_any_of() {
        let s = "resist(element[\"fire ice\"])";
        let tag = ValueTag::parse(s).unwrap();

        assert_eq!(tag.primary_value_target, "resist");
        if let Some(ref groups) = tag.groups {
            assert_eq!(groups.len(), 1);

            let element_group = groups.get("element").unwrap();
            if let TagGroup::AnyOf(values) = element_group {
                assert_eq!(values.len(), 2);
                assert!(values.contains("fire"));
                assert!(values.contains("ice"));
            } else {
                panic!("Expected AnyOf group");
            }
        }
    }

    #[test]
    fn test_parse_complex() {
        let s = "bonus(melee weapon[\"axe mace sword\"])";
        let tag = ValueTag::parse(s).unwrap();

        assert_eq!(tag.primary_value_target, "bonus");
        if let Some(ref groups) = tag.groups {
            assert_eq!(groups.len(), 2);

            assert!(groups.get("melee").unwrap().is_all());

            let weapon_group = groups.get("weapon").unwrap();
            if let TagGroup::AnyOf(values) = weapon_group {
                assert_eq!(values.len(), 3);
                assert!(values.contains("axe"));
                assert!(values.contains("mace"));
                assert!(values.contains("sword"));
            } else {
                panic!("Expected AnyOf group");
            }
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_hash_consistency() {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        // Create two identical tags through different methods
        let mut tag1 = ValueTag::new("damage".to_string(), None);
        tag1.add_all_group("physical".to_string());

        let s = "damage(physical)";
        let tag2 = ValueTag::parse(s).unwrap();

        // Hash both tags
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        tag1.hash(&mut hasher1);
        tag2.hash(&mut hasher2);

        // Compare hash values
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_roundtrip() {
        let mut original = ValueTag::new("critical".to_string(), None);
        original.add_all_group("weapon".to_string());

        let mut values = HashSet::new();
        values.insert("backstab".to_string());
        values.insert("headshot".to_string());
        original.add_any_of_group("type".to_string(), values);

        let serialized = original.stringify();
        let parsed = ValueTag::parse(&serialized).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_multiple_groups_with_spaces() {
        let s = "attack(melee ranged physical)";
        let tag = ValueTag::parse(s).unwrap();

        assert_eq!(tag.primary_value_target, "attack");
        if let Some(ref groups) = tag.groups {

            assert_eq!(groups.len(), 3);
            assert!(groups.get("melee").unwrap().is_all());
            assert!(groups.get("ranged").unwrap().is_all());
            assert!(groups.get("physical").unwrap().is_all());

            let serialized = tag.stringify();
            assert_eq!(serialized, "attack(melee physical ranged)"); // Note: keys are sorted alphabetically
        }

        // Verify the round trip works
    }

}