use bevy::prelude::Component;
use crate::stats::{EntityStat};

#[derive(Debug)]
pub enum DurationType {
    Instant,
    Infinite,
    Duration(f32)
}

#[derive(Debug)]
pub enum ModifierType {
    Flat(f32),
    Increased(f32),
    More(f32)
}


/// increased and more have 1 added to them, so there is no need to do 1.1 for 10% increased. 0.1 will increase by 10% -0.1 will decrease by 10%
///
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierValue {
    pub(crate) flat: f32,
    pub(crate) increased: f32,
    pub(crate) more: f32,
}

impl Default for ModifierValue {
    fn default() -> Self {
        ModifierValue {
            flat: 0.0,
            increased: 0.0,
            more: 1.0
        }
    }
}

#[derive(Debug)]
pub enum StatValueModifier {
    BoundedStatModifier {min: Option<ModifierType>, max: Option<ModifierType>, current: Option<ModifierType>},
    RawStatModifier {base: Option<ModifierType>, current: Option<ModifierType>},
}


#[derive(Component, Debug)]
pub struct StatModifier {
    pub duration_type: DurationType,
    pub value: StatValueModifier,
    pub target_stat: EntityStat
}

