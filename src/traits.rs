use super::prelude::*;

pub trait Named: Sized {
    const NAME: &'static str;
    fn to_string() -> String { Self::NAME.to_string() }
}

pub trait AsStr {
    fn to_str(&self) -> &str;
}

impl<T: Named> AsStr for T {
    fn to_str(&self) -> &str {
        Self::NAME
    }
}

impl AsStr for String {
    fn to_str(&self) -> &str {
        self.as_str()
    }
}

impl AsStr for &String {
    fn to_str(&self) -> &str {
        self.as_str()
    }
}

impl AsStr for &str {
    fn to_str(&self) -> &str {
        self
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