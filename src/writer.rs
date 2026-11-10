//! Entity-bound attribute mutation.
//!
//! [`BoundAttributesMut`] is the mutation API for a single entity's attributes.
//! Unlike [`AttributesMut`](crate::attributes_mut::AttributesMut), which
//! requires an entity parameter on every call, it is bound to one entity, and
//! methods operate on that entity implicitly. It is what
//! [`commands.entity(e).attrs(|attrs| { ... })`](crate::commands::AttributeCommandsExt)
//! hands its closure, and can be built directly with [`BoundAttributesMut::new`].

use bevy::prelude::*;

use crate::attributes::Attributes;
use crate::expr::CompileError;
use crate::modifier::Modifier;
use crate::node::ReduceFn;
use crate::tags::TagMask;

/// An [`AttributesMut`](crate::attributes_mut::AttributesMut) bound to one
/// entity: every method operates on that entity.
///
/// Created by [`AttributeCommandsExt::attrs`](crate::commands::AttributeCommandsExt)
/// or with [`new`](Self::new).
pub struct BoundAttributesMut<'a, 'w, 's, F: bevy::ecs::query::QueryFilter + 'static = ()> {
    pub(crate) entity: Entity,
    pub(crate) attrs: &'a mut crate::attributes_mut::AttributesMut<'w, 's, F>,
}

impl<'a, 'w, 's, F: bevy::ecs::query::QueryFilter + 'static> BoundAttributesMut<'a, 'w, 's, F> {
    /// Bind `attrs` to `entity`.
    pub fn new(entity: Entity, attrs: &'a mut crate::attributes_mut::AttributesMut<'w, 's, F>) -> Self {
        Self { entity, attrs }
    }

    /// The entity these operations apply to.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    // ── Modifiers ────────────────────────────────────────────────────────

    /// Add an untagged modifier.
    pub fn add_modifier(&mut self, attr: &str, modifier: impl Into<Modifier>) {
        self.attrs.add_modifier(self.entity, attr, modifier);
    }

    /// Add a tagged modifier.
    pub fn add_modifier_tagged(&mut self, attr: &str, modifier: impl Into<Modifier>, tag: TagMask) {
        self.attrs.add_modifier_tagged(self.entity, attr, modifier, tag);
    }

    /// Add an untagged modifier with a specific reduce function.
    pub fn add_modifier_with_reduce(&mut self, attr: &str, modifier: impl Into<Modifier>, reduce: ReduceFn) {
        self.attrs.add_modifier_with_reduce(self.entity, attr, modifier, reduce);
    }

    /// Add a tagged modifier with a specific reduce function.
    pub fn add_modifier_tagged_with_reduce(&mut self, attr: &str, modifier: impl Into<Modifier>, tag: TagMask, reduce: ReduceFn) {
        self.attrs.add_modifier_tagged_with_reduce(self.entity, attr, modifier, tag, reduce);
    }

    /// Add an untagged expression modifier.
    pub fn add_expr_modifier(&mut self, attr: &str, expr: &str) -> Result<(), CompileError> {
        self.attrs.add_expr_modifier(self.entity, attr, expr)
    }

    /// Add a tagged expression modifier.
    pub fn add_expr_modifier_tagged(
        &mut self,
        attr: &str,
        expr: &str,
        tag: TagMask,
    ) -> Result<(), CompileError> {
        self.attrs.add_expr_modifier_tagged(self.entity, attr, expr, tag)
    }

    /// Remove an untagged modifier by value.
    pub fn remove_modifier(&mut self, attr: &str, modifier: &Modifier) {
        self.attrs.remove_modifier(self.entity, attr, modifier);
    }

    /// Remove a tagged modifier by value and tag.
    pub fn remove_modifier_tagged(&mut self, attr: &str, modifier: &Modifier, tag: TagMask) {
        self.attrs.remove_modifier_tagged(self.entity, attr, modifier, tag);
    }


    // ── Base value operations ────────────────────────────────────────────

    /// Set a flat modifier on an attribute (untagged).
    pub fn set(&mut self, attr: &str, value: f32) {
        self.attrs.set(self.entity, attr, value);
    }

    /// Set a tagged flat modifier on an attribute.
    pub fn set_tagged(&mut self, attr: &str, value: f32, tag: TagMask) {
        self.attrs.set_tagged(self.entity, attr, value, tag);
    }

    /// Replace all untagged flat modifiers on an attribute with a single value.
    pub fn set_base(&mut self, attr: &str, value: f32) {
        self.attrs.set_base(self.entity, attr, value);
    }

    /// Replace all flat modifiers with a specific tag.
    pub fn set_base_tagged(&mut self, attr: &str, value: f32, tag: TagMask) {
        self.attrs.set_base_tagged(self.entity, attr, value, tag);
    }


    // ── Attribute constructors ───────────────────────────────────────────

    /// Create a flat attribute (Sum-reducing node with one flat modifier).
    pub fn flat_attribute(&mut self, name: &str, value: f32) {
        self.attrs.flat_attribute(self.entity, name, value);
    }

    /// Create a complex attribute with named parts combined by an expression.
    pub fn complex_attribute(
        &mut self,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expr: &str,
    ) -> Result<(), CompileError> {
        self.attrs.complex_attribute(self.entity, name, parts, expr)
    }

    /// Create a tagged attribute with lazy template materialization.
    pub fn tagged_attribute(
        &mut self,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expr: &str,
    ) -> Result<(), CompileError> {
        self.attrs.tagged_attribute(self.entity, name, parts, expr)
    }


    // ── Cross-entity sources ─────────────────────────────────────────────

    /// Register a cross-entity source alias.
    pub fn register_source(&mut self, alias: &str, source: Entity) {
        self.attrs.register_source(self.entity, alias, source);
    }

    /// Unregister a source alias.
    pub fn unregister_source(&mut self, alias: &str) {
        self.attrs.unregister_source(self.entity, alias);
    }


    // ── Cross-entity queries ────────────────────────────────────────────

    /// Look up which entity a source alias points to.
    pub fn resolve_source(&self, alias: &str) -> Option<Entity> {
        self.attrs.resolve_source(self.entity, alias)
    }


    // ── Reading ──────────────────────────────────────────────────────────

    /// Get read-only access to the entity's [`Attributes`] component.
    pub fn get_attributes(&self) -> Option<&Attributes> {
        self.attrs.get_attributes(self.entity)
    }

    /// Read an attribute's cached value. Returns 0.0 if it doesn't exist.
    pub fn value(&self, attr: &str) -> f32 {
        self.attrs.value(self.entity, attr)
    }

    /// Re-evaluate a known attribute, returning `None` if the name was never interned.
    pub fn try_evaluate(&mut self, attr: &str) -> Option<f32> {
        self.attrs.try_evaluate(self.entity, attr)
    }

    /// Force re-evaluation and return the value.
    pub fn evaluate(&mut self, attr: &str) -> f32 {
        self.attrs.evaluate(self.entity, attr)
    }

    /// Evaluate with a tag filter.
    pub fn evaluate_tagged(&mut self, attr: &str, query: TagMask) -> f32 {
        self.attrs.evaluate_tagged(self.entity, attr, query)
    }
}
