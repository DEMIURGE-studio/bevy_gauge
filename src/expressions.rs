use evalexpr::{DefaultNumericTypes, HashMapContext, Node, Value};
use crate::prelude::*;

/// Represents a mathematical expression that can be evaluated to calculate a stat value.
///
/// Expressions are defined as strings (e.g., `"base * (1 + increased) * more"`) and are compiled
/// into an internal representation for efficient evaluation. They can reference other stats
/// or fixed values.
///
/// `Expression`s are used for calculating total stat values (see `Config::register_total_expression`)
/// and for creating modifiers that depend on other stats (see `StatsMutator::add_modifier`).
#[derive(Debug, Clone)]
pub struct Expression {
    pub(crate) definition: String,
    pub(crate) compiled: Node<DefaultNumericTypes>,
}

impl Expression {
    /// Creates a new `Expression` by parsing and compiling an expression string.
    ///
    /// # Arguments
    ///
    /// * `expression`: A string slice representing the mathematical expression
    ///                 (e.g., `"Health.base + Vitality * 10"`).
    ///
    /// # Returns
    ///
    /// A `StatResult<Self>` which is `Ok(Expression)` if parsing and compilation are successful,
    /// or `Err(StatError)` if the expression string is invalid.
    pub fn new(expression: &str) -> StatResult<Self> {
        let compiled = evalexpr::build_operator_tree(expression)
            .map_err(|err| StatError::ExpressionError {
                expression: expression.to_string(),
                details: err.to_string(),
            })?;
            
        Ok(Self {
            definition: expression.to_string(),
            compiled,
        })
    }

    pub(crate) fn evaluate(&self, context: &HashMapContext) -> f32 {
        self.compiled
            .eval_with_context(context)
            .unwrap_or(Value::Float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32
    }
}

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        self.definition == other.definition
    }
}

/// Represents the type of a modifier that can be applied to a stat.
///
/// Modifiers can either be a simple literal numerical value or a more complex `Expression`
/// that can reference other stat values.
#[derive(Debug, Clone)]
pub enum ModifierType {
    /// A direct numerical value to be applied as a modifier.
    Literal(f32),
    /// An `Expression` that will be evaluated to determine the modifier's value.
    /// This allows for dynamic modifiers based on other stats (e.g., `"Strength * 0.5"`).
    Expression(Expression),
}

impl Default for ModifierType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

impl From<Expression> for ModifierType {
    fn from(value: Expression) -> Self {
        Self::Expression(value)
    }
}

impl From<&str> for ModifierType {
    fn from(value: &str) -> Self {
        Self::Expression(Expression {
            definition: value.to_string(),
            compiled: evalexpr::build_operator_tree(value).unwrap(),
        })
    }
}

impl From<String> for ModifierType {
    fn from(value: String) -> Self {
        Self::Expression(Expression {
            definition: value.clone(),
            compiled: evalexpr::build_operator_tree(&value).unwrap(),
        })
    }
}

impl From<f32> for ModifierType {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

impl From<u32> for ModifierType {
    fn from(value: u32) -> Self {
        Self::Literal(value as f32)
    }
}

impl From<i32> for ModifierType {
    fn from(value: i32) -> Self {
        Self::Literal(value as f32)
    }
}