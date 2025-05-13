use bevy::prelude::*;

/// The primary error type used throughout the stat system.
///
/// This enum encompasses various issues that can arise during stat configuration,
/// modification, evaluation, or path parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum StatError {
    /// Indicates that a provided string could not be successfully parsed into a valid stat path.
    /// This can happen if the path format is incorrect.
    InvalidStatPath { 
        /// The problematic path string that caused the error.
        path: String, 
        /// Specific details about why parsing failed.
        details: String 
    },
    
    /// Occurs when an error is encountered during the evaluation of a stat expression.
    /// This could be due to syntax errors in the expression, missing variables, or other evaluation issues.
    ExpressionError { 
        /// The expression string that failed to evaluate.
        expression: String, 
        /// Details from the expression evaluation engine about the failure.
        details: String 
    },
    
    /// Signifies that an operation was attempted on an entity that does not exist or
    /// no longer has a `Stats` component.
    EntityNotFound { 
        /// The `Entity` that was not found.
        entity: Entity 
    },
    
    /// Indicates that a specific stat (or stat part) could not be found on an entity.
    /// This might mean the stat was never defined or the path was misspelled.
    StatNotFound { 
        /// The path of the stat that was not found.
        path: String 
    },
    
    /// Occurs when a tag in a stat path is not in the expected numerical format.
    InvalidTagFormat { 
        /// The incorrectly formatted tag string.
        tag: String, 
        /// The full stat path where the invalid tag was encountered.
        path: String 
    },
    
    /// Signals that a circular dependency was detected during stat evaluation.
    /// For example, Stat A depends on Stat B, and Stat B depends back on Stat A.
    DependencyCycle { 
        /// The stat path where the cycle was detected or that is part of the cycle.
        path: String 
    },
    
    /// Occurs when an expression references a source alias (e.g., `"Stat@SourceName"`)
    /// for which no corresponding source entity has been registered on the target entity.
    MissingSource { 
        /// The name of the source alias that was expected but not found.
        source_name: String, 
        /// The stat path on the target entity that contained the reference to the missing source.
        path: String 
    },
    
    /// A general-purpose error for internal issues within the stat system that don't
    /// fit into the more specific categories.
    Internal { 
        /// A string providing more details about the internal error.
        details: String 
    },
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