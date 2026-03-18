//! Deferred attribute operations via entity commands.
//!
//! Provides [`AttributeCommandsExt`], an extension trait on [`EntityCommands`]
//! that adds an `attrs()` method. The closure receives a [`BoundAttributesMut`]
//! bound to the entity, backed by the real [`AttributesMut`] system parameter
//! via Bevy's [`SystemState`].
//!
//! This solves the fundamental problem of needing expression modifiers,
//! complex attributes, and other `AttributesMut` operations at entity spawn
//! time, when only `Commands` is available.
//!
//! # Example
//!
//! ```ignore
//! commands.entity(e).attrs(|attrs| {
//!     attrs.flat_attribute("Strength", 50.0);
//!     attrs.add_expr_modifier("MaxHealth", "Strength * 2.0 + 100.0").unwrap();
//!     attrs.complex_attribute(
//!         "Damage",
//!         &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
//!         "base * (1 + increased)",
//!     ).unwrap();
//! });
//! ```

use bevy::ecs::system::SystemState;
use bevy::ecs::world::EntityWorldMut;
use bevy::prelude::*;

use crate::attributes_mut::AttributesMut;
use crate::writer::BoundAttributesMut;

/// Extension trait on [`EntityCommands`] for deferred attribute operations.
pub trait AttributeCommandsExt {
    /// Queue attribute operations on this entity, executed when commands flush.
    ///
    /// The closure receives a [`BoundAttributesMut`] bound to the entity,
    /// backed by the real `AttributesMut` system parameter. All operations
    /// (expression modifiers, complex attributes, source registration, etc.)
    /// are available and execute with full dependency tracking.
    fn attrs(&mut self, f: impl FnOnce(&mut BoundAttributesMut) + Send + 'static) -> &mut Self;
}

impl AttributeCommandsExt for EntityCommands<'_> {
    fn attrs(&mut self, f: impl FnOnce(&mut BoundAttributesMut) + Send + 'static) -> &mut Self {
        self.queue(AttrsEntityCommand { f: Box::new(f) });
        self
    }
}

/// An entity command that runs a closure with `BoundAttributesMut` access.
struct AttrsEntityCommand {
    f: Box<dyn FnOnce(&mut BoundAttributesMut) + Send + 'static>,
}

impl EntityCommand for AttrsEntityCommand {
    fn apply(self, entity_world: EntityWorldMut<'_>) {
        let entity = entity_world.id();
        let world = entity_world.into_world_mut();
        let mut state = SystemState::<AttributesMut>::new(world);
        let mut attrs_mut = state.get_mut(world);
        let mut bound = BoundAttributesMut {
            entity,
            attrs: &mut attrs_mut,
        };
        (self.f)(&mut bound);
        state.apply(world);
    }
}
