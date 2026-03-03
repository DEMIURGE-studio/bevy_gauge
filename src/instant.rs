//! One-shot attribute mutations — Set, Add, or Subtract a value once without
//! leaving a persistent modifier on the attribute node.
//!
//! [`InstantModifierSet`] is a portable collection of [`InstantEntry`] ops
//! that can be attached as a component (e.g., on ability effect entities) and
//! applied when triggered.
//!
//! # Role-based evaluation
//!
//! Expression values can reference attributes on **role entities** via the `@role`
//! syntax (e.g., `"Strength@attacker * 0.5"`). Roles are temporary source
//! aliases registered on the target entity for the duration of evaluation.
//!
//! # Example
//!
//! ```ignore
//! let instant = instant! {
//!     "Scorch" += 1.0,
//!     "Doom" += "-Doom@target",
//!     "ProjectileLife" -= 1.0,
//! };
//! apply_instant(&instant, &roles, defender, &mut attributes);
//! ```

use bevy::prelude::*;

use crate::attributes_mut::AttributesMut;
use crate::modifier_set::ModifierValue;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// The operation to perform on a attribute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstantOp {
    /// Overwrite the attribute's value.
    Set,
    /// Add to the attribute's current value.
    Add,
    /// Subtract from the attribute's current value.
    Sub,
}

/// A single entry in an [`InstantModifierSet`].
#[derive(Clone, Debug)]
pub struct InstantEntry {
    /// The attribute path (e.g., `"Damage.base"`, `"Scorch"`).
    pub attribute: String,
    /// Which operation to perform.
    pub op: InstantOp,
    /// The value — either a literal f32 or an expression source string that
    /// is compiled at apply time.
    pub value: ModifierValue,
}

/// A portable collection of one-shot attribute operations.
///
/// Unlike [`ModifierSet`](crate::modifier_set::ModifierSet), which adds
/// persistent modifiers to attribute nodes, `InstantModifierSet` applies its
/// operations once and does not leave any modifiers behind.
///
/// Build one with the [`instant!`](crate::instant!) macro or the builder
/// methods, then apply it via [`apply_instant`].
#[derive(Component, Clone, Debug, Default)]
pub struct InstantModifierSet {
    pub(crate) entries: Vec<InstantEntry>,
}

impl InstantModifierSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a **Set** operation (overwrites the attribute value).
    pub fn add_set(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(InstantEntry {
            attribute: attribute.to_string(),
            op: InstantOp::Set,
            value: value.into(),
        });
    }

    /// Add an **Add** operation (adds to the current attribute value).
    pub fn add_add(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(InstantEntry {
            attribute: attribute.to_string(),
            op: InstantOp::Add,
            value: value.into(),
        });
    }

    /// Add a **Sub** operation (subtracts from the current attribute value).
    pub fn add_sub(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(InstantEntry {
            attribute: attribute.to_string(),
            op: InstantOp::Sub,
            value: value.into(),
        });
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Evaluated entries
// ---------------------------------------------------------------------------

/// An [`InstantEntry`] after expression evaluation — holds a concrete f32.
#[derive(Clone, Debug)]
pub struct EvaluatedInstantEntry {
    pub attribute: String,
    pub op: InstantOp,
    pub value: f32,
}

// ---------------------------------------------------------------------------
// Role map
// ---------------------------------------------------------------------------

/// A slice of `(role_name, entity)` pairs used during role-based evaluation.
///
/// Example: `&[("attacker", attacker_entity), ("defender", defender_entity)]`
pub type RoleMap<'a> = &'a [(&'a str, Entity)];

// ---------------------------------------------------------------------------
// Evaluation & application
// ---------------------------------------------------------------------------

/// Evaluate all entries in an [`InstantModifierSet`] into concrete f32 values.
///
/// Roles are registered as temporary source aliases on `target_entity` so
/// that expressions like `"Strength@attacker"` resolve correctly. Aliases
/// are cleaned up after evaluation.
pub fn evaluate_instant(
    instant: &InstantModifierSet,
    roles: RoleMap,
    target_entity: Entity,
    attributes: &mut AttributesMut,
) -> Vec<EvaluatedInstantEntry> {
    // Register temporary source aliases for each role
    for &(role_name, role_entity) in roles {
        attributes.register_source(target_entity, role_name, role_entity);
    }

    let mut out = Vec::with_capacity(instant.entries.len());

    for entry in &instant.entries {
        let value = match &entry.value {
            ModifierValue::Literal(v) => *v,
            ModifierValue::ExprSource(src) => {
                let interner = attributes.interner();
                let expr = crate::expr::Expr::compile_with_tags(
                    src,
                    &interner,
                    Some(attributes.tag_resolver()),
                );
                match expr {
                    Ok(compiled) => {
                        // Evaluate against the target entity's context
                        match attributes.get_attributes(target_entity) {
                            Some(attrs) => compiled.evaluate(&attrs.context),
                            None => 0.0,
                        }
                    }
                    Err(_) => 0.0,
                }
            }
        };

        out.push(EvaluatedInstantEntry {
            attribute: entry.attribute.clone(),
            op: entry.op.clone(),
            value,
        });
    }

    // Clean up temporary aliases
    for &(role_name, _) in roles {
        attributes.unregister_source(target_entity, role_name);
    }

    out
}

/// Apply previously evaluated instant operations to a specific entity.
pub fn apply_evaluated_instant(
    evaluated: &[EvaluatedInstantEntry],
    target_entity: Entity,
    attributes: &mut AttributesMut,
) {
    for entry in evaluated {
        match entry.op {
            InstantOp::Set => {
                attributes.set_base(target_entity, &entry.attribute, entry.value);
            }
            InstantOp::Add => {
                let current = attributes.evaluate(target_entity, &entry.attribute);
                attributes.set_base(target_entity, &entry.attribute, current + entry.value);
            }
            InstantOp::Sub => {
                let current = attributes.evaluate(target_entity, &entry.attribute);
                attributes.set_base(target_entity, &entry.attribute, current - entry.value);
            }
        }
    }
}

/// Evaluate and immediately apply an [`InstantModifierSet`] to a target entity.
///
/// This is a convenience wrapper around [`evaluate_instant`] +
/// [`apply_evaluated_instant`].
pub fn apply_instant(
    instant: &InstantModifierSet,
    roles: RoleMap,
    target_entity: Entity,
    attributes: &mut AttributesMut,
) {
    let evaluated = evaluate_instant(instant, roles, target_entity, attributes);
    apply_evaluated_instant(&evaluated, target_entity, attributes);
}

// ---------------------------------------------------------------------------
// instant! macro
// ---------------------------------------------------------------------------

/// Create an [`InstantModifierSet`] from a declarative list of operations.
///
/// # Syntax
///
/// ```ignore
/// instant! {
///     "AttributeName" = value,    // Set
///     "AttributeName" += value,   // Add
///     "AttributeName" -= value,   // Sub
/// }
/// ```
///
/// - **`value`** can be an `f32` literal or a string expression
///   (e.g., `"-Doom@target"`).
///
/// # Example
///
/// ```ignore
/// let effects = instant! {
///     "Scorch" += 1.0,
///     "Doom" += "-Doom@target",
///     "ProjectileLife" -= 1.0,
///     "Health" = "Strength@attacker * 0.5",
/// };
/// ```
#[macro_export]
macro_rules! instant {
    { $( $attribute:literal $op:tt $value:expr ),* $(,)? } => {{
        let mut _set = $crate::instant::InstantModifierSet::new();
        $(
            $crate::instant!(@entry _set, $attribute, $op, $value);
        )*
        _set
    }};

    (@entry $set:ident, $attribute:literal, +=, $value:expr) => {
        $set.add_add($attribute, $value);
    };
    (@entry $set:ident, $attribute:literal, -=, $value:expr) => {
        $set.add_sub($attribute, $value);
    };
    (@entry $set:ident, $attribute:literal, =, $value:expr) => {
        $set.add_set($attribute, $value);
    };
}
