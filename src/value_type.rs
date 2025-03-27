use std::collections::{HashMap, HashSet};
use bevy::prelude::{Deref, DerefMut};
use bevy_mod_debugdump::print_render_graph;
use evalexpr::{
    ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use crate::stats::StatCollection;

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
        let bound_deps = if let Some(bounds) = &self.bounds {
            bounds.extract_bound_dependencies()
        } else { None };
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

   pub fn set_value_with_context(&mut self, stats: &StatCollection) {
    // We need to collect all variable names/values for both the main expression
    // and any expressions in the bounds
    let mut all_variable_names = HashSet::new();
    
    // Collect variable names from main expression
    if let ValueType::Expression(expression) = &self.value {
        all_variable_names.extend(
            expression.iter_variable_identifiers()
                .map(|s| s.to_string())
        );
    }
    
    // Collect variable names from bounds if they exist
    if let Some(bounds) = &self.bounds {
        if let Some(min_bound) = &bounds.min {
            if let ValueType::Expression(min_expr) = min_bound {
                all_variable_names.extend(
                    min_expr.iter_variable_identifiers()
                        .map(|s| s.to_string())
                );
            }
        }
        
        if let Some(max_bound) = &bounds.max {
            if let ValueType::Expression(max_expr) = max_bound {
                all_variable_names.extend(
                    max_expr.iter_variable_identifiers()
                        .map(|s| s.to_string())
                );
            }
        }
    }
    
    // If we have no expressions to evaluate, we can return early
    if all_variable_names.is_empty() && !matches!(self.value, ValueType::Expression(_)) {
        return;
    }
    
    // Use thread-local to track stack for cycle detection
    thread_local! {
        static EVAL_STACK: std::cell::RefCell<HashSet<String>> = 
            std::cell::RefCell::new(HashSet::new());
    }
    
    // Collect all variable values once
    let mut variable_values = HashMap::new();
    for var_name in all_variable_names {
        let is_cyclic = EVAL_STACK.with(|stack| {
            stack.borrow().contains(&var_name)
        });
        
        let val = if is_cyclic {
            0.0 // Break cycles
        } else {
            // Add to stack to detect cycles
            EVAL_STACK.with(|stack| {
                stack.borrow_mut().insert(var_name.clone())
            });
            
            // Get value safely without recursive mutable borrowing
            let result = match stats.stats.get(&var_name) {
                Some(stat_instance) => stat_instance.stat.get_value(),
                None => 0.0,
            };
            
            // Remove from stack
            EVAL_STACK.with(|stack| {
                stack.borrow_mut().remove(&var_name)
            });
            
            result as f64
        };
        
        variable_values.insert(var_name, val);
    }
    
    // Create evaluation context once with all collected variables
    let mut context = HashMapContext::new();
    for (name, value) in variable_values {
        context
            .set_value(name, EvalValue::from_float(value))
            .unwrap_or_default();
    }
    
    // First, evaluate and update bounds using the same context
    let mut min_value = f32::MIN;
    let mut max_value = f32::MAX;
    
    if let Some(bounds) = &mut self.bounds {
        // Evaluate min bound if it's an expression and update its cached value
        if let Some(ValueType::Expression(min_expr)) = &mut bounds.min {
            let min_result = min_expr
                .eval_with_context_mut(&mut context)
                .unwrap_or(EvalValue::from_float(f64::MIN))
                .as_number()
                .unwrap_or(f64::MIN as f64) as f32;
                
            // Critically, update the cached value in the expression
            min_expr.cached_value = min_result;
            min_value = min_result;
        } else if let Some(ValueType::Literal(val)) = &bounds.min {
            min_value = *val;
        }
        
        // Evaluate max bound if it's an expression and update its cached value
        if let Some(ValueType::Expression(max_expr)) = &mut bounds.max {
            let max_result = max_expr
                .eval_with_context_mut(&mut context)
                .unwrap_or(EvalValue::from_float(f64::MAX))
                .as_number()
                .unwrap_or(f64::MAX as f64) as f32;
                
            // Critically, update the cached value in the expression
            max_expr.cached_value = max_result;
            max_value = max_result;
        } else if let Some(ValueType::Literal(val)) = &bounds.max {
            max_value = *val;
        }
    }
    
    // Now evaluate the main expression and apply bounds
    if let ValueType::Expression(expression) = &mut self.value {
        // Evaluate expression with the prepared context
        let result = expression
            .eval_with_context_mut(&mut context)
            .unwrap_or(EvalValue::from_float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32;
            
        // Apply the evaluated bounds and update cached value
        expression.cached_value = result.clamp(min_value, max_value);
    }
    // For literal values with bounds, apply the bounds
    else if let ValueType::Literal(ref mut val) = self.value {
        *val = val.clamp(min_value, max_value);
    }
}
    
    pub fn get_value_f32(&self) -> f32 {
        let val = self.value.evaluate();
        let (min, max) = self.get_bounds();
        
        val.clamp(min, max)
        
    }
    
    pub fn get_bounds(&self) -> (f32, f32) {
        let mut val_bounds = (f32::MIN, f32::MAX);
        
        if let Some(bounds) = &self.bounds {
            if let Some(min_bound) = &bounds.min {
                val_bounds.0 = min_bound.evaluate();
            }

            if let Some(max_bound) = &bounds.max {
                val_bounds.1 = max_bound.evaluate();
            }
        }
        
        val_bounds
    }
    
    pub fn set_bounds(&mut self, bounds: Option<ValueBounds>) {
        self.bounds = bounds;
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


