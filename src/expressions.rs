use evalexpr::{Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};
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
    pub(crate) cached_stat_paths: Vec<CachedStatPath>,
}

#[derive(Debug, Clone)]
pub struct CachedStatPath {
    pub original: String,
    pub name: String,
    pub part: Option<String>,
    pub tag: Option<u32>,
    pub target: Option<String>,
}

impl Expression {
    /// Creates a new `Expression` by parsing and compiling an expression string.
    ///
    /// # Arguments
    ///
    /// * `expression`: A string slice representing the mathematical expression
    ///                 (e.g., `"Life.base + Vitality * 10"`).
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
        
        // Build a cache of parsed stat paths for variable identifiers.
        let mut cached_stat_paths = Vec::new();
        for var_name in compiled.iter_variable_identifiers() {
            let path = StatPath::parse(var_name);
            cached_stat_paths.push(CachedStatPath {
                original: var_name.to_string(),
                name: path.name().to_string(),
                part: path.part().map(|s| s.to_string()),
                tag: path.tag(),
                target: path.target().map(|s| s.to_string()),
            });
        }

        Ok(Self {
            definition: expression.to_string(),
            compiled,
            cached_stat_paths,
        })
    }

    /// Returns the cached, pre-parsed stat paths for this expression's identifiers.
    pub fn stat_paths(&self) -> &[CachedStatPath] {
        &self.cached_stat_paths
    }

    pub fn evaluate(&self, context: &HashMapContext) -> f32 {
        self.try_evaluate(context).unwrap_or(0.0)
    }

    /// Fallible evaluation that uses the compiled expression without reparsing.
    ///
    /// - Clones the provided context
    /// - Ensures all identifiers referenced by this expression exist in the context, defaulting to 0.0
    /// - Returns a `StatResult<f32>` with detailed `StatError` on failure
    pub(crate) fn try_evaluate(&self, base_context: &HashMapContext) -> StatResult<f32> {
        let mut context = base_context.clone();

        // Ensure all variables referenced by this expression are present in the context
        for var_name in self.compiled.iter_variable_identifiers() {
            if context.get_value(var_name).is_none() {
                context
                    .set_value(var_name.to_string(), Value::Float(0.0))
                    .map_err(|e| StatError::Internal { details: format!("Failed to set default value for missing variable '{}': {}", var_name, e) })?;
            }
        }

        let eval_value = self
            .compiled
            .eval_with_context(&context)
            .map_err(|e| StatError::ExpressionError { expression: self.definition.clone(), details: e.to_string() })?;

        Ok(eval_value.as_number().unwrap_or(0.0) as f32)
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
        Self::Expression(Expression::new(value).unwrap())
    }
}

impl From<String> for ModifierType {
    fn from(value: String) -> Self {
        Self::Expression(Expression::new(&value).unwrap())
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