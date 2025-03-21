use std::collections::HashMap;
use std::fmt::Debug;
use bevy::ecs::relationship::Relationship;
use bevy::prelude::*;
use evalexpr::{
    Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value as EvalValue
};
use crate::prelude::*;

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
pub enum ValueType {
    Literal(f32),
    Expression(Expression),
}


#[derive(Debug, Clone)]
pub struct ValueBounds {
    min: Option<ValueType>,
    max: Option<ValueType>,
}

impl ValueBounds {
    pub fn new(min: Option<ValueType>, max: Option<ValueType>) -> Self {
        Self { min, max }
    }
    
    //TODO implement bounds
    
}

impl ValueType {
    pub fn from_float(val: f32) -> ValueType {
        ValueType::Literal(val)
    }

    pub fn add(&mut self, val: f32) {
        match self {
            ValueType::Literal(current_val) => {
                *current_val += val;
            },
            ValueType::Expression(_) => { },
        }
    }

    pub fn subtract(&mut self, val: f32) {
        match self {
            ValueType::Literal(current_val) => {
                *current_val -= val;
            },
            ValueType::Expression(_) => { },
        }
    }

    /// Evaluate this expression into a final f32, given a stat context.
    pub fn evaluate(&self, eval_context: &StatContextRefs) -> f32 {
        if let ValueType::Literal(val) = self {
            return *val;
        }

        let ValueType::Expression(expr)= self else {
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
        ValueType::Expression(value)
    }
}

impl Default for ValueType {
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


#[derive(Debug, Clone, Deref, DerefMut)]
pub struct StatInstance {
    pub tag: String,
    #[deref]
    pub value: ValueType,
    pub bounds: Option<ValueBounds>
}

#[derive(Component, Debug, Default, Clone, Deref, DerefMut)]
#[require(StatContext)]
pub struct StatCollection(pub HashMap<String, StatInstance>);

impl StatCollection {
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
            Some(value) => match &**value {
                ValueType::Literal(val) => Ok(*val),
                ValueType::Expression(_) => Err(StatError::BadOpp("Expression found".to_string())),
            },
            None => return Err(StatError::BadOpp("Literal not found".to_string())),
        }

    }

    /// Add a new `StatType` or update an existing one with additional value.
    pub fn add<S: AsRef<str>, V: AsF32>(&mut self, stat: S, value: V) -> Result<(), StatError> {
        let stat_name = stat.as_ref();
        let current = self.entry(stat_name.to_string()).or_insert_with(|| StatInstance{tag: String::from("None"), value: ValueType::Literal(0.0), bounds: None});
        //TODO FIX ME PLEASE ^^^
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
    pub fn set<S: AsRef<str>, T: Into<ValueType> + Debug>(&mut self, stat: S, stat_type: T) -> &mut Self {
        self.insert(stat.as_ref().to_string(), StatInstance{tag: "".to_string(), value: stat_type.into(), bounds: None});
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
    
    pub fn add_stats(&mut self, stats: &StatCollection) -> Result<(), StatError> {
        for (stat, stat_type) in &stats.0 {
            if let ValueType::Literal(val) = &**stat_type {
                self.add(stat, *val)?;
            } else {
                self.set(stat, stat_type.clone().value);
            }
        }
        Ok(())
    }

    /// Remove all stats from another `StatDefinitions`.
    pub fn remove_stats(&mut self, stats: &StatCollection) -> Result<(), StatError> {
        for (stat, _) in &stats.0 {
            self.remove(stat)?;
        }
        Ok(())
    }
}

fn update_stats(
    stat_entity_query: Query<Entity, Changed<StatContext>>,
    mut commands: Commands,
) {
    for entity in stat_entity_query.iter() {
    }
}

/// This works for "parent" context updates but other contexts will need bespoke updating systems
fn update_parent_stat_definitions(
    stat_entity_query: Query<Entity, Or<(Changed<StatCollection>, Changed<StatContext>)>>,
    children_query: Query<&Children>,
    mut commands: Commands,
) {
    for entity in stat_entity_query.iter() {
        for child in children_query.iter_descendants(entity) {
        }
    }
}

fn update_parent_context(
    mut stat_entity_query: Query<(&ChildOf, &mut StatContext), Changed<ChildOf>>,
    parent_query: Query<Entity, With<StatCollection>>,
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

// TODO This does not take into account if the root changes. So if the root ever changes without the parent changing, this will break. This could happen if an item is traded theoretically.
fn update_root_context(
    mut changed_parent_query: Query<(Entity, &mut StatContext), Changed<ChildOf>>,
    parent_query: Query<&ChildOf>,
) {
    for (entity, mut stat_context) in changed_parent_query.iter_mut() {
        let root = parent_query.root_ancestor(entity);
        
        stat_context.insert("root", root);
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(AddStatComponent, (
        update_stats,
        update_parent_stat_definitions,
        update_parent_context,
        update_self_context,
        update_root_context,
    ));
}