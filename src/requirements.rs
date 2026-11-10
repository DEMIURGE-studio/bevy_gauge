//! Boolean attribute requirements - expressions evaluated against an entity's
//! [`Attributes`] that gate state-machine transitions, equipment prerequisites,
//! ability conditions, etc.
//!
//! # Example
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_gauge::prelude::*;
//! # fn setup(mut commands: Commands) {
//! // As a component on a state-machine edge:
//! commands.spawn((
//!     requires! { "ProjectileLife <= 0" },
//!     // ...
//! ));
//! # }
//! # fn check(attribute_requirements: &AttributeRequirements, attrs: &Attributes) {
//! // Checking requirements in a system:
//! if attribute_requirements.met(attrs) {
//!     // all requirements satisfied
//! }
//! # }
//! ```

use std::sync::OnceLock;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;

use crate::attributes::Attributes;
use crate::expr::{Dependency, Expr};
use crate::tags::TagResolver;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single boolean requirement over attributes.
///
/// The expression is compiled once. When [`AttributeRequirements`] is added
/// to an entity, a component hook compiles it against the app's
/// [`TagResolver`], so `{TAG}` syntax works. A requirement that was never
/// compiled that way compiles itself on its first [`met`](Self::met) call,
/// without tag support. An expression that fails to compile logs its error
/// once and is never met.
#[derive(Clone, Debug)]
pub struct AttributeRequirement {
    source: String,
    /// `None` inside once compilation was attempted and failed.
    compiled: OnceLock<Option<Expr>>,
}

impl AttributeRequirement {
    /// Create a new requirement from an expression string.
    ///
    /// The expression is stored but not compiled until
    /// [`compile_with`](Self::compile_with) (typically from the component
    /// hook), [`compile`](Self::compile), or its first evaluation.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            compiled: OnceLock::new(),
        }
    }

    #[cfg(test)]
    fn compiled(source: impl Into<String>) -> Self {
        let mut req = Self::new(source);
        req.compile();
        req
    }

    /// Compile the expression without a tag resolver (no `{TAG}` syntax).
    /// Does nothing if it was already compiled.
    pub fn compile(&mut self) {
        self.compiled.get_or_init(|| compile_source(&self.source, None));
    }

    /// Compile the expression against `resolver`, so it can use `{TAG}`
    /// syntax. Does nothing if it was already compiled. Called by the
    /// component hook.
    pub fn compile_with(&mut self, resolver: &TagResolver) {
        self.compiled
            .get_or_init(|| compile_source(&self.source, Some(resolver)));
    }

    /// Check whether this requirement is satisfied. Compiles the expression
    /// (without tag support) if nothing has compiled it yet.
    ///
    /// `{TAG}` queries are answered by [`Attributes::get_tagged`], so they
    /// work whether or not anything registered the query on that entity.
    pub fn met(&self, attrs: &Attributes) -> bool {
        let Some(expr) = self.compiled.get_or_init(|| compile_source(&self.source, None)) else {
            return false;
        };
        let value = expr.evaluate_with(|id| {
            let tag_query = expr.dependencies().iter().find_map(|dep| match dep {
                Dependency::TagQuery { attribute, mask, synthetic } if *synthetic == id => {
                    Some((*attribute, *mask))
                }
                _ => None,
            });
            match tag_query {
                Some((attribute, mask)) => attrs.get_tagged(attribute, mask),
                None => attrs.context.get(id),
            }
        });
        value != 0.0
    }

    /// Get the source expression string.
    pub fn source(&self) -> &str {
        &self.source
    }
}

fn compile_source(source: &str, resolver: Option<&TagResolver>) -> Option<Expr> {
    match Expr::compile(source, resolver) {
        Ok(expr) => Some(expr),
        Err(err) => {
            warn!("AttributeRequirement '{source}' failed to compile ({err}); it will never be met");
            None
        }
    }
}

impl<S: Into<String>> From<S> for AttributeRequirement {
    fn from(value: S) -> Self {
        Self::new(value)
    }
}

// ---------------------------------------------------------------------------
// AttributeRequirements component
// ---------------------------------------------------------------------------

/// A component holding one or more boolean attribute requirements.
///
/// All requirements must be satisfied for [`met`](Self::met) to return `true`.
/// Expressions are compiled against the app's [`TagResolver`] by a component
/// hook when this is added to an entity.
#[derive(Component, Debug, Default, Clone)]
#[component(on_add = compile_requirements_hook)]
pub struct AttributeRequirements(pub Vec<AttributeRequirement>);

fn compile_requirements_hook(mut world: DeferredWorld, ctx: HookContext) {
    // Compile against a borrowed resolver, then store: the component can't be
    // borrowed mutably while the resource is borrowed.
    let compiled: Vec<Option<Option<Expr>>> = {
        let Some(reqs) = world.get::<AttributeRequirements>(ctx.entity) else {
            return;
        };
        let resolver = world.get_resource::<TagResolver>();
        reqs.0
            .iter()
            .map(|req| {
                req.compiled
                    .get()
                    .is_none()
                    .then(|| compile_source(&req.source, resolver))
            })
            .collect()
    };
    let Some(mut reqs) = world.get_mut::<AttributeRequirements>(ctx.entity) else {
        return;
    };
    for (req, compiled) in reqs.0.iter_mut().zip(compiled) {
        if let Some(compiled) = compiled {
            let _ = req.compiled.set(compiled);
        }
    }
}

impl AttributeRequirements {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a requirement from an expression string.
    pub fn add(&mut self, expr: impl Into<String>) {
        self.0.push(AttributeRequirement::new(expr));
    }

    /// Merge requirements from another set into this one.
    pub fn combine(&mut self, other: &AttributeRequirements) {
        self.0.extend(other.0.iter().cloned());
    }

    /// Compile each [`AttributeRequirement`] without a tag resolver.
    pub fn compile(&mut self) {
        for req in self.0.iter_mut() {
            req.compile();
        }
    }

    /// Compile each [`AttributeRequirement`] against `resolver`.
    pub fn compile_with(&mut self, resolver: &TagResolver) {
        for req in self.0.iter_mut() {
            req.compile_with(resolver);
        }
    }

    /// Check whether **all** requirements are satisfied.
    ///
    /// Returns `true` if the list is empty (vacuous truth).
    pub fn met(&self, attrs: &Attributes) -> bool {
        self.0.iter().all(|req| req.met(attrs))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<I, S> From<I> for AttributeRequirements
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    fn from(value: I) -> Self {
        Self(value.into_iter().map(AttributeRequirement::new).collect())
    }
}

// ---------------------------------------------------------------------------
// requires! macro
// ---------------------------------------------------------------------------

/// Create a [`AttributeRequirements`] component from one or more expression strings.
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_gauge::prelude::*;
/// let one = requires! { "ProjectileLife <= 0" };
/// let all = requires! { "Strength >= 10", "Level >= 5" };
/// # assert_eq!((one.len(), all.len()), (1, 2));
/// ```
#[macro_export]
macro_rules! requires {
    { $( $expr:literal ),* $(,)? } => {{
        let mut _reqs = $crate::requirements::AttributeRequirements::new();
        $(
            _reqs.add($expr);
        )*
        _reqs
    }};
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_id::Interner;
    use crate::modifier::Modifier;
    use crate::node::ReduceFn;

    fn test_interner() -> Interner {
        // First call creates and sets the global; subsequent calls are no-ops.
        let i = Interner::new();
        i.set_global();
        Interner::global()
    }

    fn make_attrs(interner: &Interner, attributes: &[(&str, f32)]) -> Attributes {
        let mut attrs = Attributes::new();
        for &(name, value) in attributes {
            let id = interner.get_or_intern(name);
            let node = attrs.ensure_node(id, ReduceFn::Sum);
            node.add_modifier(Modifier::Flat(value));
            attrs.evaluate_and_cache(id);
        }
        attrs
    }

    #[test]
    fn single_requirement_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 25.0)]);
        let req = AttributeRequirement::compiled("Strength >= 10");
        assert!(req.met(&attrs));
    }

    #[test]
    fn single_requirement_not_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 5.0)]);
        let req = AttributeRequirement::compiled("Strength >= 10");
        assert!(!req.met(&attrs));
    }

    #[test]
    fn requirements_all_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 25.0), ("Level", 10.0)]);
        let reqs = {
            let mut reqs = AttributeRequirements::from(vec!["Strength >= 10", "Level >= 5"]);
            reqs.compile();
            reqs
        };
        assert!(reqs.met(&attrs));
    }

    #[test]
    fn requirements_partial_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 25.0), ("Level", 3.0)]);
        let reqs = {
            let mut reqs = AttributeRequirements::from(vec!["Strength >= 10", "Level >= 5"]);
            reqs.compile();
            reqs
        };
        assert!(!reqs.met(&attrs));
    }

    #[test]
    fn empty_requirements_are_met() {
        let attrs = Attributes::new();
        let reqs = AttributeRequirements::new();
        assert!(reqs.met(&attrs));
    }

    #[test]
    fn le_zero_check() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("ProjectileLife", 0.0)]);
        let req = AttributeRequirement::compiled("ProjectileLife <= 0");
        assert!(req.met(&attrs));
    }

    #[test]
    fn combine_merges() {
        let a = AttributeRequirements::from(vec!["A >= 1"]);
        let b = AttributeRequirements::from(vec!["B >= 2", "C >= 3"]);
        let mut combined = a;
        combined.combine(&b);
        assert_eq!(combined.len(), 3);
    }
}
