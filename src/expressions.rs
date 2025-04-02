use bevy::prelude::{Deref, DerefMut};
use evalexpr::{DefaultNumericTypes, Node};

#[derive(Debug, Clone, PartialEq, Deref, DerefMut)]
pub struct Expression {
    #[deref]
    pub expression: Node<DefaultNumericTypes>,
    pub cached_value: f32,
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
            cached_value: 0.0,
        }
    }

    pub fn extract_dependencies(&self) -> Vec<(String, String)> {
        let identifiers: Vec<_> = self
            .iter_variable_identifiers()
            .map(|val| String::from(val))
            .collect();

        let mut dependencies: Vec<(String, String)> = Vec::new();
        for identifier in identifiers {
            let group_type = identifier.split('.').collect::<Vec<&str>>();
            dependencies.push((group_type[0].to_string(), group_type[1].to_string()));
        }
        dependencies
    }
}
