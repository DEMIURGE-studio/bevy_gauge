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