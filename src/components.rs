use std::fmt::Debug;

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

#[derive(Debug, Clone)]
pub enum StatType {
    Literal(f32),
    Expression(Expression),
}

impl StatType {
    pub fn from_float(val: f32) -> StatType {
        StatType::Literal(val)
    }

    pub fn add(&mut self, val: f32) {
        match self {
            StatType::Literal(mut current_val) => {
                current_val += val;
            },
            StatType::Expression(_) => { },
        }
    }

    pub fn subtract(&mut self, val: f32) {
        match self {
            StatType::Literal(mut current_val) => {
                current_val -= val;
            },
            StatType::Expression(_) => { },
        }
    }

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self, eval_context: &StatContextRefs) -> f32 {
        if let StatType::Literal(val) = self {
            return *val;
        }

        let StatType::Expression(expr)= self else {
            return 0.0;
        };

        // Start from base
        let mut current_value = 0.0;
        let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

        context.set_value("Total".to_string(), EvalValue::from_float(current_value as f64)).unwrap();

        // Fill that context with variable identifiers
        for var_name in expr.iter_variable_identifiers() {
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
        expr.eval_with_context_mut(&mut context).unwrap();
        current_value = (context.get_value("Total").unwrap().as_number().unwrap()) as f32;

        current_value
    }

    pub fn from_expression(value: Expression) -> Self {
        StatType::Expression(value)
    }
}

impl Default for StatType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Expression(pub Node<DefaultNumericTypes>);

impl Default for Expression {
    fn default() -> Self {
        Self(evalexpr::build_operator_tree("Total = 0").unwrap())
    }
}

#[derive(Component, Debug, Clone)]
pub struct Stats(pub HashMap<String, StatType>);

impl Stats {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    
    /// Get the value of a stat by name, evaluating it if necessary.
    pub fn get_str(
        &self,
        stat: &str,
        eval_context: &StatContextRefs,
    ) -> Result<f32, StatError> {
        match self.0.get(stat) {
            Some(stat_type) => Ok(stat_type.evaluate(eval_context)),
            None => Err(StatError::NotFound(stat.to_string())),
        }
    }

    /// Get the value of a stat by name, evaluating it if necessary.
    pub fn get<S: AsRef<str>>(
        &self,
        stat: S,
        eval_context: &StatContextRefs,
    ) -> Result<f32, StatError> {
        return self.get_str(stat.as_ref(), eval_context);
    }

    /// Add a new `StatType` or update an existing one with additional value.
    pub fn add<S: AsRef<str>, V: Into<f32>>(&mut self, stat: S, value: V) -> Result<(), StatError> {
        let stat_name = stat.as_ref();
        let current = self.0.entry(stat_name.to_string()).or_insert_with(|| StatType::Literal(0.0));
        current.add(value.into());
        Ok(())
    }

    /// Subtract a value from an existing `StatType`.
    pub fn subtract<S: AsRef<str>, V: Into<f32>>(&mut self, stat: S, value: V) -> Result<(), StatError> {
        let stat_name = stat.as_ref();
        let current = self.0.get_mut(stat_name);
        if let Some(current_stat) = current {
            current_stat.subtract(value.into());
            Ok(())
        } else {
            Err(StatError::NotFound(stat_name.to_string()))
        }
    }

    /// Set a stat to a specific `StatType`.
    pub fn set<S: AsRef<str>, T: Into<StatType> + Debug>(&mut self, stat: S, stat_type: T) -> &mut Self {
        println!("{:#?}", stat_type);
        self.0.insert(stat.as_ref().to_string(), stat_type.into());
        self
    }

    /// Remove a stat by name.
    pub fn remove<S: AsRef<str>>(&mut self, stat: S) -> Result<(), StatError> {
        if self.0.remove(stat.as_ref()).is_some() {
            Ok(())
        } else {
            Err(StatError::NotFound(stat.as_ref().to_string()))
        }
    }

    /// Add all stats from another `StatDefinitions`.
    pub fn add_stats(&mut self, stats: &GrantsStats) -> Result<(), StatError> {
        for (stat, stat_type) in &stats.0.0 {
            if let StatType::Literal(val) = stat_type {
                self.add(stat, *val)?;
            } else {
                self.set(stat, stat_type.clone());
            }
        }
        Ok(())
    }

    /// Remove all stats from another `StatDefinitions`.
    pub fn remove_stats(&mut self, stats: &GrantsStats) -> Result<(), StatError> {
        for (stat, _) in &stats.0.0 {
            self.remove(stat)?;
        }
        Ok(())
    }
}

impl From<HashMap<String, StatType>> for Stats {
    fn from(value: HashMap<String, StatType>) -> Self {
        Self(value)
    }
}

#[derive(Component, Debug, Deserialize, Clone, Deref, DerefMut)]
pub struct GrantsStats(Stats);

impl GrantsStats {
    pub fn new() -> Self {
        Self(Stats::new())
    }
}

impl From<Stats> for GrantsStats {
    fn from(value: Stats) -> Self {
        Self(value)
    }
}

fn update_stats(
    stat_entity_query: Query<Entity, Changed<StatContext>>,
    mut commands: Commands,
) {
    for entity in stat_entity_query.iter() {
        commands.entity(entity).touch::<Stats>();
    }
}

/// This works for "parent" context updates but other contexts will need bespoke updating systems
fn update_parent_stat_definitions(
    stat_entity_query: Query<&Children, Or<(Changed<Stats>, Changed<StatContext>)>>,
    mut commands: Commands,
) {
    for children in stat_entity_query.iter() {
        for child in children.iter() {
            commands.entity(*child).touch::<Stats>();
        }
    }
}

fn update_parent_context(
    mut stat_entity_query: Query<(&Parent, &mut StatContext), Changed<Parent>>,
    parent_query: Query<Entity, With<Stats>>,
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