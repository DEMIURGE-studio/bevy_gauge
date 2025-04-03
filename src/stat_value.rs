use crate::expressions::Expression;
use crate::prelude::{ValueBounds, ValueType};
use evalexpr::{
    ContextWithMutableVariables, HashMapContext, Value as EvalValue,
};
use std::collections::{HashMap, HashSet};
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

    pub fn extract_dependencies(&self) -> Option<Vec<(String, String)>> {
        let bound_deps = if let Some(bounds) = &self.bounds {
            bounds.extract_dependencies()
        } else {
            None
        };
        let deps = &self.value.extract_dependencies();

        if bound_deps.is_none() && deps.is_none() {
            return None;
        }
        let mut dependencies = Vec::new();

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
            ValueType::Expression(_) => {}
        }
    }

    pub fn get_value_mut(&mut self) -> &mut ValueType {
        &mut self.value
    }

    pub fn update_value_with_context(&mut self, value_context: &HashMap<String, f32>) {
        // Collect all variable names/values for both the main expression
        // and any expressions in the bounds
        let mut all_variable_names = HashSet::new();

        // Collect variable names from main expression
        if let ValueType::Expression(expression) = &self.value {
            all_variable_names.extend(
                expression
                    .iter_variable_identifiers()
                    .map(|s| s.to_string()),
            );
        }

        // Collect variable names from bounds if they exist
        if let Some(bounds) = &self.bounds {
            if let Some(ValueType::Expression(min_expr)) = &bounds.min {
                all_variable_names
                    .extend(min_expr.iter_variable_identifiers().map(|s| s.to_string()));
            }

            if let Some(ValueType::Expression(max_expr)) = &bounds.max {
                all_variable_names
                    .extend(max_expr.iter_variable_identifiers().map(|s| s.to_string()));
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
            let is_cyclic = EVAL_STACK.with(|stack| stack.borrow().contains(&var_name));

            if is_cyclic {
                variable_values.insert(var_name, 0.0); // Break cycles
                continue;
            }

            // Add to stack to detect cycles
            EVAL_STACK.with(|stack| stack.borrow_mut().insert(var_name.clone()));

            // Parse the variable name to get group and tag ID or name
            let parts: Vec<&str> = var_name.split('.').collect();
            if parts.len() != 2 {
                variable_values.insert(var_name.clone(), 0.0); // Default for invalid variable names
                EVAL_STACK.with(|stack| stack.borrow_mut().remove(&var_name));
                continue;
            }

            let group = parts[0];
            let tag_str = parts[1];

            // If we have a valid tag ID, try to get the attribute value
            let val = match value_context.get(format!("{}.{}", group, tag_str).as_str()) {
                Some(&value) => value as f64,
                None => 0.0,
            };

            variable_values.insert(var_name.clone(), val);

            // Remove from stack
            EVAL_STACK.with(|stack| stack.borrow_mut().remove(&var_name));
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
