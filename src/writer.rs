//! Entity-bound attribute mutation trait.
//!
//! [`AttributeWriter`] defines the mutation API for a single entity's attributes.
//! Unlike [`AttributesMut`], which requires an entity parameter on every call,
//! `AttributeWriter` is bound to a specific entity - methods operate on that
//! entity implicitly.
//!
//! This trait is implemented by [`BoundAttributesMut`] (wraps a real
//! `AttributesMut` + entity) and is used in deferred contexts like
//! [`commands.entity(e).attrs(|attrs| { ... })`](crate::commands::AttributeCommandsExt).
//!
//! Library code that should work in both direct and deferred contexts can be
//! generic over `impl AttributeWriter`.

use bevy::prelude::*;

use crate::attributes::Attributes;
use crate::expr::CompileError;
use crate::modifier::Modifier;
use crate::node::ReduceFn;
use crate::tags::TagMask;

/// Entity-bound attribute mutation API.
///
/// All methods operate on a single entity that was bound at construction time.
/// This is the trait to implement for custom attribute backends and to use as
/// a generic bound in library code.
pub trait AttributeWriter {
    // ── Modifiers ────────────────────────────────────────────────────────

    /// Add an untagged modifier.
    fn add_modifier(&mut self, attr: &str, modifier: impl Into<Modifier>);

    /// Add a tagged modifier.
    fn add_modifier_tagged(&mut self, attr: &str, modifier: impl Into<Modifier>, tag: TagMask);

    /// Add an untagged modifier with a specific reduce function.
    fn add_modifier_with_reduce(&mut self, attr: &str, modifier: impl Into<Modifier>, reduce: ReduceFn);

    /// Add a tagged modifier with a specific reduce function.
    fn add_modifier_tagged_with_reduce(&mut self, attr: &str, modifier: impl Into<Modifier>, tag: TagMask, reduce: ReduceFn);

    /// Add an untagged expression modifier.
    fn add_expr_modifier(&mut self, attr: &str, expr: &str) -> Result<(), CompileError>;

    /// Add a tagged expression modifier.
    fn add_expr_modifier_tagged(
        &mut self,
        attr: &str,
        expr: &str,
        tag: TagMask,
    ) -> Result<(), CompileError>;

    /// Remove an untagged modifier by value.
    fn remove_modifier(&mut self, attr: &str, modifier: &Modifier);

    /// Remove a tagged modifier by value and tag.
    fn remove_modifier_tagged(&mut self, attr: &str, modifier: &Modifier, tag: TagMask);

    // ── Base value operations ────────────────────────────────────────────

    /// Set a flat modifier on an attribute (untagged).
    fn set(&mut self, attr: &str, value: f32);

    /// Set a tagged flat modifier on an attribute.
    fn set_tagged(&mut self, attr: &str, value: f32, tag: TagMask);

    /// Replace all untagged flat modifiers on an attribute with a single value.
    fn set_base(&mut self, attr: &str, value: f32);

    /// Replace all flat modifiers with a specific tag.
    fn set_base_tagged(&mut self, attr: &str, value: f32, tag: TagMask);

    // ── Attribute constructors ───────────────────────────────────────────

    /// Create a flat attribute (Sum-reducing node with one flat modifier).
    fn flat_attribute(&mut self, name: &str, value: f32);

    /// Create a complex attribute with named parts combined by an expression.
    fn complex_attribute(
        &mut self,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expr: &str,
    ) -> Result<(), CompileError>;

    /// Create a tagged attribute with lazy template materialization.
    fn tagged_attribute(
        &mut self,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expr: &str,
    ) -> Result<(), CompileError>;

    // ── Cross-entity sources ─────────────────────────────────────────────

    /// Register a cross-entity source alias.
    fn register_source(&mut self, alias: &str, source: Entity);

    /// Unregister a source alias.
    fn unregister_source(&mut self, alias: &str);

    // ── Cross-entity queries ────────────────────────────────────────────

    /// Look up which entity a source alias points to.
    fn resolve_source(&self, alias: &str) -> Option<Entity>;

    // ── Reading ──────────────────────────────────────────────────────────

    /// Get read-only access to the entity's [`Attributes`] component.
    fn get_attributes(&self) -> Option<&Attributes>;

    /// Read an attribute's cached value. Returns 0.0 if it doesn't exist.
    fn value(&self, attr: &str) -> f32;

    /// Re-evaluate a known attribute, returning `None` if the name was never interned.
    fn try_evaluate(&mut self, attr: &str) -> Option<f32>;

    /// Force re-evaluation and return the value.
    fn evaluate(&mut self, attr: &str) -> f32;

    /// Evaluate with a tag filter.
    fn evaluate_tagged(&mut self, attr: &str, query: TagMask) -> f32;
}

/// Wraps an [`AttributesMut`] reference bound to a specific entity.
///
/// Created by [`AttributeCommandsExt::attrs`](crate::commands::AttributeCommandsExt)
/// or manually for use in generic code.
pub struct BoundAttributesMut<'a, 'w, 's, F: bevy::ecs::query::QueryFilter + 'static = ()> {
    pub(crate) entity: Entity,
    pub(crate) attrs: &'a mut crate::attributes_mut::AttributesMut<'w, 's, F>,
}

impl<F: bevy::ecs::query::QueryFilter + 'static> AttributeWriter for BoundAttributesMut<'_, '_, '_, F> {
    fn add_modifier(&mut self, attr: &str, modifier: impl Into<Modifier>) {
        self.attrs.add_modifier(self.entity, attr, modifier);
    }

    fn add_modifier_tagged(&mut self, attr: &str, modifier: impl Into<Modifier>, tag: TagMask) {
        self.attrs.add_modifier_tagged(self.entity, attr, modifier, tag);
    }

    fn add_modifier_with_reduce(&mut self, attr: &str, modifier: impl Into<Modifier>, reduce: ReduceFn) {
        self.attrs.add_modifier_with_reduce(self.entity, attr, modifier, reduce);
    }

    fn add_modifier_tagged_with_reduce(&mut self, attr: &str, modifier: impl Into<Modifier>, tag: TagMask, reduce: ReduceFn) {
        self.attrs.add_modifier_tagged_with_reduce(self.entity, attr, modifier, tag, reduce);
    }

    fn add_expr_modifier(&mut self, attr: &str, expr: &str) -> Result<(), CompileError> {
        self.attrs.add_expr_modifier(self.entity, attr, expr)
    }

    fn add_expr_modifier_tagged(
        &mut self,
        attr: &str,
        expr: &str,
        tag: TagMask,
    ) -> Result<(), CompileError> {
        self.attrs.add_expr_modifier_tagged(self.entity, attr, expr, tag)
    }

    fn remove_modifier(&mut self, attr: &str, modifier: &Modifier) {
        self.attrs.remove_modifier(self.entity, attr, modifier);
    }

    fn remove_modifier_tagged(&mut self, attr: &str, modifier: &Modifier, tag: TagMask) {
        self.attrs.remove_modifier_tagged(self.entity, attr, modifier, tag);
    }

    fn set(&mut self, attr: &str, value: f32) {
        self.attrs.set(self.entity, attr, value);
    }

    fn set_tagged(&mut self, attr: &str, value: f32, tag: TagMask) {
        self.attrs.set_tagged(self.entity, attr, value, tag);
    }

    fn set_base(&mut self, attr: &str, value: f32) {
        self.attrs.set_base(self.entity, attr, value);
    }

    fn set_base_tagged(&mut self, attr: &str, value: f32, tag: TagMask) {
        self.attrs.set_base_tagged(self.entity, attr, value, tag);
    }

    fn flat_attribute(&mut self, name: &str, value: f32) {
        self.attrs.flat_attribute(self.entity, name, value);
    }

    fn complex_attribute(
        &mut self,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expr: &str,
    ) -> Result<(), CompileError> {
        self.attrs.complex_attribute(self.entity, name, parts, expr)
    }

    fn tagged_attribute(
        &mut self,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expr: &str,
    ) -> Result<(), CompileError> {
        self.attrs.tagged_attribute(self.entity, name, parts, expr)
    }

    fn register_source(&mut self, alias: &str, source: Entity) {
        self.attrs.register_source(self.entity, alias, source);
    }

    fn unregister_source(&mut self, alias: &str) {
        self.attrs.unregister_source(self.entity, alias);
    }

    fn resolve_source(&self, alias: &str) -> Option<Entity> {
        self.attrs.resolve_source(self.entity, alias)
    }

    fn get_attributes(&self) -> Option<&Attributes> {
        self.attrs.get_attributes(self.entity)
    }

    fn value(&self, attr: &str) -> f32 {
        self.attrs.value(self.entity, attr)
    }

    fn try_evaluate(&mut self, attr: &str) -> Option<f32> {
        self.attrs.try_evaluate(self.entity, attr)
    }

    fn evaluate(&mut self, attr: &str) -> f32 {
        self.attrs.evaluate(self.entity, attr)
    }

    fn evaluate_tagged(&mut self, attr: &str, query: TagMask) -> f32 {
        self.attrs.evaluate_tagged(self.entity, attr, query)
    }
}
