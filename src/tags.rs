stat_macros::define_tags! {
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

pub const TAG_CATEGORIES: &[u32] = &[
    DAMAGE_TYPE,
    ELEMENTAL,
    WEAPON_TYPE,
    MELEE,
    RANGED,
];

pub fn build_permissive_tag(tag: u32) -> u32 {
    let mut result = u32::MAX;
    
    // First check if the tag matches any complete categories
    for &category in TAG_CATEGORIES {
        // Check if the tag fully contains a category
        if tag & category != 0 {
            // If the tag has any bits from this category
            // (We don't require it to be the complete category, just any overlap)
            
            // Clear all bits in this category
            result &= !category;
            
            // Set only the bits from the input tag that belong to this category
            result |= tag & category;
        }
    }
    
    // For any remaining individual bits not handled by categories
    let mut i = 0;
    
    // Get all categories that we've processed already
    let processed_categories = TAG_CATEGORIES.iter().fold(0, |acc, &cat| {
        if tag & cat != 0 {
            acc | cat
        } else {
            acc
        }
    });
    
    // Only process bits that weren't part of any processed category
    let remaining_bits = tag & !processed_categories;
    let mut temp_tag = remaining_bits;
    
    while temp_tag > 0 {
        if temp_tag & 1 == 1 {
            let bit_tag = 1 << i;
            let category = tag_category(bit_tag);
            result &= !category;
            result |= bit_tag;
        }
        temp_tag >>= 1;
        i += 1;
    }
    
    result
}

pub fn permissive_tag_from_str(tag_str: &str) -> u32 {
    let tag = match_tag(tag_str);
    build_permissive_tag(tag)
}

pub fn build_permissive_mask(tag_expression: &str) -> u32 {
    let tags: Vec<&str> = tag_expression.split('|').collect();
    let mut result = u32::MAX;
    let mut tag_bits = 0u32;
    for tag_str in tags {
        let tag_str = tag_str.trim().to_lowercase();
        let tag = match_tag(&tag_str);
        if tag > 0 {
            let category = tag_category(tag);
            result &= !category;
            tag_bits |= tag;
        }
    }
    result |= tag_bits;
    result
}

/// A trait for types that act like a set of tags.
pub trait TagLike: Sized + Copy {
    /// Returns `true` if any of the bits in `other` are set in `self`.
    fn has_any(self, other: Self) -> bool;
    
    /// Returns `true` if the specific tag (or bit) in `other` is set in `self`.
    /// When `other` has only one bit set, this is equivalent to `has_any`.
    fn has_tag(self, other: Self) -> bool;
    
    /// Returns `true` if all of the bits in `other` are set in `self`.
    fn has_all(self, other: Self) -> bool;
    
    /// Inserts the bits in `other` into `self`.
    fn insert(&mut self, other: Self);
    
    /// Removes the bits in `other` from `self`.
    fn remove(&mut self, other: Self);
    
    /// Toggles the bits in `other` in `self`.
    fn toggle(&mut self, other: Self);
    
    /// Returns `true` if no tags are set.
    fn is_empty(self) -> bool;
    
    /// Returns the number of tags set.
    fn count(self) -> u32;
    
    /// Returns the union of `self` and `other` (bitwise OR).
    fn union(self, other: Self) -> Self;
    
    /// Returns the intersection of `self` and `other` (bitwise AND).
    fn intersection(self, other: Self) -> Self;
    
    /// Returns the underlying bitmask.
    fn bits(self) -> u32;
}

impl TagLike for u32 {
    fn has_any(self, other: Self) -> bool {
        self & other != 0
    }
    
    fn has_tag(self, other: Self) -> bool {
        self.has_any(other)
    }
    
    fn has_all(self, other: Self) -> bool {
        (self & other) == other
    }
    
    fn insert(&mut self, other: Self) {
        *self |= other;
    }
    
    fn remove(&mut self, other: Self) {
        *self &= !other;
    }
    
    fn toggle(&mut self, other: Self) {
        *self ^= other;
    }
    
    fn is_empty(self) -> bool {
        self == 0
    }
    
    fn count(self) -> u32 {
        self.count_ones()
    }
    
    fn union(self, other: Self) -> Self {
        self | other
    }
    
    fn intersection(self, other: Self) -> Self {
        self & other
    }
    
    fn bits(self) -> u32 {
        self
    }
}