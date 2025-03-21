use bevy::prelude::{Deref, DerefMut};
use evalexpr::{
    ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use crate::prelude::StatCollection;
use crate::traits::AsF32;

#[derive(Debug)]
pub enum StatError {
    BadOpp(String),
    NotFound(String),
}

#[derive(Debug, Clone)]
pub enum ValueType {
    Literal(f32),
    Expression(Expression),
}


#[derive(Debug, Clone)]
pub struct ValueBounds {
    min: Option<ValueType>,
    max: Option<ValueType>,
}

impl ValueBounds {
    pub fn new(min: Option<ValueType>, max: Option<ValueType>) -> Self {
        Self { min, max }
    }
    //TODO implement bounds
}

impl ValueType {
    pub fn from_float(val: f32) -> ValueType {
        ValueType::Literal(val)
    }

    pub fn add(&mut self, val: f32) {
        match self {
            ValueType::Literal(current_val) => {
                *current_val += val;
            },
            ValueType::Expression(_) => { },
        }
    }

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self, stat_collection: &StatCollection) -> f32 {
        if let ValueType::Literal(val) = self {
            return *val;
        }

        let ValueType::Expression(expr) = self else {
            return 0.0;
        };

        // Start from base
        let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

        // Health + Armor/3
        // Fill that context with variable identifiers
        for var_name in expr.iter_variable_identifiers() {
            let val = stat_collection.get(var_name).unwrap_or(0.0);
            context
                .set_value(var_name.to_string(), EvalValue::from_float(val as f64))
                .unwrap();
        }

        
        // Evaluate. We just unwrap because:
        //  1. Eval should not fail
        //  2. get_value("Total") should never fail
        //  3. because stat expressions all return number values, as_number should never fail
        expr.eval_with_context_mut(&mut context).unwrap().as_number().unwrap() as f32
        // TODO add some error handling
    }

    pub fn from_expression(value: Expression) -> Self {
        ValueType::Expression(value)
    }
}

impl Default for ValueType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

// impl AsF32 for ValueType {
//     fn to_f32(&self) -> f32 {
//         match self {
//             ValueType::Literal(val) => *val,
//             ValueType::Expression()
//         }
//     }
// }

#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Expression(pub Node<DefaultNumericTypes>);

impl Default for Expression {
    fn default() -> Self {
        Self(evalexpr::build_operator_tree("Total = 0").unwrap())
    }
}



