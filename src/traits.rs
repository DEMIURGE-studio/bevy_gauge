use std::sync::Arc;

use super::prelude::*;

pub trait Named: Sized {
    const NAME: &'static str;
    fn to_string() -> String { Self::NAME.to_string() }
}

pub trait AsArcStr {
    fn to_str(&self) -> Arc<str>;
}

impl<T: Named> AsArcStr for T {
    fn to_str(&self) -> Arc<str> {
        Self::NAME.into()
    }
}

impl AsArcStr for String {
    fn to_str(&self) -> Arc<str> {
        self.as_str().into()
    }
}

impl AsArcStr for &String {
    fn to_str(&self) -> Arc<str> {
        self.as_str().into()
    }
}

impl AsArcStr for &str {
    fn to_str(&self) -> Arc<str> {
        (*self).into()
    }
}

impl AsArcStr for Arc<str> {
    fn to_str(&self) -> Arc<str> {
        self.clone()
    }
}

impl AsArcStr for &Arc<str> {
    fn to_str(&self) -> Arc<str> {
        (*self).clone()
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