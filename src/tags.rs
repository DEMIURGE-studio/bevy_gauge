/// A trait for types that act like a set of tags.
pub trait TagLike: Sized + Copy {
    fn has_any(self, other: Self) -> bool;
    fn has_tag(self, other: Self) -> bool;
    fn has_all(self, other: Self) -> bool;
    fn insert(&mut self, other: Self);
    fn remove(&mut self, other: Self);
    fn toggle(&mut self, other: Self);
    fn is_empty(self) -> bool;
    fn count(self) -> u32;
    fn union(self, other: Self) -> Self;
    fn intersection(self, other: Self) -> Self;
    fn bits(self) -> u32;
}

impl TagLike for u32 {
    fn has_any(self, other: Self) -> bool { self & other != 0 }
    fn has_tag(self, other: Self) -> bool { self.has_any(other) }
    fn has_all(self, other: Self) -> bool { (self & other) == other }
    fn insert(&mut self, other: Self) { *self |= other; }
    fn remove(&mut self, other: Self) { *self &= !other; }
    fn toggle(&mut self, other: Self) { *self ^= other; }
    fn is_empty(self) -> bool { self == 0 }
    fn count(self) -> u32 { self.count_ones() }
    fn union(self, other: Self) -> Self { self | other }
    fn intersection(self, other: Self) -> Self { self & other }
    fn bits(self) -> u32 { self }
}

pub trait TagSet {
    fn match_tag(&self, tag_str: &str) -> u32;
    fn tag_category_for_bit(&self, tag_bit: u32) -> u32;
    fn tag_category_for_group(&self, group_tag: u32) -> u32;
    fn all_defined_groups(&self) -> &'static [u32];

    fn build_permissive_tag(&self, tag: u32) -> u32 {
        let mut result = u32::MAX;
        let defined_groups = self.all_defined_groups();

        for &group_mask in defined_groups {
            if (tag & group_mask) != 0 { // Check for any overlap
                result &= !group_mask;
                result |= tag & group_mask;
            }
        }

        let processed_groups_mask = defined_groups.iter().fold(0, |acc, &group_mask| {
            if (tag & group_mask) != 0 {
                acc | group_mask
            } else {
                acc
            }
        });

        let remaining_tag_bits = tag & !processed_groups_mask;
        let mut temp_tag = remaining_tag_bits;
        let mut bit_pos = 0;

        while temp_tag > 0 {
            if temp_tag & 1 == 1 {
                let current_single_bit = 1 << bit_pos;
                let owning_group = self.tag_category_for_bit(current_single_bit);
                result &= !owning_group;
                result |= current_single_bit;
            }
            temp_tag >>= 1;
            bit_pos += 1;
        }
        result
    }

    fn permissive_tag_from_str(&self, tag_str: &str) -> u32 {
        let tag = self.match_tag(tag_str);
        self.build_permissive_tag(tag)
    }

    fn build_permissive_mask(&self, tag_expression: &str) -> u32 {
        let tags_in_expr: Vec<&str> = tag_expression.split('|').collect();
        let mut final_mask = u32::MAX;
        let mut accumulated_bits_from_expr = 0u32;
        let mut known_tag_found = false;

        for tag_str_raw in tags_in_expr {
            let tag_str = tag_str_raw.trim().to_lowercase();
            if tag_str.is_empty() { continue; } 
            let matched_tag_bits = self.match_tag(&tag_str);

            if matched_tag_bits > 0 {
                known_tag_found = true;
                let root_category_for_matched_tag = self.tag_category_for_group(matched_tag_bits);
                final_mask &= !root_category_for_matched_tag;
                accumulated_bits_from_expr |= matched_tag_bits;
            } 
        }
        if !known_tag_found {
            return u32::MAX; 
        }
        final_mask | accumulated_bits_from_expr
    }
}

/// Converts a stat path string with a string-based tag expression to one with a numerical tag.
///
/// The function identifies the tag expression as the last segment of the path before any
/// "@" target alias. It uses the provided `tag_resolver`'s `build_permissive_mask` method
/// to convert this expression into a u32 bitmask.
///
/// # Arguments
/// * `path_str`: The input stat path string (e.g., "Damage.increased.FIRE|AXES@Player").
/// * `tag_resolver`: An implementation of the `TagSet` trait used to resolve the tag string.
///
/// # Returns
/// A `String` with the string tag expression replaced by its numerical u32 representation.
pub fn process_tag(path_str: &str, tag_resolver: &(dyn TagSet + Send + Sync)) -> String {
    let mut target_alias_part = "";
    let mut main_path_part = path_str;

    if let Some((main, _alias_with_at)) = path_str.rsplit_once('@') {
        main_path_part = main;
        target_alias_part = &path_str[main.len()..];
    }

    if main_path_part.is_empty() {
        return path_str.to_string();
    }

    // Look for the pattern ".{tag_expression}"
    if main_path_part.ends_with('}') {
        if let Some(dot_brace_idx) = main_path_part.rfind(".{") {
            // Ensure there's a base path before ".{"
            // Or it could be just ".{TAG}" which is not what we usually expect for a base_path_part.
            // However, if dot_brace_idx is 0, it means the string starts with ".{", e.g. ".{TAG}"
            // This case is unusual, let's assume base_path_part should not be empty if this pattern matches.
            if dot_brace_idx == 0 && !main_path_part.starts_with(".{") { // Should be main_path_part[0] == '.' and main_path_part[1] == '{'
                 // This is effectively a path like ".{TAG}" which means empty base_path_part.
                 // Let's consider this as not matching the intended "BasePath.{TagExpr}" pattern for now.
                 // Or, we decide base_path_part can be empty. For now, assume it implies original string if base is empty.
                 // return path_str.to_string(); // Or handle differently if ".{TAG}" is valid by itself
            } 

            let base_path_part = &main_path_part[..dot_brace_idx];
            let tag_expression_str = &main_path_part[dot_brace_idx + 2 .. main_path_part.len() - 1];

            if tag_expression_str.is_empty() {
                return path_str.to_string(); // e.g. "Name.{}"
            }

            // If content inside {} is purely digits, assume it is NOT a string tag expression
            // and should not be processed by the TagSet resolver here.
            if tag_expression_str.chars().all(|c| c.is_digit(10)) {
                 return path_str.to_string(); // e.g. "Name.{123}"
            }

            let numeric_tag = tag_resolver.build_permissive_mask(tag_expression_str);
            if numeric_tag == u32::MAX && tag_expression_str != u32::MAX.to_string() {
                return path_str.to_string(); // Keep original if no valid tags resolved
            }
            
            // Avoid empty base_path_part leading to just ".<num>@target"
            if base_path_part.is_empty() && main_path_part.starts_with(".{") {
                 return format!(".{}{}", numeric_tag, target_alias_part); // Path was like ".{TAG}"
            }
            return format!("{}.{}{}", base_path_part, numeric_tag, target_alias_part);
        }
    }

    // Check if the entire main_path_part is a "{TAG_EXPR}" without a preceding base path part.
    if main_path_part.starts_with('{') && main_path_part.ends_with('}') && main_path_part.len() > 2 {
        let tag_expression_str = &main_path_part[1 .. main_path_part.len() - 1];

        if tag_expression_str.is_empty() {
            return path_str.to_string(); // e.g. "{}"
        }
        if tag_expression_str.chars().all(|c| c.is_digit(10)) {
            return path_str.to_string(); // e.g. "{123}"
        }

        let numeric_tag = tag_resolver.build_permissive_mask(tag_expression_str);
        if numeric_tag == u32::MAX && tag_expression_str != u32::MAX.to_string() {
            return path_str.to_string();
        }
        return format!("{}{}", numeric_tag, target_alias_part);
    }
    
    path_str.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct DamageTags;

    impl DamageTags {
        pub const FIRE: u32 = 1u32 << 0u32;
        pub const COLD: u32 = 1u32 << 1u32;
        pub const LIGHTNING: u32 = 1u32 << 2u32;
        pub const ELEMENTAL: u32 = 1u32 << 0u32 | 1u32 << 1u32 | 1u32 << 2u32;
        pub const PHYSICAL: u32 = 1u32 << 3u32;
        pub const CHAOS: u32 = 1u32 << 4u32;
        pub const DAMAGE_TYPE: u32 =
            1u32 << 0u32 | 1u32 << 1u32 | 1u32 << 2u32 | 1u32 << 3u32 | 1u32 << 4u32;
        pub const SWORD: u32 = 1u32 << 5u32;
        pub const AXE: u32 = 1u32 << 6u32;
        pub const MELEE: u32 = 1u32 << 5u32 | 1u32 << 6u32;
        pub const BOW: u32 = 1u32 << 7u32;
        pub const WAND: u32 = 1u32 << 8u32;
        pub const RANGED: u32 = 1u32 << 7u32 | 1u32 << 8u32;
        pub const WEAPON_TYPE: u32 = 1u32 << 5u32 | 1u32 << 6u32 | 1u32 << 7u32 | 1u32 << 8u32;
        fn generated_tag_category(tag_value: u32) -> u32 {
            match tag_value {
                Self::ELEMENTAL => Self::DAMAGE_TYPE,
                Self::FIRE => Self::DAMAGE_TYPE,
                Self::COLD => Self::DAMAGE_TYPE,
                Self::LIGHTNING => Self::DAMAGE_TYPE,
                Self::PHYSICAL => Self::DAMAGE_TYPE,
                Self::CHAOS => Self::DAMAGE_TYPE,
                Self::MELEE => Self::WEAPON_TYPE,
                Self::SWORD => Self::WEAPON_TYPE,
                Self::AXE => Self::WEAPON_TYPE,
                Self::RANGED => Self::WEAPON_TYPE,
                Self::BOW => Self::WEAPON_TYPE,
                Self::WAND => Self::WEAPON_TYPE,
                val => val,
            }
        }
    }
    impl TagSet for DamageTags {
        fn match_tag(&self, tag_str: &str) -> u32 {
            match tag_str.trim().to_lowercase().as_str() {
                "damage_type" => Self::DAMAGE_TYPE,
                "elemental" => Self::ELEMENTAL,
                "fire" => Self::FIRE,
                "cold" => Self::COLD,
                "lightning" => Self::LIGHTNING,
                "physical" => Self::PHYSICAL,
                "chaos" => Self::CHAOS,
                "weapon_type" => Self::WEAPON_TYPE,
                "melee" => Self::MELEE,
                "sword" => Self::SWORD,
                "axe" => Self::AXE,
                "ranged" => Self::RANGED,
                "bow" => Self::BOW,
                "wand" => Self::WAND,
                _ => 0,
            }
        }
        fn tag_category_for_bit(&self, tag_bit: u32) -> u32 {
            Self::generated_tag_category(tag_bit)
        }
        fn tag_category_for_group(&self, group_tag: u32) -> u32 {
            Self::generated_tag_category(group_tag)
        }
        fn all_defined_groups(&self) -> &'static [u32] {
            &[
                Self::DAMAGE_TYPE,
                Self::ELEMENTAL,
                Self::WEAPON_TYPE,
                Self::MELEE,
                Self::RANGED,
            ]
        }
    }

    #[test]
    fn test_convert_simple_curly_tags() {
        let resolver = DamageTags;
        let fire_val = resolver.build_permissive_mask("FIRE");
        assert_eq!(process_tag("Damage.increased.{FIRE}", &resolver), format!("Damage.increased.{}", fire_val));
        
        let axe_val = resolver.build_permissive_mask("AXE");
        assert_eq!(process_tag("Attack.speed.{AXE}", &resolver), format!("Attack.speed.{}", axe_val));
    }

    #[test]
    fn test_convert_combined_curly_tags() {
        let resolver = DamageTags;
        let fire_axe_val = resolver.build_permissive_mask("FIRE|AXE");
        assert_eq!(process_tag("Damage.type.{FIRE|AXE}", &resolver), format!("Damage.type.{}", fire_axe_val));

        let melee_phys_val = resolver.build_permissive_mask("MELEE|PHYSICAL");
        assert_eq!(process_tag("Effect.scale.{MELEE|PHYSICAL}", &resolver), format!("Effect.scale.{}", melee_phys_val));
    }

    #[test]
    fn test_convert_curly_with_target_alias() {
        let resolver = DamageTags;
        let fire_val = resolver.build_permissive_mask("FIRE");
        assert_eq!(process_tag("Damage.increased.{FIRE}@Player", &resolver), format!("Damage.increased.{}@Player", fire_val));

        let melee_phys_val = resolver.build_permissive_mask("MELEE|PHYSICAL");
        assert_eq!(process_tag("Effect.scale.{MELEE|PHYSICAL}@Enemy1", &resolver), format!("Effect.scale.{}@Enemy1", melee_phys_val));
    }

    #[test]
    fn test_numerical_tags_and_no_curly_tags_unchanged() {
        let resolver = DamageTags;
        assert_eq!(process_tag("Damage.increased.123", &resolver), "Damage.increased.123");
        assert_eq!(process_tag("Effect.modifier.42@Source", &resolver), "Effect.modifier.42@Source");
        assert_eq!(process_tag("Health.current", &resolver), "Health.current");
        assert_eq!(process_tag("Health", &resolver), "Health");
        assert_eq!(process_tag("Health@Player", &resolver), "Health@Player");
    }
    
    #[test]
    fn test_curly_numerical_content_unchanged() { 
        let resolver = DamageTags;
        assert_eq!(process_tag("Damage.increased.{123}", &resolver), "Damage.increased.{123}");
    }

    #[test]
    fn test_pure_curly_tag_expression_no_base_path() {
        let resolver = DamageTags;
        let fire_axe_val = resolver.build_permissive_mask("FIRE|AXE");
        assert_eq!(process_tag("{FIRE|AXE}", &resolver), format!("{}", fire_axe_val));
        let sword_val = resolver.build_permissive_mask("SWORD");
        assert_eq!(process_tag("{SWORD}@Source", &resolver), format!("{}@Source", sword_val));
    }
    
    #[test]
    fn test_empty_curly_tag_expression_or_parts() {
        let resolver = DamageTags;
        assert_eq!(process_tag("Damage.increased.{}", &resolver), "Damage.increased.{}");
        let fire_axe_val = resolver.build_permissive_mask("FIRE||AXE"); 
        assert_eq!(process_tag("Damage.mod.{FIRE||AXE}", &resolver), format!("Damage.mod.{}", fire_axe_val));
    }

    #[test]
    fn test_unknown_tags_in_curly_expression() {
        let resolver = DamageTags;
        let fire_unknown_val = resolver.build_permissive_mask("FIRE|UNKNOWNTAG"); 
        assert_eq!(process_tag("Damage.value.{FIRE|UNKNOWNTAG}", &resolver), format!("Damage.value.{}", fire_unknown_val));
        
        assert_eq!(process_tag("Damage.value.{COMPLETELYUNKNOWN}", &resolver), "Damage.value.{COMPLETELYUNKNOWN}");
    }

    #[test]
    fn test_case_insensitivity_in_curly() { // Renamed for clarity
        let resolver = DamageTags;
        let fire_axe_val = resolver.build_permissive_mask("fire|axe");
        assert_eq!(process_tag("Damage.type.{FiRe|AxE}", &resolver), format!("Damage.type.{}", fire_axe_val));
    }

    #[test]
    fn test_edge_cases_curly_empty_and_at_only() { // Renamed for clarity
        let resolver = DamageTags;
        assert_eq!(process_tag("", &resolver), "");
        assert_eq!(process_tag("@Player", &resolver), "@Player");
        assert_eq!(process_tag(".{}", &resolver), ".{}"); 
        assert_eq!(process_tag("Name.{}.@Target", &resolver), "Name.{}.@Target");
        assert_eq!(process_tag("{}", &resolver), "{}");
        assert_eq!(process_tag("{}@Target", &resolver), "{}@Target");
    }
     #[test]
    fn test_path_like_dot_curly_tag() {
        let resolver = DamageTags;
        let fire_val = resolver.build_permissive_mask("FIRE");
        assert_eq!(process_tag(".{FIRE}", &resolver), format!(".{}", fire_val));
    }
}