use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

use crate::attributes_mut::AttributesMut;
use crate::node::ReduceFn;
use crate::tags::TagMask;

// ---------------------------------------------------------------------------
// AttributeBuilder trait
// ---------------------------------------------------------------------------

/// Trait for structural attribute operations that run during initialization.
///
/// Builders set up attribute structure (nodes, expressions, dependencies)
/// before modifier values are applied. Unlike modifier entries, builders
/// are not reversible via [`ModifierSet::remove`].
///
/// Implement this for custom attribute setup patterns. bevy_gauge provides
/// [`ComplexAttribute`] as a built-in builder.
pub trait AttributeBuilder: Send + Sync {
    /// Apply this builder's operations to the given entity.
    fn apply(&self, entity: Entity, attributes: &mut AttributesMut);

    /// Clone this builder into a boxed trait object.
    fn clone_box(&self) -> Box<dyn AttributeBuilder>;

    /// Format this builder for debug output.
    fn fmt_debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

impl Clone for Box<dyn AttributeBuilder> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl std::fmt::Debug for Box<dyn AttributeBuilder> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_debug(f)
    }
}

// ---------------------------------------------------------------------------
// ComplexAttribute builder
// ---------------------------------------------------------------------------

/// A builder that creates a complex attribute with named parts and an expression.
///
/// When applied, this creates part nodes with the specified reduce functions
/// and wires up an expression modifier on the parent attribute.
///
/// # Example
///
/// ```ignore
/// let builder = ComplexAttribute::new("Damage",
///     &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
///     "base * (1 + increased)",
/// );
/// ```
#[derive(Clone, Debug)]
pub struct ComplexAttribute {
    pub name: String,
    pub parts: Vec<(String, ReduceFn)>,
    pub expression: String,
}

impl ComplexAttribute {
    pub fn new(name: &str, parts: &[(&str, ReduceFn)], expression: &str) -> Self {
        Self {
            name: name.to_string(),
            parts: parts.iter().map(|(n, r)| (n.to_string(), r.clone())).collect(),
            expression: expression.to_string(),
        }
    }
}

impl AttributeBuilder for ComplexAttribute {
    fn apply(&self, entity: Entity, attributes: &mut AttributesMut) {
        let parts: Vec<(&str, ReduceFn)> = self.parts
            .iter()
            .map(|(n, r)| (n.as_str(), r.clone()))
            .collect();
        let _ = attributes.complex_attribute(entity, &self.name, &parts, &self.expression);
    }

    fn clone_box(&self) -> Box<dyn AttributeBuilder> {
        Box::new(self.clone())
    }

    fn fmt_debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

// ---------------------------------------------------------------------------
// ModifierValue
// ---------------------------------------------------------------------------

/// How a modifier value is stored before application.
///
/// - `Literal` values become `Modifier::Flat` when applied.
/// - `ExprSource` values are compiled to `Modifier::Expr` when applied (at
///   which point the `Interner` and `TagResolver` are available).
#[derive(Clone, Debug)]
pub enum ModifierValue {
    /// A constant f32 value.
    Literal(f32),
    /// An expression source string to be compiled at apply time.
    ExprSource(String),
}

impl From<f32> for ModifierValue {
    fn from(val: f32) -> Self {
        ModifierValue::Literal(val)
    }
}

impl From<&str> for ModifierValue {
    fn from(s: &str) -> Self {
        ModifierValue::ExprSource(s.to_string())
    }
}

impl From<String> for ModifierValue {
    fn from(s: String) -> Self {
        ModifierValue::ExprSource(s)
    }
}

/// A single entry in a [`ModifierSet`].
#[derive(Clone, Debug)]
pub struct ModifierEntry {
    /// The attribute path (e.g., `"Damage.Added"`).
    pub attribute: String,
    /// The modifier value - either a literal or an expression source string.
    pub value: ModifierValue,
    /// Tag mask for the modifier. `TagMask::NONE` means global.
    pub tag: TagMask,
}

// ---------------------------------------------------------------------------
// ModifierSet
// ---------------------------------------------------------------------------

/// A portable collection of modifiers and builders that can be applied to an entity.
///
/// Build one manually or via the [`attributes!`] / [`mod_set!`] macros.
/// Apply it to an entity by spawning it as [`AttributeInitializer`] or by
/// calling [`apply`](Self::apply) directly with an [`AttributesMut`].
///
/// # Example
///
/// ```ignore
/// let mut set = ModifierSet::new();
/// set.add("Strength", 50.0);
/// set.add_tagged("Damage.Added", 25.0, FIRE | MELEE);
/// set.add_expr("Health", "Strength * 2.0");
/// set.add_builder(ComplexAttribute::new("Damage",
///     &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
///     "base * (1 + increased)",
/// ));
/// set.apply(entity, &mut attributes);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ModifierSet {
    pub(crate) entries: Vec<ModifierEntry>,
    pub(crate) builders: Vec<Box<dyn AttributeBuilder>>,
}

impl ModifierSet {
    /// Create a new empty modifier set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an untagged modifier (literal f32 or expression string).
    pub fn add(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(ModifierEntry {
            attribute: attribute.to_string(),
            value: value.into(),
            tag: TagMask::NONE,
        });
    }

    /// Add a tagged modifier (literal f32 or expression string).
    pub fn add_tagged(&mut self, attribute: &str, value: impl Into<ModifierValue>, tag: TagMask) {
        self.entries.push(ModifierEntry {
            attribute: attribute.to_string(),
            value: value.into(),
            tag,
        });
    }

    /// Add an untagged expression modifier from a source string.
    pub fn add_expr(&mut self, attribute: &str, expr_source: &str) {
        self.add(
            attribute,
            ModifierValue::ExprSource(expr_source.to_string()),
        );
    }

    /// Add a tagged expression modifier from a source string.
    pub fn add_expr_tagged(&mut self, attribute: &str, expr_source: &str, tag: TagMask) {
        self.add_tagged(
            attribute,
            ModifierValue::ExprSource(expr_source.to_string()),
            tag,
        );
    }

    /// Add an [`AttributeBuilder`] for structural attribute setup.
    ///
    /// Builders run before modifier entries during [`apply_all`](Self::apply_all),
    /// so attribute structure (nodes, expressions) is in place before values flow in.
    pub fn add_builder(&mut self, builder: impl AttributeBuilder + 'static) {
        self.builders.push(Box::new(builder));
    }

    /// Run all builders on an entity. Called before modifier entries so that
    /// attribute structure is wired up before values are applied.
    pub fn apply_builders(&self, entity: Entity, attributes: &mut AttributesMut) {
        for builder in &self.builders {
            builder.apply(entity, attributes);
        }
    }

    /// Apply all modifiers in this set to an entity via `AttributesMut`.
    ///
    /// Literal values are added as flat modifiers. Expression strings are
    /// compiled and added as expression modifiers (compilation errors are
    /// silently ignored - use `try_apply` for error handling).
    ///
    /// **Note:** this does not run builders. The [`AttributeInitializer`]
    /// observer calls [`apply_builders`](Self::apply_builders) before this
    /// method automatically. If calling manually, use [`apply_all`](Self::apply_all).
    pub fn apply<F: QueryFilter>(&self, entity: Entity, attributes: &mut AttributesMut<'_, '_, F>) {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    attributes.add_modifier_tagged(entity, &entry.attribute, *val, entry.tag);
                }
                ModifierValue::ExprSource(src) => {
                    if entry.tag.is_empty() {
                        let _ = attributes.add_expr_modifier(entity, &entry.attribute, src);
                    } else {
                        let _ = attributes.add_expr_modifier_tagged(
                            entity,
                            &entry.attribute,
                            src,
                            entry.tag,
                        );
                    }
                }
            }
        }
    }

    /// Apply all modifiers, returning errors for any expression compilation failures.
    pub fn try_apply<F: QueryFilter>(
        &self,
        entity: Entity,
        attributes: &mut AttributesMut<'_, '_, F>,
    ) -> Result<(), crate::expr::CompileError> {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    attributes.add_modifier_tagged(entity, &entry.attribute, *val, entry.tag);
                }
                ModifierValue::ExprSource(src) => {
                    if entry.tag.is_empty() {
                        attributes.add_expr_modifier(entity, &entry.attribute, src)?;
                    } else {
                        attributes.add_expr_modifier_tagged(
                            entity,
                            &entry.attribute,
                            src,
                            entry.tag,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Convenience: run builders then apply modifiers in one call.
    ///
    /// Equivalent to calling [`apply_builders`](Self::apply_builders) followed
    /// by [`apply`](Self::apply). Only works with unfiltered `AttributesMut`.
    pub fn apply_all(&self, entity: Entity, attributes: &mut AttributesMut) {
        self.apply_builders(entity, attributes);
        self.apply(entity, attributes);
    }

    /// Remove all modifiers in this set from an entity via `AttributesMut`.
    ///
    /// This is the inverse of [`apply`](Self::apply). Literal values are removed
    /// as flat modifiers. Expression strings are recompiled and removed as
    /// expression modifiers (compilation errors are silently ignored).
    ///
    /// Builders are not reversed - they define structure, not removable modifiers.
    pub fn remove<F: QueryFilter>(
        &self,
        entity: Entity,
        attributes: &mut AttributesMut<'_, '_, F>,
    ) {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    let modifier = crate::modifier::Modifier::Flat(*val);
                    attributes.remove_modifier_tagged(
                        entity,
                        &entry.attribute,
                        &modifier,
                        entry.tag,
                    );
                }
                ModifierValue::ExprSource(src) => {
                    if let Ok(expr) =
                        crate::expr::Expr::compile(src, Some(attributes.tag_resolver()))
                    {
                        let modifier = crate::modifier::Modifier::Expr(expr);
                        attributes.remove_modifier_tagged(
                            entity,
                            &entry.attribute,
                            &modifier,
                            entry.tag,
                        );
                    }
                }
            }
        }
    }

    /// Remove all modifiers, returning errors for any expression compilation failures.
    pub fn try_remove<F: QueryFilter>(
        &self,
        entity: Entity,
        attributes: &mut AttributesMut<'_, '_, F>,
    ) -> Result<(), crate::expr::CompileError> {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    let modifier = crate::modifier::Modifier::Flat(*val);
                    attributes.remove_modifier_tagged(
                        entity,
                        &entry.attribute,
                        &modifier,
                        entry.tag,
                    );
                }
                ModifierValue::ExprSource(src) => {
                    let expr = crate::expr::Expr::compile(src, Some(attributes.tag_resolver()))?;
                    let modifier = crate::modifier::Modifier::Expr(expr);
                    attributes.remove_modifier_tagged(
                        entity,
                        &entry.attribute,
                        &modifier,
                        entry.tag,
                    );
                }
            }
        }
        Ok(())
    }

    /// Append all entries and builders from another modifier set into this one.
    pub fn combine(&mut self, other: &ModifierSet) {
        self.entries.extend(other.entries.iter().cloned());
        self.builders.extend(other.builders.iter().map(|b| b.clone_box()));
    }

    /// Number of modifier entries in this set (excludes builders).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether this set has no entries and no builders.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.builders.is_empty()
    }
}

// ---------------------------------------------------------------------------
// AttributeInitializer
// ---------------------------------------------------------------------------

/// A component that carries a [`ModifierSet`] to be applied on spawn.
///
/// When this component is added to an entity that also has [`Attributes`],
/// the builders and modifiers are automatically applied via an observer,
/// and the `AttributeInitializer` component is removed.
///
/// # Example
///
/// ```ignore
/// commands.spawn((
///     Attributes::new(),
///     AttributeInitializer::new(my_modifier_set),
/// ));
/// ```
///
/// Or with the [`attributes!`] macro:
///
/// ```ignore
/// commands.spawn((
///     Attributes::new(),
///     attributes! {
///         "Strength" => 50.0,
///         "Health" => "Strength * 2.0",
///     },
/// ));
/// ```
#[derive(Component, Clone, Debug)]
#[require(crate::prelude::Attributes)]
pub struct AttributeInitializer(pub ModifierSet);

impl AttributeInitializer {
    /// Create a new `AttributeInitializer` from a modifier set.
    pub fn new(set: ModifierSet) -> Self {
        Self(set)
    }
}

/// Observer that applies `AttributeInitializer` when the component is added.
pub(crate) fn apply_initial_attributes(
    trigger: On<Add, AttributeInitializer>,
    initial_query: Query<&AttributeInitializer>,
    mut attributes: AttributesMut,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok(initial) = initial_query.get(entity) {
        initial.0.apply_builders(entity, &mut attributes);
        initial.0.apply(entity, &mut attributes);
    }
    // Remove the component now that it's been applied
    commands.entity(entity).remove::<AttributeInitializer>();
}
