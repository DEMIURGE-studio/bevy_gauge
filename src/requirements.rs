use bevy::prelude::*;
use serde::Deserialize;
use crate::prelude::*;
use evalexpr::{ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue};

// ------------------------------------------------------------------
//  Example comparison for "requirements"
// ------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StatRequirement(pub Node<DefaultNumericTypes>);

impl StatRequirement {
    pub fn met(&self, stats: &StatContextRefs) -> bool {
        let mut context = HashMapContext::new();

        // Gather variable references from the expression
        for var in self.0.iter_variable_identifiers() {
            // If the referenced stat is missing, we'll default to 0.0
            let var_value = stats.get(var).unwrap_or(0.0);
            let _ = context.set_value(var.into(), EvalValue::from_float(var_value as f64));
        }
        match self.0.eval_boolean_with_context(&context) {
            Ok(result) => return result,
            Err(err) => println!("{:#?}", err),
        }
        false
    }
}

impl From<String> for StatRequirement {
    fn from(value: String) -> Self {
        let expr = evalexpr::build_operator_tree(&value).unwrap();
        Self(expr)
    }
}

impl From<&str> for StatRequirement {
    fn from(value: &str) -> Self {
        let expr = evalexpr::build_operator_tree(&value).unwrap();
        Self(expr)
    }
}

#[derive(Component, Debug, Default, Clone, Deserialize)]
#[require(StatContext)]
pub struct StatRequirements(pub Vec<StatRequirement>);

impl StatRequirements {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Merges in constraints from another set
    pub fn combine(&mut self, other: &StatRequirements) {
        self.0.append(&mut other.0.clone());
    }

    /// Returns true if all constraints hold.
    pub fn met(&self, stats: &StatContextRefs) -> bool {
        for req in self.0.iter() {
            if !req.met(stats) {
                return false;
            }
        }

        return true;
    }
}

impl From<Vec<String>> for StatRequirements {
    fn from(value: Vec<String>) -> Self {
        let mut result: Vec<StatRequirement> = Vec::new();
        for string in value {
            result.push(string.into())
        }
        Self(result)
    }
}