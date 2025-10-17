use bevy::prelude::*;
use evalexpr::{ContextWithMutableVariables, Value};

use crate::prelude::{Expression, StatPath, Stats};

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


