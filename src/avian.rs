//! `AttributeDerived` implementations for avian3d physics components.
//!
//! Enabled by the `avian3d` feature. Reads the `"Mass"` attribute and syncs
//! it to `avian3d::prelude::Mass`.

use avian3d::prelude::Mass;

use crate::attributes::Attributes;
use crate::derived::AttributeDerived;

impl AttributeDerived for Mass {
    fn should_update(&self, attrs: &Attributes) -> bool {
        (self.0 - attrs.value("Mass")).abs() > f32::EPSILON
    }

    fn update_from_attributes(&mut self, attrs: &Attributes) {
        self.0 = attrs.value("Mass");
    }
}

crate::register_derived!(Mass);
