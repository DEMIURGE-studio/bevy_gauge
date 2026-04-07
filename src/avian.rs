//! `AttributeDerived` implementations for avian3d physics components.
//!
//! Enabled by the `avian3d` feature. Reads the `"Mass"` attribute and syncs
//! it to `avian3d::prelude::Mass`.

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use avian3d::prelude::Mass;

use crate::attributes::Attributes;
use crate::derived::{AttributeDerived, InitTo};
use crate::prelude::AttributesMut;

impl AttributeDerived for Mass {
    fn should_update(&self, attrs: &Attributes) -> bool {
        (self.0 - attrs.value("Mass")).abs() > f32::EPSILON
    }

    fn update_from_attributes(&mut self, attrs: &Attributes) {
        self.0 = attrs.value("Mass");
    }
}

impl InitTo for Mass {
    fn init_to_attributes<F: QueryFilter>(&self, entity: Entity, attributes: &mut AttributesMut<'_, '_, F>) {
        attributes.set(entity, "Mass", self.0);
    }
}

crate::register_derived!(Mass);
crate::register_init_to!(Mass);
