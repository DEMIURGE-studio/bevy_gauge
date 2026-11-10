//! Nested attribute resolution for composite types.
//!
//! [`AttributeResolvable`] allows struct and enum fields to be updated from
//! attributes using dot-separated paths. Terminal types (f32, integers, bool,
//! Duration) read a single attribute value. Composite types delegate to their
//! fields, appending the field name to the prefix.
//!
//! A terminal whose attribute doesn't exist on the entity is left as it is
//! (and never asks to be resolved), so gauge can drive some fields of a
//! composite while the rest keep their authored values.
//!
//! # Path convention
//!
//! Given:
//! ```
//! struct Outer {
//!     inner: Inner,
//! }
//! struct Inner {
//!     radius: f32,
//!     count: u32,
//! }
//! ```
//!
//! The attribute paths are `"Outer.inner.radius"` and `"Outer.inner.count"`.
//!
//! For enums, the variant name is transparent — fields are matched by name
//! across all variants. Variants without a matching field are no-ops.
//!
//! # Example
//!
//! ```
//! # use bevy_gauge::prelude::*;
//! # struct Inner { radius: f32, count: u32 }
//! impl AttributeResolvable for Inner {
//!     fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
//!         self.radius.should_resolve(&format!("{prefix}.radius"), attrs)
//!             || self.count.should_resolve(&format!("{prefix}.count"), attrs)
//!     }
//!
//!     fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
//!         self.radius.resolve(&format!("{prefix}.radius"), attrs);
//!         self.count.resolve(&format!("{prefix}.count"), attrs);
//!     }
//! }
//! ```

use std::sync::{Arc, OnceLock, PoisonError, RwLock};
use std::time::Duration;

use bevy::platform::collections::HashMap;

use crate::attribute_id::{global_rodeo, AttributeId};
use crate::attributes::Attributes;

/// A type whose fields can be updated from attribute values using path-based
/// resolution. Implement for terminal types (leaf values) and composite types
/// (structs/enums that delegate to their fields).
pub trait AttributeResolvable {
    /// Returns `true` if any attribute value differs from the current field state.
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool;

    /// Update fields from attribute values. Only called when `should_resolve`
    /// returned `true`.
    fn resolve(&mut self, prefix: &str, attrs: &Attributes);
}

// ---------------------------------------------------------------------------
// Terminal impls — floats
// ---------------------------------------------------------------------------

impl AttributeResolvable for f32 {
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
        attrs.try_value(prefix).is_some_and(|v| float_differs(*self, v))
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        if let Some(v) = attrs.try_value(prefix) {
            *self = v;
        }
    }
}

impl AttributeResolvable for f64 {
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
        // Attribute values are f32; compare at f32 precision.
        attrs.try_value(prefix).is_some_and(|v| float_differs(*self as f32, v))
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        if let Some(v) = attrs.try_value(prefix) {
            *self = v as f64;
        }
    }
}

// ---------------------------------------------------------------------------
// Terminal impls — integers
// ---------------------------------------------------------------------------

macro_rules! impl_resolvable_int {
    ($($ty:ty),*) => {$(
        impl AttributeResolvable for $ty {
            fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
                attrs.try_value(prefix).is_some_and(|v| *self != v.round() as $ty)
            }

            fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
                if let Some(v) = attrs.try_value(prefix) {
                    *self = v.round() as $ty;
                }
            }
        }
    )*};
}

impl_resolvable_int!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

// ---------------------------------------------------------------------------
// Terminal impls — bool
// ---------------------------------------------------------------------------

impl AttributeResolvable for bool {
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
        attrs.try_value(prefix).is_some_and(|v| *self != (v != 0.0))
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        if let Some(v) = attrs.try_value(prefix) {
            *self = v != 0.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Terminal impls — Duration
// ---------------------------------------------------------------------------

/// Resolves from seconds. Zero is a valid duration; negative or non-finite
/// values are ignored (a `Duration` can't represent them), as is a missing
/// attribute.
impl AttributeResolvable for Duration {
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
        duration_secs(prefix, attrs).is_some_and(|secs| float_differs(self.as_secs_f32(), secs))
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        if let Some(secs) = duration_secs(prefix, attrs) {
            *self = Duration::from_secs_f32(secs);
        }
    }
}

fn duration_secs(prefix: &str, attrs: &Attributes) -> Option<f32> {
    attrs.try_value(prefix).filter(|secs| *secs >= 0.0 && secs.is_finite())
}

// ---------------------------------------------------------------------------
// Option<T> — resolves inner if Some, no-op if None
// ---------------------------------------------------------------------------

impl<T: AttributeResolvable> AttributeResolvable for Option<T> {
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
        match self {
            Some(inner) => inner.should_resolve(prefix, attrs),
            None => false,
        }
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        if let Some(inner) = self {
            inner.resolve(prefix, attrs);
        }
    }
}

// ---------------------------------------------------------------------------
// Support for `#[derive(AttributeResolvable)]`
// ---------------------------------------------------------------------------

/// One field's full attribute path under a given prefix, and its interned id.
#[doc(hidden)]
pub struct FieldPath {
    pub path: Box<str>,
    pub id: AttributeId,
}

type PathsByPrefix = HashMap<Box<str>, Arc<[FieldPath]>>;

/// The derive's per-type cache of field paths, keyed by prefix.
///
/// A derived type is resolved under a handful of fixed prefixes (the attribute
/// names its components declare), so each prefix's paths are formatted and
/// interned once, the first time it is seen. After that a resolve reads leaf
/// fields by id and hands composite fields a ready-made path: no `format!` and
/// no string lookup per field per frame.
#[doc(hidden)]
pub struct FieldPathCache {
    suffixes: &'static [&'static str],
    by_prefix: OnceLock<RwLock<PathsByPrefix>>,
}

impl FieldPathCache {
    pub const fn new(suffixes: &'static [&'static str]) -> Self {
        Self { suffixes, by_prefix: OnceLock::new() }
    }

    /// The paths of every field under `prefix`, in declaration order.
    pub fn get(&self, prefix: &str) -> Arc<[FieldPath]> {
        let map = self.by_prefix.get_or_init(Default::default);
        if let Some(paths) = map.read().unwrap_or_else(PoisonError::into_inner).get(prefix) {
            return paths.clone();
        }
        let paths: Arc<[FieldPath]> = self
            .suffixes
            .iter()
            .map(|suffix| {
                let path = format!("{prefix}{suffix}");
                let id = AttributeId(global_rodeo().get_or_intern(&path));
                FieldPath { path: path.into_boxed_str(), id }
            })
            .collect();
        map.write()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(prefix.into())
            .or_insert(paths)
            .clone()
    }
}

/// Whether a float field differs from its attribute value, with the same
/// relative tolerance the hand-written terminal impls use.
#[doc(hidden)]
#[inline]
pub fn float_differs(field: f32, value: f32) -> bool {
    !crate::expr::approx_eq(field, value)
}
