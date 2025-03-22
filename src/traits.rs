use stat_macros::stat_component;

use super::prelude::*;

pub trait AsF32 {
    fn to_f32(&self) -> f32;
}

impl AsF32 for f32 {
    fn to_f32(&self) -> f32 {
        *self
    }
}

impl AsF32 for i32 {
    fn to_f32(&self) -> f32 {
        *self as f32
    }
}

/// Requires a corresponding stat_component_system.
pub trait StatDerived {
    fn from_stats(stats: &StatContextRefs) -> Self;

    fn should_update(&self, stats: &StatContextRefs) -> bool;

    fn update_from_stats(&mut self, stats: &StatContextRefs);

    fn is_valid(stats: &StatContextRefs) -> bool;
}

pub trait WriteBack {
    fn write_back(&self, stats: &mut StatDefinitions);
}

stat_component!(
    pub struct Simple {
        max: ..,
        current: ..WriteBack,
    };
); 

#[derive(Debug, Default)]
struct Damage {
    max: f32,
    min: f32,
}

stat_component!(
    pub struct Depth {
        damage: Damage {
            max: ..,
            min: ..,
        }
    }
);

#[derive(Default)]
pub struct OnBlock;

#[derive(Default)]
pub struct OnMeditate;

stat_component!(
    pub struct Generic<T> {
        max: ..,
        current: WriteBack,
    };
    (OnBlock, OnMeditate)
);