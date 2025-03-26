use std::collections::{HashMap, HashSet};
use std::ops::{BitAnd, BitOr};
use std::sync::RwLock;
use bevy::prelude::*;
use bitvec::bitvec;
use bitvec::boxed::BitBox;
use crate::tags::{TagGroup, ValueTag};

#[derive(Clone, Debug, Default)]
pub enum BitPolicy {
    /// Missing bits are treated as 0 (strict matching)
    Strict,

    #[default]
    /// Missing bits are treated as 1 (permissive matching)
    Permissive,
}


#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct BitTagGroup {
    pub group: &'static str,
    pub policy: BitPolicy,
    #[deref]
    pub types: u32
}

impl BitOr for BitTagGroup {
    type Output = u32;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.types | rhs.types
    }
}

impl BitAnd for BitTagGroup {
    type Output = u32;
    
    
    fn bitand(self, rhs: Self) -> Self::Output {
        self.types & rhs.types
    }
}

pub struct BitSomething {
    pub values: HashMap<String, u32>
    // DAMAGE_TYPE 0000_0000_0000_0111 - elemental
    // WEAPON_TYPE 0000_0000_0000_0111 - sword
}

pub struct ModifierV2 {
    pub tag: u32,
}


// PRIMARY -< GROUPS -< TYPES
// STRING/HASH -> STRING/HASH -> BITAND COMPARE bits in each type

// 11111 -> 1110


// TARGET TAG - Damage of some ability
// damage.fire.hammer|axe.one_handed - 100_101_01

// QUERY is made, we treat all non-specified target groups as zero

// MODIFIER TAG
// damage.elemental.sword    - 111_100_XX_XXXXX // APPLIES
// damage elemental.melee    - 111_111_XX_XXXXX // APPLIES
// damage elemental.melee.1h - 111_111_01_XXXXX // APPLIES
// damage elemental.sword.1h - 111_010_01_XXXXX // APPLIES
// damage elemental.melee.2h - 111_111_10_XXXXX // FAILS
// damage melee.1h.warrior   - XXX_111_01_00001 // FAILS
