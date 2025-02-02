use std::fmt::Debug;

use bevy::prelude::*;
use bevy_utils::HashMap;
use evalexpr::{
    Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use serde::Deserialize;
use crate::prelude::*;

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
/// TODO
/// 1. We have ".." and "WriteBack". This is an ugly parlance. Anyway we could
/// enable a "..WriteBack" that updates both ways. This could lets us get a little
/// more freaky with stat effects. HOWEVER, this could be messy if we try to write
/// to the same stat both ways in a single frame. What is the source of truth? Can
/// we fix this via ordering somehow?
/// 
/// 2. We need to work on stat effects. Without "..WriteBack", stat effects are
/// going to be more focused on calculating effects at runtine. One of the original
/// cases for StatEffects was the Worm prayer. The Worm prayers damage is calculated
/// at runtime because it requires target stat access.
/// 
/// Maybe instead of ".." and "WriteBack" we can have a string option like 
/// 
/// stat_component!(
///     WormPrayer {
///         damage: "target.Corrosion"
///     }
/// )
/// 
/// If we can figure out how the damage gets applied that would be cool. That would
/// solve it.
/// 
/// OR are stat effects just a collection of stats that change the value of stat
/// literals?

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
            StatType::Literal(ref mut current_val) => {
                *current_val += val;
            },
            StatType::Expression(_) => { },
        }
    }

    pub fn subtract(&mut self, val: f32) {
        match self {
            StatType::Literal(ref mut current_val) => {
                *current_val -= val;
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
        let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

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
        //  2. get_value("Total") should never fail
        //  3. because stat expressions all return number values, as_number should never fail
        expr.eval_with_context_mut(&mut context).unwrap();
        let current_value = (context.get_value("Total").unwrap().as_number().unwrap()) as f32;

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
#[require(StatContext)]
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

    pub fn get_literal<S: AsRef<str>>(
        &self,
        stat: S,
    ) -> Result<f32, StatError> {
        let value = self.0.get(stat.as_ref());
        match value {
            Some(value) => match value {
                StatType::Literal(val) => return Ok(*val),
                StatType::Expression(_) => return Err(StatError::BadOpp("Expression found".to_string())),
            },
            None => return Err(StatError::BadOpp("Literal not found".to_string())),
        }

    }

    /// Add a new `StatType` or update an existing one with additional value.
    pub fn add<S: AsRef<str>, V: AsF32>(&mut self, stat: S, value: V) -> Result<(), StatError> {
        let stat_name = stat.as_ref();
        let current = self.0.entry(stat_name.to_string()).or_insert_with(|| StatType::Literal(0.0));
        current.add(value.to_f32());
        Ok(())
    }

    /// Subtract a value from an existing `StatType`.
    pub fn subtract<S: AsRef<str>, V: AsF32>(&mut self, stat: S, value: V) -> Result<(), StatError> {
        let stat_name = stat.as_ref();
        let current = self.0.get_mut(stat_name);
        if let Some(current_stat) = current {
            current_stat.subtract(value.to_f32());
            Ok(())
        } else {
            Err(StatError::NotFound(stat_name.to_string()))
        }
    }

    /// Set a stat to a specific `StatType`.
    pub fn set<S: AsRef<str>, T: Into<StatType> + Debug>(&mut self, stat: S, stat_type: T) -> &mut Self {
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
    pub fn add_stats(&mut self, stats: &Stats) -> Result<(), StatError> {
        for (stat, stat_type) in &stats.0 {
            if let StatType::Literal(val) = stat_type {
                self.add(stat, *val)?;
            } else {
                self.set(stat, stat_type.clone());
            }
        }
        Ok(())
    }

    /// Remove all stats from another `StatDefinitions`.
    pub fn remove_stats(&mut self, stats: &Stats) -> Result<(), StatError> {
        for (stat, _) in &stats.0 {
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
    app.add_systems(StatComponentUpdate, (
        update_stats,
        update_parent_stat_definitions,
        update_parent_context,
        update_self_context,
    ));
}