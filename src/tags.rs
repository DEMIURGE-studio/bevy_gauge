use bevy::utils::HashMap;
use stat_macros::define_tags;

// define_tags! {
//     damage {
//         damage_type {
//             elemental { fire, cold, lightning },
//             physical,
//             chaos,
//         },
//         weapon_type {
//             melee { sword, axe },
//             ranged { bow, wand },
//         },
//     }
// }

// Recursive expansion of define_tags! macro
// ==========================================

pub mod Damage {
    pub const FIRE: u32 = 1 << 0u32; 
    pub const COLD: u32 = 1 << 1u32;
    pub const LIGHTNING: u32 = 1 << 2u32;
    pub const ELEMENTAL: u32 = 1 << 0u32 | 1 << 1u32 | 1 << 2u32;
    pub const PHYSICAL: u32 = 1 << 3u32;
    pub const CHAOS: u32 = 1 << 4u32;
    pub const DAMAGE_TYPE: u32 = 1 << 0u32 | 1 << 1u32 | 1 << 2u32 | 1 << 3u32 | 1 << 4u32;
    pub const SWORD: u32 = 1 << 5u32;
    pub const AXE: u32 = 1 << 6u32;
    pub const MELEE: u32 = 1 << 5u32 | 1 << 6u32;
    pub const BOW: u32 = 1 << 7u32;
    pub const WAND: u32 = 1 << 8u32;
    pub const RANGED: u32 = 1 << 7u32 | 1 << 8u32;
    pub const WEAPON_TYPE: u32 = 1 << 5u32 | 1 << 6u32 | 1 << 7u32 | 1 << 8u32;
    pub const DAMAGE: u32 = 1 << 0u32
        | 1 << 1u32
        | 1 << 2u32
        | 1 << 3u32
        | 1 << 4u32
        | 1 << 5u32
        | 1 << 6u32
        | 1 << 7u32
        | 1 << 8u32;
    pub fn match_tag(tag: &str) -> u32 {
        match tag {
            "fire" => FIRE,
            "cold" => COLD,
            "lightning" => LIGHTNING,
            "elemental" => ELEMENTAL,
            "physical" => PHYSICAL,
            "chaos" => CHAOS,
            "damage_type" => DAMAGE_TYPE,
            "sword" => SWORD,
            "axe" => AXE,
            "melee" => MELEE,
            "bow" => BOW,
            "wand" => WAND,
            "ranged" => RANGED,
            "weapon_type" => WEAPON_TYPE,
            "damage" => DAMAGE,
            _ => 0,
        }
    }

    pub fn build_tag_permissive(tags: &str) -> u32 {
        let cold = u32::MAX & !DAMAGE_TYPE | COLD;
        cold
    }

    pub fn build_tag_strict(tags: &str) -> u32 {
        let cold = COLD;
        cold
    }
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