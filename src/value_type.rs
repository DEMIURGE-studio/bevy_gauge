use std::collections::{HashMap, HashSet};
use bevy::prelude::{Deref, DerefMut};
use evalexpr::{
    ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use crate::stats::StatCollection;
use crate::tags::ValueTag;

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


    // pub fn evaluate_with_additional_ctx<T: AsF32>(&self, stat_collection: &StatCollection, additional_ctx: Option<HashMap<String, T>>) -> f32 {
    //     //TODO fix later and actually implement to allow for supplemental context
    //     if let ValueType::Literal(val) = self {
    //         return *val;
    //     }
    // 
    //     let ValueType::Expression(expr) = self else {
    //         return 0.0;
    //     };
    // 
    //     // Start from base
    //     let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();
    //     
    //     // Health + Armor/3
    //     // Fill that context with variable identifiers
    //     for var_name in expr.iter_variable_identifiers() {
    //         let val = stat_collection.get(var_name).unwrap_or(0.0);
    //         context
    //             .set_value(var_name.to_string(), EvalValue::from_float(val as f64))
    //             .unwrap();
    //     }
    // 
    // 
    //     // Evaluate. We just unwrap because:
    //     //  1. Eval should not fail
    //     //  2. get_value("Total") should never fail
    //     //  3. because stat expressions all return number values, as_number should never fail
    //     expr.eval_with_context_mut(&mut context).unwrap().as_number().unwrap() as f32
    //     // TODO add some error handling
    // }
    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self, stat_collection: &StatCollection) -> f32 {
        if let ValueType::Literal(val) = self {
            return *val;
        }

        let ValueType::Expression(expr) = self else {
            return 0.0;
        };

        // Track visited stats to detect cycles using a thread_local static
        thread_local! {
        static EVAL_STACK: std::cell::RefCell<std::collections::HashSet<ValueTag>> = 
            std::cell::RefCell::new(std::collections::HashSet::new());
    }

        // Start from base
        let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

        // Fill context with variable identifiers
        for var_name in expr.iter_variable_identifiers() {
            // Use the thread_local stack to check for cycles
            let is_cyclic = EVAL_STACK.with(|stack| {
                let tag = ValueTag::parse(var_name).unwrap_or_default();
                stack.borrow().contains(&tag)
            });

            // If we detect a cycle, use 0.0 as the fallback value to break the cycle
            let val = if is_cyclic {
                0.0
            } else {
                // Add this tag to stack before recursively evaluating
                EVAL_STACK.with(|stack| {
                    let tag = ValueTag::parse(var_name).unwrap_or_default();
                    stack.borrow_mut().insert(tag)
                });

                // Get the value recursively
                let result = stat_collection.get(var_name).unwrap_or(0.0);

                // Remove from stack after evaluation
                EVAL_STACK.with(|stack| {
                    let tag = ValueTag::parse(var_name).unwrap_or_default();
                    stack.borrow_mut().remove(&tag)
                });

                result
            };

            // Add to context
            context
                .set_value(var_name.to_string(), EvalValue::from_float(val as f64))
                .unwrap();
        }

        // Evaluate with populated context
        // We use unwrap_or and as_number unwrap_or for error handling
        expr.eval_with_context_mut(&mut context)
            .unwrap_or(EvalValue::from_float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32
    }




    pub fn from_expression(value: Expression) -> Self {
        ValueType::Expression(value)
    }
    
    
    pub fn extract_dependencies(&self) -> Option<HashSet<ValueTag>> {
        match self {
            ValueType::Literal(_) => { None }
            ValueType::Expression(expr) => { Some(expr.extract_dependencies()) }
        }
    }


}

impl Default for ValueType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Deref, DerefMut)]
pub struct Expression(pub Node<DefaultNumericTypes>);

impl Expression {
    pub fn extract_dependencies(&self) -> HashSet<ValueTag> {
        let mut deps = HashSet::new();

        for variable in self.iter_variable_identifiers() {
            if let Ok(variable) = ValueTag::parse(variable) {
                deps.insert(variable);
            }
        }
        deps
    }
}


impl Default for Expression {
    fn default() -> Self {
        Self(evalexpr::build_operator_tree("Total = 0").unwrap())
    }
}


