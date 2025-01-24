use super::prelude::*;

pub trait Named: Sized {
    const NAME: &'static str;
    fn to_string() -> String { Self::NAME.to_string() }
}

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

pub trait Fields {
    const FIELDS: &'static [&'static str];

    fn set(&mut self, field: &str, value: f32);
}

/// Requires a corresponding stat_component_system.
pub trait StatDerived {
    fn from_stats(stats: &StatContextRefs) -> Self;

    fn update_from_stats(&mut self, stats: &StatContextRefs);

    fn is_valid(stats: &StatContextRefs) -> bool;
}