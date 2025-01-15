use bevy_utils::HashMap;
use evalexpr::{DefaultNumericTypes, Node};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use crate::prelude::*;

impl From<f32> for Expression {
    fn from(value: f32) -> Self {
        Expression::from_float(value)
    }
}

impl From<f64> for Expression {
    fn from(value: f64) -> Self {
        Expression::from_float(value as f32)
    }
}

impl From<i32> for Expression {
    fn from(value: i32) -> Self {
        Expression::from_float(value as f32)
    }
}

impl From<i64> for Expression {
    fn from(value: i64) -> Self {
        Expression::from_float(value as f32)
    }
}

impl From<u32> for Expression {
    fn from(value: u32) -> Self {
        Expression::from_float(value as f32)
    }
}

impl From<u64> for Expression {
    fn from(value: u64) -> Self {
        Expression::from_float(value as f32)
    }
}

/// Turn a single part into an Expression with `base=0, parts=[that one part]`
impl From<ExpressionPart> for Expression {
    fn from(val: ExpressionPart) -> Self {
        Expression {
            base: 0.0,
            parts: vec![val],
        }
    }
}

pub struct ExprWrapper(Node<DefaultNumericTypes>);

impl From<Node<DefaultNumericTypes>> for ExprWrapper {
    fn from(value: Node<DefaultNumericTypes>) -> Self {
        Self(value)
    }
}

impl From<&str> for ExprWrapper {
    fn from(value: &str) -> Self {
        Self(evalexpr::build_operator_tree(("Total ".to_string() + value).as_str()).unwrap())
    }
}

impl From<String> for ExprWrapper {
    fn from(value: String) -> Self {
        Self(evalexpr::build_operator_tree(("Total ".to_string() + &value).as_str()).unwrap())
    }
}

impl<'de> Deserialize<'de> for ExprWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // A Visitor that tries to parse numeric types or strings
        struct ExprWrapperVisitor;

        impl<'de> Visitor<'de> for ExprWrapperVisitor {
            type Value = ExprWrapper;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a float or a string containing a formula")
            }

            // If we see a string, treat it as a formula
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value.into())
            }

            // If we see a string, treat it as a formula
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value.into())
            }
        }

        deserializer.deserialize_any(ExprWrapperVisitor)
    }
}

impl From<ExprWrapper> for Node<DefaultNumericTypes> {
    fn from(value: ExprWrapper) -> Self {
        value.0.clone()
    }
}

#[derive(Deserialize)]
pub struct ExpressionPartSerializer(i32, i32, String);

impl From<ExpressionPartSerializer> for ExpressionPart {
    fn from(value: ExpressionPartSerializer) -> Self {
        Self {
            priority: value.0,
            stacks: value.1,
            expr: ExprWrapper::from(value.2).0.clone(),
        }
    }
}

/// Turn a single `(priority, stacks, expr_str, op)` into an Expression
impl From<ExpressionPartSerializer> for Expression {
    fn from(val: ExpressionPartSerializer) -> Self {
        Expression::from(ExpressionPart::from(val))
    }
}

/// Turn a list of `(priority, stacks, expr_str, op)` into an Expression
impl From<Vec<ExpressionPartSerializer>> for Expression {
    fn from(vals: Vec<ExpressionPartSerializer>) -> Self {
        let parts: Vec<ExpressionPart> = vals.into_iter().map(|v| v.into()).collect();
        Expression {
            base: 0.0,
            parts,
        }
    }
}

#[derive(Deserialize)]
pub struct ExpressionSerializer {
    pub base: f32,
    pub parts: Vec<ExpressionPartSerializer>,
}

/// Turn a full ExpressionSerializer { base, parts } into an Expression
impl From<ExpressionSerializer> for Expression {
    fn from(val: ExpressionSerializer) -> Self {
        let ExpressionSerializer { base, parts } = val;
        let expression_parts = parts.into_iter().map(ExpressionPart::from).collect();
        Expression { base, parts: expression_parts }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// 2) Derive a custom `Deserialize` for Expression with an untagged enum
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(untagged)]
enum ExpressionInput {
    // If the JSON is just a float or integer, treat as a plain “base” expression
    Float(f64),
    Int(i64),

    // A single expression part
    WeightedExpression(ExpressionPartSerializer),

    // A list of expression parts
    WeightedExpressionList(Vec<ExpressionPartSerializer>),

    // A full struct { "base": ..., "parts": [...] }
    FullExpression(ExpressionSerializer),
}

impl<'de> Deserialize<'de> for Expression {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = ExpressionInput::deserialize(deserializer)?;
        let expr = match input {
            ExpressionInput::Float(f) => Expression::from_float(f as f32),
            ExpressionInput::Int(i) => Expression::from_float(i as f32),
            ExpressionInput::WeightedExpression(part_ser) => {
                // base=0, single part
                part_ser.into()
            }
            ExpressionInput::WeightedExpressionList(parts_ser) => {
                // base=0, multiple parts
                parts_ser.into()
            }
            ExpressionInput::FullExpression(expr_ser) => {
                // { base, parts }
                expr_ser.into()
            }
        };
        Ok(expr)
    }
}

impl<'de> Deserialize<'de> for StatRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ssv = String::deserialize(deserializer)?;
        Ok(ssv.into())
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NestedExpression {
    // Single expression (float, int, or "parts" from your existing Expression)
    Expression(Expression),

    // Nested map of string -> NestedExpression
    Map(HashMap<String, NestedExpression>),
}

fn flatten_nested_expressions(
    prefix: &str,
    nested: &HashMap<String, NestedExpression>,
    out: &mut HashMap<String, Expression>,
) {
    for (key, value) in nested {
        // If we already have a prefix, attach with a dot; otherwise use just `key`
        let new_key = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", prefix, key)
        };

        match value {
            NestedExpression::Expression(expr) => {
                // This is a final leaf, store the flattened key
                out.insert(new_key, expr.clone());
            }
            NestedExpression::Map(submap) => {
                // Recurse
                flatten_nested_expressions(&new_key, submap, out);
            }
        }
    }
}

impl<'de> Deserialize<'de> for StatDefinitions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First parse into an intermediate `HashMap<String, NestedExpression>`
        let nested_map = HashMap::<String, NestedExpression>::deserialize(deserializer)?;

        // We'll build a new flattened map of type `HashMap<String, Expression>`
        let mut flat_map = HashMap::new();

        // Recursively flatten everything
        flatten_nested_expressions("", &nested_map, &mut flat_map);

        // Finally produce `StatDefinitions(flat_map)`
        Ok(StatDefinitions::from(flat_map))
    }
}