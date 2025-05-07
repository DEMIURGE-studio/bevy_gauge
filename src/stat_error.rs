use bevy::prelude::*;

/// Error type for the stat system
#[derive(Debug, Clone, PartialEq)]
pub enum StatError {
    /// Failed to parse a stat path
    InvalidStatPath { path: String, details: String },
    
    /// Error when evaluating an expression
    ExpressionError { expression: String, details: String },
    
    /// Entity not found
    EntityNotFound { entity: Entity },
    
    /// Stat not found
    StatNotFound { path: String },
    
    /// Invalid tag format in path
    InvalidTagFormat { tag: String, path: String },
    
    /// Dependency cycle detected
    DependencyCycle { path: String },
    
    /// Missing source entity reference
    MissingSource { source_name: String, path: String },
    
    /// Internal error
    Internal { details: String },
}

impl std::fmt::Display for StatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatError::InvalidStatPath { path, details } => {
                write!(f, "Invalid stat path '{}': {}", path, details)
            }
            StatError::ExpressionError { expression, details } => {
                write!(f, "Failed to evaluate expression '{}': {}", expression, details)
            }
            StatError::EntityNotFound { entity } => {
                write!(f, "Entity {:?} not found", entity)
            }
            StatError::StatNotFound { path } => {
                write!(f, "Stat '{}' not found", path)
            }
            StatError::InvalidTagFormat { tag, path } => {
                write!(f, "Invalid tag format '{}' in path '{}'", tag, path)
            }
            StatError::DependencyCycle { path } => {
                write!(f, "Dependency cycle detected for stat '{}'", path)
            }
            StatError::MissingSource { source_name, path } => {
                write!(f, "Missing source '{}' referenced by '{}'", source_name, path)
            }
            StatError::Internal { details } => {
                write!(f, "Internal error: {}", details)
            }
        }
    }
}

impl std::error::Error for StatError {}

// Type alias for Result with StatError
pub type StatResult<T> = Result<T, StatError>;