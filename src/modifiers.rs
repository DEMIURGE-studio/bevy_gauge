use crate::stat_value::StatValue;
use crate::stats::AttributeId;
use crate::tags::TagRegistry;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign, Sub, SubAssign};
use evalexpr::HashMapContext;

#[derive(Debug, Clone)]
pub enum ModifierValue {
    Flat(StatValue),
    Increased(StatValue),
    More(StatValue),
}

impl ModifierValue {
    pub fn update_value_with_ctx(
        &mut self,
        stat_context: HashMapContext,
        tag_registry: &Res<TagRegistry>,
    ) {
        match self {
            ModifierValue::Flat(vt) => {
                vt.update_value_with_context(&stat_context);
            }
            ModifierValue::Increased(vt) => {
                vt.update_value_with_context(&stat_context);
            }
            ModifierValue::More(vt) => {
                vt.update_value_with_context(&stat_context);
            }
        }
    }

    pub fn get_value(&self) -> &StatValue {
        match self {
            ModifierValue::Flat(val) => val,
            ModifierValue::Increased(value) => value,
            ModifierValue::More(value) => value,
        }
    }
}

impl Default for ModifierValue {
    fn default() -> Self {
        ModifierValue::Flat(StatValue::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierValueTotal {
    flat: f32,
    increased: f32,
    more: f32,
}

impl ModifierValueTotal {
    pub fn get_total(&self) -> f32 {
        self.flat * (1.0 + self.increased) * self.more
    }

    pub fn get_total_with_base(&self, base: f32) -> f32 {
        (self.flat + base) * (1.0 + self.increased) * self.more
    }
}

impl Default for ModifierValueTotal {
    fn default() -> Self {
        Self {
            flat: 0.0,
            increased: 0.0,
            more: 1.0,
        }
    }
}

impl Add for ModifierValueTotal {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            flat: self.flat + other.flat,
            increased: self.increased + self.increased,
            more: (1.0 + self.more) * (1.0 + other.more),
        }
    }
}

impl Sub for ModifierValueTotal {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            flat: self.flat - other.flat,
            increased: self.increased - self.increased,
            more: (1.0 + self.more) / (1.0 + other.more),
        }
    }
}

impl AddAssign<&ModifierValue> for ModifierValueTotal {
    fn add_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => self.flat += val.get_value_f32(),
            ModifierValue::Increased(val) => self.increased += val.get_value_f32(),
            ModifierValue::More(val) => self.more *= 1.0 + val.get_value_f32(),
        }
    }
}

impl SubAssign<&ModifierValue> for ModifierValueTotal {
    fn sub_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => self.flat -= val.get_value_f32(),
            ModifierValue::Increased(val) => self.increased -= val.get_value_f32(),
            ModifierValue::More(val) => self.more /= 1.0 + val.get_value_f32(),
        }
    }
}

/// An instance that lives as a component, or rather a single component entity that exists as a child on a tree of a Stat Entity.
#[derive(Component, Debug)]
#[require(ModifierInstance)]
#[relationship(relationship_target = ModifierCollectionRefs)]
pub struct ModifierTarget {
    pub modifier_collection: Entity,
}

#[derive(Component, Debug, Default)]
pub struct ModifierInstance {
    pub target_stat: AttributeId,
    pub value: ModifierValue,
    pub dependencies: HashSet<AttributeId>,
}

impl ModifierInstance {
    pub fn update_value(&mut self, value: ModifierValue) {
        self.value = value;
    }
}

// /// A component on an entity to keep track of all Modifier Entities affecting this entity
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = ModifierTarget)]
pub struct ModifierCollectionRefs {
    #[relationship]
    modifiers: Vec<Entity>,
}
