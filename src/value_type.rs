use bevy::prelude::{Deref, DerefMut};
use std::collections::{HashMap, HashSet};
use crate::expressions::Expression;

#[derive(Debug)]
pub enum StatError {
    BadOpp(String),
    NotFound(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Literal(f32),
    Expression(Expression),
}


// Need to be able to optionally pass in a stat_collection or value to be added to the evalexpression context

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValueBounds {
    pub min: Option<ValueType>,
    pub max: Option<ValueType>,
}

impl ValueBounds {
    pub fn new(min: Option<ValueType>, max: Option<ValueType>) -> Self {
        Self { min, max }
    }

    pub fn extract_dependencies(&self) -> Option<Vec<(String, String)>> {
        let mut bound_dependencies = Vec::new();
        if let Some(min) = &self.min {
            if let Some(min_deps) = min.extract_dependencies() {
                bound_dependencies.extend(min_deps);
            }
        }
        if let Some(max) = &self.max {
            if let Some(max_deps) = max.extract_dependencies() {
                bound_dependencies.extend(max_deps);
            }
        }

        if bound_dependencies.is_empty() {
            None
        } else {
            Some(bound_dependencies)
        }
    }
}

impl ValueType {
    pub fn from_float(val: f32) -> ValueType {
        ValueType::Literal(val)
    }

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self) -> f32 {
        match self {
            ValueType::Literal(val) => *val,
            ValueType::Expression(expr) => expr.cached_value,
        }
    }

    pub fn from_expression(value: Expression) -> Self {
        ValueType::Expression(value)
    }

    pub fn extract_dependencies(&self) -> Option<Vec<(String, String)>> {
        match self {
            ValueType::Literal(_) => None,
            ValueType::Expression(expr) => Some(expr.extract_dependencies()),
        }
    }
}

impl Default for ValueType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

