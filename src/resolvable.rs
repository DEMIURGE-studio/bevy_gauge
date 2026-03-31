//! Nested attribute resolution for composite types.
//!
//! [`AttributeResolvable`] allows struct and enum fields to be updated from
//! attributes using dot-separated paths. Terminal types (f32, integers, bool,
//! Duration) read a single attribute value. Composite types delegate to their
//! fields, appending the field name to the prefix.
//!
//! # Path convention
//!
//! Given:
//! ```ignore
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
//! ```ignore
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

use std::time::Duration;

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
        (*self - attrs.value(prefix)).abs() > f32::EPSILON
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        *self = attrs.value(prefix);
    }
}

impl AttributeResolvable for f64 {
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
        (*self - attrs.value(prefix) as f64).abs() > f64::EPSILON
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        *self = attrs.value(prefix) as f64;
    }
}

// ---------------------------------------------------------------------------
// Terminal impls — integers
// ---------------------------------------------------------------------------

macro_rules! impl_resolvable_int {
    ($($ty:ty),*) => {$(
        impl AttributeResolvable for $ty {
            fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
                *self != attrs.value(prefix).round() as $ty
            }

            fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
                *self = attrs.value(prefix).round() as $ty;
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
        *self != (attrs.value(prefix) != 0.0)
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        *self = attrs.value(prefix) != 0.0;
    }
}

// ---------------------------------------------------------------------------
// Terminal impls — Duration
// ---------------------------------------------------------------------------

impl AttributeResolvable for Duration {
    fn should_resolve(&self, prefix: &str, attrs: &Attributes) -> bool {
        let secs = attrs.value(prefix);
        secs > 0.0 && (self.as_secs_f32() - secs).abs() > f32::EPSILON
    }

    fn resolve(&mut self, prefix: &str, attrs: &Attributes) {
        let secs = attrs.value(prefix);
        if secs > 0.0 {
            *self = Duration::from_secs_f32(secs);
        }
    }
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
