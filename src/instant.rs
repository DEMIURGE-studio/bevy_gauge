use bevy::prelude::*;
use evalexpr::{ContextWithMutableVariables, Value};

use crate::prelude::{Expression, ModifierType, StatPath, Stats, StatsMutator};

/// A compiled collection of one-shot operations to apply directly to stats.
///
/// This is a data container; evaluation and application are performed elsewhere.
#[derive(Component, Clone, Debug, Default)]
pub struct InstantModifierSet(pub(crate) Vec<InstantEntry>);

#[derive(Clone, Debug)]
pub struct InstantEntry {
    pub path: String,
    pub op: InstantOp,
    pub value: ModifierType,
}

#[derive(Clone, Debug)]
pub enum InstantOp { Set, Add, Sub }

impl InstantModifierSet {
    pub fn add_set<V: Into<ModifierType>>(&mut self, path: &str, value: V) {
        self.0.push(InstantEntry { path: path.to_string(), op: InstantOp::Set, value: value.into() });
    }

    pub fn add_add<V: Into<ModifierType>>(&mut self, path: &str, value: V) {
        self.0.push(InstantEntry { path: path.to_string(), op: InstantOp::Add, value: value.into() });
    }

    pub fn add_sub<V: Into<ModifierType>>(&mut self, path: &str, value: V) {
        self.0.push(InstantEntry { path: path.to_string(), op: InstantOp::Sub, value: value.into() });
    }
}

// ------- Role-based expression evaluation helpers -------

pub type RoleMap<'a> = &'a [(&'a str, Entity)];

fn resolve_identifier_value(
    identifier: &str,
    roles: RoleMap,
    q_stats: &Query<&Stats>,
    default_role: &str,
) -> f32 {
    let path = StatPath::parse(identifier);
    let role_key = path.target().unwrap_or(default_role);
    let stat_name = path.name();

    let Some((_, entity)) = roles.iter().find(|(k, _)| *k == role_key) else { return 0.0 };
    q_stats.get(*entity).unwrap().get(stat_name)
}

/// Evaluate with role-based stats and optional extra constants supplied via context.
/// Use the context for pipeline-stage values like \"initialHit\".
pub fn evaluate_expression_with_roles_ctx(
    expr: &Expression,
    roles: RoleMap,
    q_stats: &Query<&Stats>,
    default_role: &str,
    context: Option<&[(&str, f32)]>,
) -> f32 {
    let mut ctx = evalexpr::HashMapContext::new();

    // Populate identifiers from stats
    for var in expr.compiled.iter_identifiers() {
        let value = resolve_identifier_value(var, roles, q_stats, default_role) as f64;
        let _ = ctx.set_value(var.to_string(), Value::Float(value));
    }

    // Overlay explicit context last (takes precedence if keys collide)
    if let Some(pairs) = context {
        for (k, v) in pairs {
            let _ = ctx.set_value((*k).to_string(), Value::Float(*v as f64));
        }
    }

    expr.evaluate(&ctx)
}

/// Back-compat helper without extra context.
pub fn evaluate_expression_with_roles(
    expr: &Expression,
    roles: RoleMap,
    q_stats: &Query<&Stats>,
    default_role: &str,
) -> f32 {
    evaluate_expression_with_roles_ctx(expr, roles, q_stats, default_role, None)
}

// ------- Instant evaluation and application -------

#[derive(Clone, Debug)]
pub struct EvaluatedInstantEntry {
    pub path: String,
    pub op: InstantOp,
    pub value: f32,
}

/// Evaluate an `Instant` into concrete numeric operations using a role-aware context.
/// Returns a snapshot of entries that can be applied without further evaluation.
pub fn evaluate_instant_values(
    instant: &InstantModifierSet,
    roles: RoleMap,
    sm: &mut StatsMutator,
    default_role: &str,
) -> Vec<EvaluatedInstantEntry> {
    let mut out: Vec<EvaluatedInstantEntry> = Vec::with_capacity(instant.0.len());

    for entry in instant.0.iter() {
        let value = match &entry.value {
            ModifierType::Literal(v) => *v,
            ModifierType::Expression(expr) => {
                sm.with_stats_query(|q_stats| {
                    evaluate_expression_with_roles(expr, roles, &q_stats, default_role)
                })
            }
        };

        out.push(EvaluatedInstantEntry {
            path: entry.path.clone(),
            op: entry.op.clone(),
            value,
        });
    }

    out
}

/// Apply previously evaluated instant operations to a specific entity via `StatsMutator`.
/// Set overwrites the stat at `path`; Add/Sub read the current value and then set the new value.
pub fn apply_evaluated_instant_to_entity(
    evaluated: &[EvaluatedInstantEntry],
    sm: &mut crate::prelude::StatsMutator,
    target_entity: Entity,
) {
    for op in evaluated.iter() {
        match op.op {
            InstantOp::Set => {
                sm.set(target_entity, &op.path, op.value);
            }
            InstantOp::Add => {
                let current = sm.get(target_entity, &op.path);
                sm.set(target_entity, &op.path, current + op.value);
            }
            InstantOp::Sub => {
                let current = sm.get(target_entity, &op.path);
                sm.set(target_entity, &op.path, current - op.value);
            }
        }
    }
}

/// Convenience helper to evaluate and immediately apply an `Instant` for a single target entity.
pub fn apply_instant(
    instant: &InstantModifierSet,
    roles: RoleMap,
    default_role: &str,
    sm: &mut crate::prelude::StatsMutator,
    target_entity: Entity,
) {
    let evaluated = evaluate_instant_values(instant, roles, sm, default_role);
    apply_evaluated_instant_to_entity(&evaluated, sm, target_entity);
}
