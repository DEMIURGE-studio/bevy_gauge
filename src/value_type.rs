use std::collections::{HashMap, HashSet};
use bevy::prelude::{Deref, DerefMut};
use evalexpr::{
    ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
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


#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatValue {
    value: ValueType,
    bounds: Option<ValueBounds>,
}

impl StatValue {
    pub fn new(value: ValueType, bounds: Option<ValueBounds>) -> Self {
        Self { value, bounds }
    }
    pub fn from_f32(val: f32) -> Self {
        Self {
            value: ValueType::Literal(val),
            bounds: None,
        }
    }

    pub fn from_expression(expression: Expression) -> Self {
        Self {
            value: ValueType::Expression(expression),
            bounds: None,
        }
    }
    
    pub fn extract_dependencies(&self) -> Option<HashSet<String>> {
        let bound_deps = &self.bounds.clone()?.extract_bound_dependencies();
        let deps = &self.value.extract_dependencies();
        
        if bound_deps.is_none() && deps.is_none() {
            return None;
        }
        let mut dependencies = HashSet::new();
        
        if let Some(bound_deps) = &bound_deps {
            dependencies.extend(bound_deps.clone());
        }
        
        if let Some(deps) = &deps {
            dependencies.extend(deps.clone());
        }
        
        Some(dependencies)
    }
    
    pub fn set_value(&mut self, value: f32) {
        match &self.value { 
            ValueType::Literal(_) => {
                let (min, max) = self.get_bounds();
                self.value = ValueType::Literal(value.clamp(min, max));
            }
            ValueType::Expression(_) => {
            }
        }
    }
    
    pub fn get_value_f32(&self) -> f32 {
        self.value.evaluate()
    }
    
    pub fn get_bounds(&self) -> (f32, f32) {
        let mut val_bounds = (f32::MIN, f32::MAX);
        
        if let Some(bounds) = &self.bounds {
            if let Some(min_bound) = &bounds.min {
                val_bounds.0 = min_bound.evaluate();
            }

            if let Some(max_bound) = &bounds.max {
                val_bounds.0 = max_bound.evaluate();
            }
        }
        
        val_bounds
    }
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
    
    pub fn extract_bound_dependencies(&self) -> Option<HashSet<String>> {
        let mut bound_dependencies = HashSet::new();
        if let Some(min) = &self.min {
            if let Some(min_deps) = min.extract_dependencies(){
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
    
    
    pub fn extract_dependencies(&self) -> Option<HashSet<String>> {
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


impl Default for Expression {
    fn default() -> Self {
        Self {
            expression: evalexpr::build_operator_tree("0").unwrap(),
            cached_value: 0.0,
        }
    }
}

impl Expression {
    pub fn new(node: Node<DefaultNumericTypes>) -> Self {
        Self {
            expression: node,
            cached_value: 0.0
        }
    }

    pub fn extract_dependencies(&self) -> HashSet<String> {
        self.iter_variable_identifiers()
            .map(|val| String::from(val))
            .collect()
    }
}


