use crate::expressions::Expression;
use crate::prelude::{ValueBounds, ValueType};
use evalexpr::{Context, ContextWithMutableVariables, HashMapContext, Value as EvalValue};
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

    pub fn update_value_with_context(&mut self, value_context: &HashMapContext) {
        // Collect all variable names/values for both the main expression
        // and any expressions in the bounds


        // First, evaluate and update bounds using the same context
        let mut min_value = f32::MIN;
        let mut max_value = f32::MAX;

        if let Some(bounds) = &mut self.bounds {
            // Evaluate min bound if it's an expression and update its cached value
            if let Some(ValueType::Expression(min_expr)) = &mut bounds.min {
                let min_result = min_expr
                    .eval_with_context(value_context)
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
                    .eval_with_context(value_context)
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
        self.value.evaluate_expression(value_context);
        if let ValueType::Expression(expression) = &mut self.value {
            // Apply the evaluated bounds and update cached value
            expression.cached_value = expression.cached_value.clamp(min_value, max_value);
        }
        // For literal values with bounds, apply the bounds
        else if let ValueType::Literal(ref mut val) = self.value {
            *val = val.clamp(min_value, max_value);
        }
    }

    pub fn get_value_f32(&self) -> f32 {
        let val = self.value.get_value();
        let (min, max) = self.get_bounds();

        val.clamp(min, max)
    }

    pub fn get_bounds(&self) -> (f32, f32) {
        let mut val_bounds = (f32::MIN, f32::MAX);

        if let Some(bounds) = &self.bounds {
            if let Some(min_bound) = &bounds.min {
                val_bounds.0 = min_bound.get_value();
            }

            if let Some(max_bound) = &bounds.max {
                val_bounds.1 = max_bound.get_value();
            }
        }

        val_bounds
    }

    pub fn set_bounds(&mut self, bounds: Option<ValueBounds>) {
        self.bounds = bounds;
    }
}
