use std::collections::HashMap;
use crate::expressions::Expression;
use evalexpr::{
    ContextWithMutableVariables, HashMapContext, Value as EvalValue,
};

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


impl ValueType {
    pub fn from_float(val: f32) -> ValueType {
        ValueType::Literal(val)
    }

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn get_value(&self) -> f32 {
        match self {
            ValueType::Literal(val) => *val,
            ValueType::Expression(expr) => expr.cached_value,
        }
    }
    
    pub fn evaluate_expression(&mut self, value_context: &HashMapContext) {
        match self {
            ValueType::Expression(expr) => {
                let result = expr
                    .eval_with_context(value_context)
                    .unwrap_or(EvalValue::from_float(0.0))
                    .as_number()
                    .unwrap_or(0.0) as f32;
                expr.cached_value = result;
            }
            _ => return
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
