use evalexpr::{DefaultNumericTypes, HashMapContext, Node, Value};

#[derive(Debug, Clone)]
pub struct Expression {
    pub(crate) string: String,
    pub(crate) value: Node<DefaultNumericTypes>,
}

impl Expression {
    pub(crate) fn evaluate(&self, context: &HashMapContext) -> f32 {
        self.value
            .eval_with_context(context)
            .unwrap_or(Value::Float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32
    }
}

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        self.string == other.string
    }
}

#[derive(Debug, Clone)]
pub enum ValueType {
    Literal(f32),
    Expression(Expression),
}

impl Default for ValueType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

impl From<Expression> for ValueType {
    fn from(value: Expression) -> Self {
        Self::Expression(value)
    }
}

impl From<&str> for ValueType {
    fn from(value: &str) -> Self {
        Self::Expression(Expression {
            string: value.to_string(),
            value: evalexpr::build_operator_tree(value).unwrap(),
        })
    }
}

impl From<String> for ValueType {
    fn from(value: String) -> Self {
        Self::Expression(Expression {
            string: value.clone(),
            value: evalexpr::build_operator_tree(&value).unwrap(),
        })
    }
}

impl From<f32> for ValueType {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

impl From<u32> for ValueType {
    fn from(value: u32) -> Self {
        Self::Literal(value as f32)
    }
}