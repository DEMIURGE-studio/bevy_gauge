use bevy::prelude::*;
use crate::prelude::*;
use evalexpr::{DefaultNumericTypes, Node};

#[derive(Debug, Clone)]
pub struct StatRequirement(pub Node<DefaultNumericTypes>);

impl StatRequirement {
    pub fn met(&self, stats: &Stats) -> bool {
        match self.0.eval_boolean_with_context(stats.get_context()) {
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

#[derive(Component, Debug, Default, Clone)]
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
    pub fn met(&self, stats: &Stats) -> bool {
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