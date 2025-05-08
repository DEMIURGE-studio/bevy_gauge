use std::{cell::SyncUnsafeCell, sync::RwLock};
use bevy::{ecs::system::SystemParam, prelude::*, utils::{HashMap, HashSet}};
use evalexpr::{DefaultNumericTypes, HashMapContext, Node};

/// Stat system features:

/// Error type for the stat system
#[derive(Debug, Clone, PartialEq)]
pub enum StatError {
    /// Failed to parse a stat path
    InvalidStatPath { path: String, details: String },

    /// TODO
    InvalidModifier { path: String, details: String },
    
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
            StatError::InvalidModifier { path, details } => {
                write!(f, "") // TODO
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)] // Added derives
pub struct StatPath {
    owner: Option<String>, // Name of the source (e.g., "EquippedTo", "Parent")
    path: String,          // The full original string (e.g., "Strength@EquippedTo")
    local_path: String,    // Path relative to the owner (e.g., "Strength")
    parts: Vec<String>,    // Local path split by '.' (e.g., ["Damage", "Added", "12"]), where 12 is a u32 representing arbitrary tags.
}

impl StatPath {
    // Parse a string into a StatPath
    fn parse<S: AsRef<str>>(path_str_ref: S) -> StatResult<Self> {
        let path_str = path_str_ref.as_ref();
        let mut owner = None;
        let local_path_str;

        if let Some((local, owner_str)) = path_str.rsplit_once('@') {
            owner = Some(owner_str.to_string());
            local_path_str = local;
        } else {
            local_path_str = path_str;
        }

        let parts: Vec<String> = local_path_str.split('.').map(String::from).collect();
        if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
            return Err(StatError::InvalidStatPath {
                path: path_str.to_string(),
                details: "Path cannot be empty or contain empty parts".to_string(),
            });
        }

        Ok(Self {
            owner,
            path: path_str.to_string(),
            local_path: local_path_str.to_string(),
            parts,
        })
    }
}

pub struct Expression {
    definition: String,
    compiled: Node<DefaultNumericTypes>,
}

pub enum ModifierType {
    Literal(f32),
    Expression(Expression),
}

pub struct StatConfig {

}

impl StatConfig {

}

trait StatLike {
    fn initialize(&self, _path: &StatPath, _stats: &mut Stats) { }
    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType) -> StatResult<()>;
    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) -> StatResult<()>;
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32;
}

enum StatType {
    Flat(Flat),
    Modifiable(Modifiable),
    Complex(Complex),
    Tagged(Tagged),
}

struct Flat {
    base: f32,
}

impl StatLike for Flat {
    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType) -> StatResult<()> {
        let ModifierType::Literal(value) = modifier else {
            return Err(StatError::InvalidModifier { path: todo!(), details: todo!() })
        };

        self.base += value;
        
        Ok(())
    }

    fn remove_modifier(&mut self, _path: &StatPath, modifier: &ModifierType) -> StatResult<()> {
        let ModifierType::Literal(value) = modifier else {
            return Err(StatError::InvalidModifier { path: todo!(), details: todo!() })
        };

        self.base += value;
        
        Ok(())
    }

    fn evaluate(&self, _path: &StatPath, _stats: &Stats) -> f32 {
        self.base
    }
}

struct Modifiable {
    base: f32,
    expressions: Vec<Expression>,
}

impl StatLike for Modifiable {
    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType) -> StatResult<()> {
        match modifier {
            ModifierType::Literal(value) => {
                self.base += value;
            },
            ModifierType::Expression(expression) => {
                self.expressions.push(expression);
            },
        }
        
        Ok(())
    }

    fn remove_modifier(&mut self, _path: &StatPath, modifier: &ModifierType) -> StatResult<()> {
        match modifier {
            ModifierType::Literal(value) => {
                self.base -= value;
            },
            ModifierType::Expression(expression) => {
                for (index, e) in self.expressions.iter().enumerate() {
                    if e.definition == expression.definition {
                        self.expressions.remove(index);
                        break;
                    }
                }
            },
        }
        
        Ok(())
    }

    fn evaluate(&self, _path: &StatPath, _stats: &Stats) -> f32 {
        // iterate every modifier. Combine them (add? product?)
        todo!()
    }
}

struct Complex {
    total: Expression,
    parts: HashMap<String, Modifiable>,
}

impl StatLike for Complex {
    fn initialize(&self, path: &StatPath, stats: &mut Stats) {
        // Add total as dependent of parts. I.e., "Life" depends on "Life.Added."
        // This will make it so that when Life.Added is updated, Life is also automatically
        // kept up to date.
        todo!()
    }

    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType) -> StatResult<()> {
        // Figure out the part based on the path and call add_modifier on that part
        todo!()
    }

    fn remove_modifier(&mut self, _path: &StatPath, modifier: &ModifierType) -> StatResult<()> {
        // add_modifier in reverse
        todo!()
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        // if the paths length is 1, we're evaluating a total like "Life." So we will evaluate
        // each "part" (i.e., "Added," "Increased"), add the output to a context, and feed the
        // context to the total Expression. Return the result.
        todo!()
    }
}

struct TaggedEntry {
    expressions: HashMap<u32, Vec<Modifiable>>,
}

struct Tagged {
    total: Expression,
    parts: HashMap<String, TaggedEntry>,
    cached_queries: RwLock<HashMap<u32, HashSet<String>>>,
}

// An example would be "Damage.Added.FIRE" for instance.
// Modifiers are stored permissively and require a "part", i.e., "Damage.Added.FIRE" applies 
// to every weapon type as long as it deals fire damage.
// Queries are strict and must be as specific as possible and do not require a part. For 
// example "Damage.FIRE|AXES" or "Damage.Added.ICE|SPELLS." 
impl StatLike for Tagged {
    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType) -> StatResult<()> {
        // add in the usual way
        todo!()
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) -> StatResult<()> {
        // remove in the usual way
        todo!()
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        // For a part query:
        // Go through the specific parts modifiers and check if modifier & query == query. If so,
        // accumulate the value. 
        // For a total query:
        // Go through all the different parts and check if modifier & query == query. If so,
        // accumulate the value to a context.
        // Once all parts have been accumulated, evaluate the context via the "total" expression.

        // However, after a query has been made, the value of the query itself should be added to the 
        // cached_stats for later. Since evaluate is called from get, any writing must be done using
        // internal mutability.

        // When the stat a query is based on is updated, the cached query value must also be updated.
        // Therefore there must be some way to match queries to their cached values.
        todo!()
    }
}

struct SyncContext(SyncUnsafeCell<HashMapContext>);

pub enum DependentType {
    Local(String),
    Entity(Entity, String),
}

// Stats stores all entity specific stat information. 
// Stats does not have a mutable public API. Any mutations of the Stats
// component should go through the StatAccessor.
#[derive(Component)]
pub struct Stats {
    // Matches a top-level stat identifier to its stat data.
    definitions: HashMap<String, StatType>,

    // Caches the value of a fully qualified stat. 
    cached_values: SyncContext,

    // Tracks any stat (on any entity) that is dependent on stat changes on this entity.
    dependents_map: HashMap<String, HashMap<DependentType, u32>>,

    // Tracks what stats a stat is dependent on, even if that stat is on another entity.
    depends_on_map: HashMap<String, HashMap<DependentType, u32>>,

    // Matches names to source entities.
    sources: HashMap<String, Entity>,
}

impl Stats {
    pub fn get(path: &StatPath) -> f32 {
        // If the value is not found in the cache, evaluate and cache the value, then return it.
        // If it is found then return it.
        todo!()
    }
}

#[derive(SystemParam)]
pub struct StatAccessor<'w, 's> {
    query: Query<'w, 's, &'static mut Stats>,
}

impl StatAccessor<'_, '_> {

}