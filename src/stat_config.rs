use bevy::{utils::HashMap, prelude::*};
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value};
use super::prelude::*;

// Configuration resource for stat types
#[derive(Resource, Clone, Debug, Default)]
pub struct StatConfig {
    // Maps stat name prefix to stat type
    pub type_map: HashMap<String, StatTypeKind>,
    // Maps stat name to initial modifier value
    pub initial_values: HashMap<String, f32>,
    // Maps stat name to total expression
    pub expressions: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug)]
pub enum StatTypeKind {
    Basic,
    Composable,
    Tagged,
}

impl StatConfig {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_type(mut self, name_prefix: &str, kind: StatTypeKind) -> Self {
        self.type_map.insert(name_prefix.to_string(), kind);
        self
    }
    
    pub fn with_initial_value(mut self, name: &str, value: f32) -> Self {
        self.initial_values.insert(name.to_string(), value);
        self
    }
    
    pub fn with_expression(mut self, name: &str, expression: &str) -> Self {
        self.expressions.insert(name.to_string(), expression.to_string());
        self
    }
    
    pub fn get_type(&self, name: &str) -> StatTypeKind {
        // Look for prefixes that match
        for (prefix, kind) in &self.type_map {
            if name.starts_with(prefix) {
                return *kind;
            }
        }
        
        // Default based on path segments
        let segments = name.split('.').count();
        match segments {
            1 => StatTypeKind::Basic,
            2 => StatTypeKind::Composable,
            3 => StatTypeKind::Tagged,
            _ => StatTypeKind::Basic,
        }
    }
    
    pub fn get_initial_value(&self, name: &str) -> f32 {
        self.initial_values.get(name).copied().unwrap_or(0.0)
    }
    
    pub fn get_expression(&self, name: &str) -> String {
        // If the name contains "more" and we don't have a specific expression,
        // default to a multiplicative relationship
        if name.to_lowercase().contains("more") && !self.expressions.contains_key(name) {
            "1.0".to_string()
        } else {
            self.expressions.get(name).cloned().unwrap_or_else(|| {
                // Default to simple addition for most stats
                name.split('.').collect::<Vec<&str>>().join(" + ")
            })
        }
    }
}