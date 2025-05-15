stat_macros::define_tags! {
    DamageTags,
    damage_type {
        elemental { fire, cold, lightning },
        physical,
        chaos,
    },
    weapon_type {
        melee { sword, axe },
        ranged { bow, wand },
    },
}

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

        for tag_str_raw in tags_in_expr {
            let tag_str = tag_str_raw.trim().to_lowercase();
            let matched_tag_bits = self.match_tag(&tag_str);

            if matched_tag_bits > 0 {
                // Use tag_category_for_group as matched_tag_bits can be a group like ELEMENTAL
                let root_category_for_matched_tag = self.tag_category_for_group(matched_tag_bits);
                final_mask &= !root_category_for_matched_tag;
                accumulated_bits_from_expr |= matched_tag_bits;
            }
        }
        final_mask | accumulated_bits_from_expr
    }
}