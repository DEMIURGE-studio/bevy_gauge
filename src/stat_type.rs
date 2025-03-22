use bevy::prelude::{Deref, DerefMut};
use evalexpr::{Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue};

use super::prelude::*;

#[derive(Debug, Clone)]
pub enum StatType {
    Literal(f32),
    Expression(Expression),
}

impl StatType {
    pub fn from_float(val: f32) -> StatType {
        StatType::Literal(val)
    }

    pub fn add(&mut self, val: f32) {
        match self {
            StatType::Literal(ref mut current_val) => {
                *current_val += val;
            },
            StatType::Expression(_) => { },
        }
    }

    pub fn subtract(&mut self, val: f32) {
        match self {
            StatType::Literal(ref mut current_val) => {
                *current_val -= val;
            },
            StatType::Expression(_) => { },
        }
    }

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self, eval_context: &StatContextRefs) -> f32 {
        if let StatType::Literal(val) = self {
            return *val;
        }

        let StatType::Expression(expr)= self else {
            return 0.0;
        };

        // Start from base
        let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

        // Fill that context with variable identifiers
        for var_name in expr.iter_variable_identifiers() {
            // Skip total
            if var_name == "Total" { continue; }

            let val = eval_context.get(var_name).unwrap_or(0.0);
            context
                .set_value(var_name.to_string(), EvalValue::from_float(val as f64))
                .unwrap();
        }
        
        // Evaluate. We just unwrap because:
        //  1. Eval should not fail
        //  2. get_value("Total") should never fail
        //  3. because stat expressions all return number values, as_number should never fail
        expr.eval_with_context_mut(&mut context).unwrap();
        let current_value = (context.get_value("Total").unwrap().as_number().unwrap()) as f32;

        current_value
    }

    pub fn from_expression(value: Expression) -> Self {
        StatType::Expression(value)
    }
}

impl Default for StatType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

impl From<f32> for StatType {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Expression(pub Node<DefaultNumericTypes>);

impl Default for Expression {
    fn default() -> Self {
        Self(evalexpr::build_operator_tree("Total = 0").unwrap())
    }
}