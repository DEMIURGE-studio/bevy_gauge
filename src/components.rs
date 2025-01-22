use bevy::prelude::*;
use bevy_utils::HashMap;
use evalexpr::{
    Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use serde::Deserialize;
use crate::prelude::*;

use super::serialization::ExprWrapper;

/// Open problems:
/// - Stat components: Stat components are components that are built and updated based
/// on the owning entities StatContextRefs. Right now I use somewhat complex Fields 
/// trait that lets a components fields be accessed via strings. This is great in some
/// cases but limited in others. It may be the case that I just want to implement
/// From<&StatContextRefs> for a component and have some helper functions that 
/// automatically create, add, and update components based on the From
/// 
/// StatComponent could require From<&StatContextRefs>, and would still have the 
/// is_valid and update functions. 
/// 
/// Ergonomics 1:
/// stat_component!(
///     SelfExplosionEffect<T> {
///         pub radius: "SelfExplosionEffect<T>.radius"
///         pub damage: Damage {
///             min: "SelfExplosionEffect<T>.damage.min",
///             max: "SelfExplosionEffect<T>.damage.max"
///         }
///     }
/// )
/// Thoughts? Soooo much needless repetition. Why are we here? Just to suffer?
/// 
/// 
/// 
/// - Write-back components: While some components may want to derive their values 
/// from StatDefinitions, others may want to write their values to StatDefinitions.
/// For instance, it may be important for your Stats to be aware of a characters
/// current life. However it doesn't make much sense to have "CurrentLife" as a 
/// typical stat since it's value may be constantly changing and its value isnt 
/// derived from any expression. So you could have a stat component definition that
/// looks like this:
/// 
/// struct Life {
///     max: "Life.max",
///     current: WriteBack
/// }
/// 
/// 
/// 
/// - Selective updates: When stat definitions change, we should only update
/// components when values relevant to that component are changed. This is complicated
/// with the introduction of StatContextRefs, which allow stat values to be derived 
/// from arbitrary entities in the context tree.
/// 
/// Lets say we have the following structure:
/// StatEntityA
///     StatEntityB
///     StatEntityC
/// 
/// If StatEntityA's definitions are updated, any definitions in StatEntityB or C that
/// depend on StatEntityA should also be updated. This is simple to do generally; We
/// just touch the StatDefinitions of StatEntity B and C so that they are caught by
/// change detection. 
/// 
/// Lets say StatEntityA's Strength is updated. StatEntityB has an expression that
/// relies on "parent.Strength". StatEntityA has to send a list of updated stats to
/// StatEntityB and C. StatEntityB and C will have an update registry that will
/// selectively update specific stats. So the selective entity will match 
/// "parent.Strength" to an array of effected stats. Then each changed stat is iterated
/// over, matched to an array of effected stats, and each effected stat is 
/// recalculated. 
/// 
/// Concepts:
///     StatDefinitions - The collection of expressions that are used to calculate
///         a stats value.
///     Stat components - Components that derive their values from StatDefinitions
///     StatUpdateRegistry - Component that maps qualified stats to dependent stats.
///         Take the stat definition "Strength = parent.Strength + self.Willpower"
///         The StatUpdateRegistry would map "parent.Strength" -> "Strength" and
///         "self.Willpower" -> "Strength"
///     
///         KEY MISSING CONCEPT - This will cause a cascade of unnecessary calculations.
///         If parent.Strength updates, that changes self.Strength. What if something
///         relies on self.Strength? Well then, we'll recalculate self.Strength, which
///         will recalculate parent.Strength and so on. This is not desirable.
///             - We could maintain a hashmap of already-calculated stats
///             - We could prune the update-tree so that only the highest level stats
///                 are updated. But... idk
/// 
/// 
/// ACTION ITEMS:
/// - It would be nice to be able to bake expression parts into a single expression, 
/// as benchmarks show this is about 2x as fast to evaluate.
///     This makes it hard to do "clamping" actions
/// - Can we automatically derive the ordering of expression parts without losing
/// anything? Automatically deriving them would be more elegant as we could get rid of
/// the entire concept of expression collisions
///     For example, most of the time evaluation should happen in the order "x - * /"
///     but are there exceptions? If not, I can just order my statements based on the
///     operator.
///     Right now I can clamp with a "Total = value" which could be useful for 
///     CI style interactions. This makes more sense in a stepwise part-evaluation
///     style system. In the baked version you might see total life calculation like
///     so: "Total = (Base + AddedLife) * (IncreasedLife)" where we might want to
///     see something like "Total = clamp((Base + AddedLife) * (IncreasedLife), 1, 1)"
///     
///     Okay so MAYBE we can auto-derive it, we just need a smarter way to parse
///     values from expression parts into the baked expression strings.

// =======================================================
// 1. StatError
// =======================================================

#[derive(Debug)]
pub enum StatError {
    BadOpp(String),
    NotFound(String),
}

// =======================================================
// 2. Expression & ExpressionPart
// =======================================================

#[derive(Debug, Clone, Default)]
pub struct Expression {
    /// The “base” numeric value for simple stats (parts = empty) 
    /// or the starting point for more advanced stats (parts != empty).
    pub base: f32,

    /// Additional expression parts (like `+ 2 * parent.Strength`).
    pub parts: Vec<ExpressionPart>,
}

impl Expression {
    /// Construct a plain numeric expression with no additional parts
    pub fn from_float(value: f32) -> Self {
        Self {
            base: value,
            parts: vec![],
        }
    }

    /// Merge two expressions by “adding” them.
    ///
    /// - Add both `base` values
    /// - Merge expression parts if they share the same priority & identical node
    pub fn add(&self, other: &Expression) -> Result<Expression, StatError> {
        // Start by cloning `self`
        let mut merged = self.clone();

        // Add the base
        merged.base += other.base;

        // Merge the parts from `other`
        for part2 in &other.parts {
            // Look for an existing part with the same priority
            if let Some(existing_part) = merged
                .parts
                .iter_mut()
                .find(|p| p.priority == part2.priority)
            {
                // If it’s exactly the same expression string/Node, stack them
                if existing_part.expr == part2.expr {
                    existing_part.stacks += part2.stacks;
                } else {
                    // conflict => different expression at the same priority
                    return Err(StatError::BadOpp(format!(
                        "Conflict detected for priority {} with different expressions.",
                        part2.priority
                    )));
                }
            } else {
                // If we don’t have an existing part at this priority, just push it
                merged.parts.push(part2.clone());
            }
        }

        // Sort by priority ascending
        merged.parts.sort_by_key(|p| p.priority);
        Ok(merged)
    }

    /// Merge two expressions by “subtracting” the other.
    pub fn remove(&self, other: &Expression) -> Result<Expression, StatError> {
        // Start by cloning `self`
        let mut merged = self.clone();

        // Subtract the base
        merged.base -= other.base;

        // Remove the parts from `other`
        for part2 in &other.parts {
            if let Some(existing_part) = merged
                .parts
                .iter_mut()
                .find(|p| p.priority == part2.priority)
            {
                if existing_part.expr == part2.expr {
                    existing_part.stacks -= part2.stacks;
                    // If stacks <= 0, remove that part
                    if existing_part.stacks <= 0 {
                        merged.parts.retain(|p| p.priority != part2.priority);
                    }
                } else {
                    return Err(StatError::BadOpp(format!(
                        "Conflict detected for priority {} with different expressions.",
                        part2.priority
                    )));
                }
            } else {
                return Err(StatError::BadOpp(format!(
                    "Attempting to subtract a non-existent expression with priority {}.",
                    part2.priority
                )));
            }
        }

        Ok(merged)
    }

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self, eval_context: &StatContextRefs) -> f32 {
        // Start from base
        let mut current_value = self.base;
        let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

        for part in self.parts.iter() {
            // Build a local evalexpr context for each part
            context.clear();

            context.set_value("Total".to_string(), EvalValue::from_float(current_value as f64)).unwrap();

            // Fill that context with variable identifiers
            for var_name in part.expr.iter_variable_identifiers() {
                // Skip total
                if var_name == "Total" { continue; }

                let val = eval_context.get(var_name).unwrap_or(0.0);
                context
                    .set_value(var_name.to_string(), EvalValue::from_float(val as f64))
                    .unwrap();
            }
            
            // Evaluate. We just unwrap because:
            //  1. Eval should not fail
            //  2. get_value("Total") should never fail because we inserted Total into the context just above this
            //  3. because stat expressions all return number values, as_number should never fail
            part.expr.eval_with_context_mut(&mut context).unwrap();
            current_value = (context.get_value("Total").unwrap().as_number().unwrap()) as f32;
        }

        current_value
    }
}

#[derive(Debug, Clone)]
pub struct ExpressionPart {
    pub priority: i32,
    pub stacks: i32,
    pub expr: Node<DefaultNumericTypes>,
}

impl ExpressionPart {
    pub fn new(priority: i32, expr: &str) -> Self {
        Self {
            priority,
            stacks: 1,
            expr: ExprWrapper::from(expr).into(),
        }
    }
}

#[derive(Component, Debug, Clone)]
#[require(StatContext)]
pub struct StatDefinitions(HashMap<String, Expression>);

impl StatDefinitions {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Core getter method that accepts &str.
    pub(crate) fn get_str(&self, stat: &str, eval_context: &StatContextRefs) -> Result<f32, StatError> {
        match self.0.get(stat) {
            Some(stat_value) => Ok(stat_value.evaluate(eval_context)),
            None => Err(StatError::NotFound(stat.to_string())),
        }
    }

    /// Helper getter method that accepts any AsStr implementor.
    pub fn get<S: AsStr>(&self, stat: S, eval_context: &StatContextRefs) -> Result<f32, StatError> {
        self.get_str(stat.to_str(), eval_context)
    }

    /// Adds a value to a specific stat.
    pub fn add<S: AsStr, V>(&mut self, stat: S, value: V) -> Result<(), StatError>
    where
        V: Into<Expression>,
    {
        let stat_name = stat.to_str();
        let stat_value = value.into();
        let current = self.0.get(stat_name).cloned().unwrap_or_default();
        let new_value = current.add(&stat_value)?;
        self.0.insert(stat_name.to_string(), new_value);
        Ok(())
    }

    /// Subtracts a value from a specific stat.
    pub fn subtract<S: AsStr, V>(&mut self, stat: S, value: V) -> Result<(), StatError>
    where
        V: Into<Expression>,
    {
        let stat_name = stat.to_str();
        let stat_value = value.into();
        let current = self.0.get(stat_name).cloned().unwrap_or_default();
        let new_value = current.remove(&stat_value)?;
        self.0.insert(stat_name.to_string(), new_value);
        Ok(())
    }

    /// Adds all stats from `GrantsStats` into `HasStats`.
    pub fn add_stats(&mut self, stats: &GrantsStats) -> Result<(), StatError> {
        for (stat, stat_val) in &stats.0.0 {
            self.add(stat, stat_val.clone())?;
        }
        Ok(())
    }

    /// Subtracts all stats from `GrantsStats` into `HasStats`.
    pub fn remove_stats(&mut self, stats: &GrantsStats) -> Result<(), StatError> {
        for (stat, stat_val) in &stats.0.0 {
            self.subtract(stat, stat_val.clone())?;
        }
        Ok(())
    }

    pub fn set<S: AsStr, V>(&mut self, stat: S, value: V) -> &mut Self
    where
        V: Into<Expression>,
    {
        let stat_name = stat.to_str();
        self.0.insert(stat_name.to_string(), value.into());
        self
    }
}

impl From<HashMap<String, Expression>> for StatDefinitions {
    fn from(value: HashMap<String, Expression>) -> Self {
        return StatDefinitions(value);
    }
}

#[derive(Component, Debug, Deserialize, Clone, Deref, DerefMut)]
pub struct GrantsStats(StatDefinitions);

impl GrantsStats {
    pub fn new() -> Self {
        Self(StatDefinitions::new())
    }
}

impl From<StatDefinitions> for GrantsStats {
    fn from(value: StatDefinitions) -> Self {
        Self(value)
    }
}

fn update_stats(
    stat_entity_query: Query<Entity, Changed<StatContext>>,
    mut commands: Commands,
) {
    for entity in stat_entity_query.iter() {
        commands.entity(entity).touch::<StatDefinitions>();
    }
}

/// This works for "parent" context updates but other contexts will need bespoke updating systems
fn update_parent_stat_definitions(
    stat_entity_query: Query<&Children, Or<(Changed<StatDefinitions>, Changed<StatContext>)>>,
    mut commands: Commands,
) {
    for children in stat_entity_query.iter() {
        for child in children.iter() {
            commands.entity(*child).touch::<StatDefinitions>();
        }
    }
}

fn update_parent_context(
    mut stat_entity_query: Query<(&Parent, &mut StatContext), Changed<Parent>>,
    parent_query: Query<Entity, With<StatDefinitions>>,
) {
    for (parent, mut stat_context) in stat_entity_query.iter_mut() {
        if parent_query.contains(parent.get()) {
            stat_context.insert("parent", parent.get());
        }
    }
}

// self context
fn update_self_context(
    mut stat_entity_query: Query<(Entity, &mut StatContext), Added<StatContext>>,
) {
    for (entity, mut stat_context) in stat_entity_query.iter_mut() {
        stat_context.insert("self", entity);
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(StatsUpdate, (
        update_stats,
        update_parent_stat_definitions,
        update_parent_context,
        update_self_context,
    ));
}