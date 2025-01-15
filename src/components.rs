use bevy::prelude::*;
use bevy_utils::HashMap;
use evalexpr::{
    Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use serde::Deserialize;
use crate::prelude::*;

use super::serialization::ExprWrapper;

/// Problems: 
/// Memoization - Evaluating the same stat when none of its parent stats have changed is somewhat
///     problematic. In theory, the child values have not changed so we could just store the eval
///     and return that when re-evaluating stats that have already been evaluated.
///     - How do we know if a stat needs to be reevaluated? Each stat would have to store a hashset
///         of all of the stat values it contributes to. When it's updated, the Stats hashmap is
///         traversed with every newly dirtied value marked.
///     - When updating StatComponents, we only need to change the value of the component if the
///         underlying stat was dirtied. 
/// StatComponent updates. Currently stat components act as a read-only window into a stats evaluated
///     value, however what if we could make changes the other way? For instance, updating the Life
///     components 'current' field updates the "Life.current" value in the Stats component.
/// StatEffects - Can we generalize things like buffs, debuffs, equipment, talents into a generalized
///     "Stat effect"?
///     Maybe there's an InstantStatEffect and a StatEffect
///         InstantStatEffects only apply to the base and modify it in some instant way
///         StatEffects can add expressions
///     The expectation being that InstantStatEffects are fire-and-forget while StatEffects are usually
///         tracked somehow in order to be removable at a later interval. An instant effect will usually
///         apply to something like current life or a resource that the player gets and loses through
///         play - mana, power charges, etc. Stat effects are used for changes that can be added and 
///         reverted as the user chooses. For instance equipping and removing a piece of armor, or 
///         activating and de-activating a buff.
/// 
/// Right now we're trying to imagine how mantras will work. It's clear that sometimes we want the
/// characters stats and sometimes we want the mantras stats. Take Nesh Ti for example:
///     - Max 3 charges
///     - Uses all charges
///     - Heal 200 * charges used
///     - Remove all scorch from yourself
///     - +1 charge when you enter a new area
/// 
/// So the mantras stats might look something like {
///     "MaxCharges": 3,
///     "CurrentCharges": 3,
///     "SelfHeal<OnActivate>": "+= 200 * self.CurrentCharges",
///     "StatEffect<OnActivate>": (
///         "parent.Scorch = 0",
///     ),
///     "StatEffect<OnPostActivate>": (
///         "self.CurrentCharges = 0",
///     )
/// }
/// 
/// Ok so I support stuff like 
/// {
///     SelfExplosionEffect<OnBlock>: {
///         damage: formula,
///         radius: formula,
///     }
/// }
/// 
/// but can we go further?
/// {
///     SelfExplosionEffect<OnBlock>: {
///         damage: {
///             min: formula,
///             max: formula,
///         },
///         radius: formula,
///     }
/// }
/// 
/// It could be cool... but it's so fucking hard to code.
/// Maybe we just hold back on it until a usecase becomes
/// more clear?
/// 
/// Another day another problem. the Worm in Achra is a god that has
/// an ability: "Damage = Stacks of Corrosion on the Target". This is not
/// something the current stat system was necessarily designed for. So
/// how can we support it?
/// 
/// Damage as an expression: Instead of dealing damage as an f32 sent to
/// the target, maybe damage can be an expression evaluated with a special
/// context for the target.
/// 
/// So when we deal damage we can deal it as an expression that is evaluated 
/// against a special temporary context that includes the target entity and
/// its subcontexts, so we could have something like "Total += target.Corrosion"
/// as the expression

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

        // Evaluate each part in ascending priority
        let parts = self.parts.clone();
        for part in parts {
            // Build a local evalexpr context for each part
            let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

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
#[require(Stats, StatContext)]
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

#[derive(Component, Debug, Deserialize, Clone, Default)]
pub struct Stats(pub HashMap<String, f32>);

impl Stats {
    /// Core getter method that accepts &str.
    pub(crate) fn get_str(&self, stat: &str) -> Result<f32, StatError> {
        let val = self.0.get(stat);

        match val {
            Some(val) => Ok(*val),
            None => Err(StatError::NotFound(format!("Stat was not found: {:#?}", stat))),
        }
    }

    /// Helper getter method that accepts any AsStr implementor.
    pub fn get<S: AsStr>(&self, stat: S) -> Result<f32, StatError> {
        self.get_str(stat.to_str())
    }
}

// TODO I need to make a plugin for this stuff. I also maybe need to adjust the stats schedule
fn add_new_stats_system(
    mut stat_entity_query: Query<(&StatDefinitions, &mut Stats), Or<(Changed<StatDefinitions>, Changed<StatContext>)>>,
) {
    for (stat_definitions, mut stats) in stat_entity_query.iter_mut() {
        for (stat, _) in stat_definitions.0.iter() {
            if !stats.0.contains_key(stat) {
                stats.0.insert(stat.to_string(), 0.0);
            }
        }
        stats.0.retain(|stat_key, _| stat_definitions.0.contains_key(stat_key));
    }
}

fn update_stats_system(
    mut stat_entity_query: Query<(Entity, &mut Stats), Or<(Changed<StatDefinitions>, Changed<StatContext>)>>,
    stat_context_query: Query<&StatContext>,
    stat_definitions_query: Query<&StatDefinitions>,
) {
    for (stats_entity, mut stats) in stat_entity_query.iter_mut() {

        let stat_context = StatContextRefs::build(stats_entity, &stat_definitions_query, &stat_context_query);

        for (stat, value) in stats.0.iter_mut() {
            *value = stat_context.get(stat).unwrap_or(0.0);
        }
    } 
}

/// This works for "parent" context updates but other contexts will need bespoke updating systems
fn update_parent_stat_definitions(
    stat_entity_query: Query<&Parent, Changed<StatDefinitions>>,
    mut stat_context_query: Query<&mut StatContext, Changed<StatContext>>,
) {
    for parent in stat_entity_query.iter() {
        let Ok(mut parent_context) = stat_context_query.get_mut(parent.get()) else {
            continue;
        };

        parent_context.trigger_change_detection();
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
        (
            add_new_stats_system,
            update_stats_system,
        ).chain(),
        update_parent_stat_definitions,
        update_parent_context,
        update_self_context,
    ));
}