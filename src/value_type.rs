use std::collections::{HashSet};
use bevy::prelude::{Deref, DerefMut};
use evalexpr::{
    ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
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

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self) -> f32 {
        if let ValueType::Literal(val) = self {
            return *val;
        }

        let ValueType::Expression(expr) = self else {
            return 0.0;
        };
        
        expr.cached_value
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
pub struct Expression { 
    #[deref]
    pub expression: Node<DefaultNumericTypes>,
    pub cached_value: f32
}


impl Expression {
    pub fn new(node: Node<DefaultNumericTypes>) -> Self {
        Self {
            expression: node,
            cached_value: 0.0
        }
    }
    
    
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
        Self {
            expression: evalexpr::build_operator_tree("0").unwrap(),
            cached_value: 0.0,
        }
    }
}


