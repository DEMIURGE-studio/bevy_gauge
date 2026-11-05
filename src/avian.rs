//! `AttributeDerived` implementations for avian3d physics components.
//!
//! Enabled by the `avian3d` feature. Covers [`Mass`] only: a rigid body with
//! both `Mass` and [`Attributes`] has its `"Mass"` attribute seeded from the
//! component at spawn, and the component follows the attribute afterwards.
//!
//! `Mass` is avian's user-specified mass, which avian never writes, so gauge
//! driving it doesn't fight the engine: avian still folds it, together with
//! collider-density masses, into `ComputedMass`. An entity whose
//! `Attributes` has no `"Mass"` attribute keeps its `Mass` untouched.

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use avian3d::prelude::Mass;

use crate::attribute_id::{global_rodeo, AttributeId};
use crate::attributes::Attributes;
use crate::derived::{AttributeDerived, InitTo};
use crate::prelude::AttributesMut;

/// The `"Mass"` attribute's value, if the entity has one.
fn mass_attribute(attrs: &Attributes) -> Option<f32> {
    let id = AttributeId(global_rodeo().get("Mass")?);
    attrs.has_attribute(id).then(|| attrs.get(id))
}

impl AttributeDerived for Mass {
    fn should_update(&self, attrs: &Attributes) -> bool {
        mass_attribute(attrs).is_some_and(|mass| !crate::expr::approx_eq(self.0, mass))
    }

    fn update_from_attributes(&mut self, attrs: &Attributes) {
        if let Some(mass) = mass_attribute(attrs) {
            self.0 = mass;
        }
    }
}

impl InitTo for Mass {
    fn init_to_attributes<F: QueryFilter>(&self, entity: Entity, attributes: &mut AttributesMut<'_, '_, F>) {
        attributes.set_base(entity, "Mass", self.0);
    }
}

crate::register_derived!(Mass);
crate::register_init_to!(Mass);
