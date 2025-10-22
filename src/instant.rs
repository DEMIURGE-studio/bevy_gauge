use bevy::prelude::*;
use evalexpr::{ContextWithMutableVariables, Value};

use crate::prelude::{Expression, ModifierType, StatPath, Stats};

/// A compiled collection of one-shot operations to apply directly to stats.
///
/// This is a data container; evaluation and application are performed elsewhere.
#[derive(Clone, Debug, Default)]
pub struct Instant(pub(crate) Vec<InstantEntry>);

#[derive(Clone, Debug)]
pub struct InstantEntry {
    pub path: String,
    pub op: InstantOp,
    pub value: ModifierType,
}

#[derive(Clone, Debug)]
pub enum InstantOp { Set, Add, Sub }

impl Instant {
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
    let Ok(stats) = q_stats.get(*entity) else { return 0.0 };
    stats.get(stat_name)
}

pub fn evaluate_expression_with_roles(
    expr: &Expression,
    roles: RoleMap,
    q_stats: &Query<&Stats>,
    default_role: &str,
) -> f32 {
    let mut context = evalexpr::HashMapContext::new();

    for var in expr.compiled.iter_identifiers() {
        let value = resolve_identifier_value(var, roles, q_stats, default_role) as f64;
        let _ = context.set_value(var.to_string(), Value::Float(value));
    }

    expr.evaluate(&context)
}


