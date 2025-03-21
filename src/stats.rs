use std::collections::HashMap;
use std::fmt::Debug;
use bevy::asset::io::memory::Value::Static;
use bevy::ecs::relationship::Relationship;
use bevy::prelude::*;
use evalexpr::build_operator_tree;
use crate::prelude::*;
use crate::value_type::{Expression, StatError, ValueBounds, ValueType};


#[derive(Debug, Clone, Default)]
pub struct StatInstance {
    pub value: ValueType,
    pub bounds: Option<ValueBounds>
}

impl StatInstance {
    pub fn new(value: ValueType, bounds: Option<ValueBounds>) -> Self {
        Self {
            value,
            bounds
        }
    }
    pub fn from_f32(val: f32) -> Self {
        Self {
            value: ValueType::Literal(val),
            bounds: None
        }
    }
    
    pub fn from_expression(expression: Expression) -> Self {
        Self {
            value: ValueType::Expression(expression),
            bounds: None
        }
    }
}

#[derive(Component, Debug, Default, Clone, Deref, DerefMut)]
// #[require(StatContext)]
pub struct StatCollection {
    #[deref]
    pub stats: HashMap<String, StatInstance>
}

impl StatCollection {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new() 
        }
    }
    
    /// Get the value of a stat by name, evaluating it if necessary.
    pub fn get_str(
        &self,
        stat: &str,
    ) -> Result<f32, StatError> {
        match self.stats.get(stat) {
            Some(stat_type) => Ok(stat_type.value.evaluate(self)),
            None => Err(StatError::NotFound(stat.to_string())),
        }
    }

    /// Get the value of a stat by name, evaluating it if necessary.
    pub fn get<S: AsRef<str>>(
        &self,
        stat: S,
    ) -> Result<f32, StatError> {
        self.get_str(stat.as_ref())
    }

    /// Add a new `StatType` or update an existing one with additional value.
    // pub fn add<S: AsRef<str>, V: AsF32>(&mut self, stat: S, value: V) -> Result<(), StatError> {
    //     let stat_name = stat.as_ref();
    //     let current = self.entry(stat_name.to_string()).or_insert_with(|| StatInstance{ value: ValueType::Literal(0.0), bounds: None});
    //     //TODO FIX ME PLEASE ^^^
    //     current.add(value.to_f32());
    //     Ok(())
    // }

    /// Subtract a value from an existing `StatType`.
    // pub fn subtract<S: AsRef<str>, V: AsF32>(&mut self, stat: S, value: V) -> Result<(), StatError> {
    //     let stat_name = stat.as_ref();
    //     let current = self.stats.get_mut(stat_name);
    //     if let Some(current_stat) = current {
    //         current_stat.subtract(value.to_f32());
    //         Ok(())
    //     } else {
    //         Err(StatError::NotFound(stat_name.to_string()))
    //     }
    // }

    /// Set a stat to a specific `StatType`.
    pub fn set<S: AsRef<str>, T: Into<ValueType> + Debug>(&mut self, stat: S, stat_type: T) -> &mut Self {
        self.stats.insert(stat.as_ref().to_string(), StatInstance{value: stat_type.into(), bounds: None});
        self
    }

    /// Remove a stat by name.
    pub fn remove<S: AsRef<str>>(&mut self, stat: S) -> Result<(), StatError> {
        if self.stats.remove(stat.as_ref()).is_some() {
            Ok(())
        } else {
            Err(StatError::NotFound(stat.as_ref().to_string()))
        }
    }

    // Add all stats from another `StatDefinitions`.
    
    // pub fn add_stats(&mut self, stats: &StatCollection) -> Result<(), StatError> {
    //     for (stat, stat_instance) in &stats.stats {
    //         if let ValueType::Literal(val) = stat_instance.value {
    //             self.add(stat, *val)?;
    //         } else {
    //             self.set(stat, stat_instance.clone().value);
    //         }
    //     }
    //     Ok(())
    // }
    // 
    // /// Remove all stats from another `StatDefinitions`.
    // pub fn remove_stats(&mut self, stats: &StatCollection) -> Result<(), StatError> {
    //     for (stat, _) in &stats.stats {
    //         self.remove(stat)?;
    //     }
    //     Ok(())
    // }
}

fn update_stats(
    stat_entity_query: Query<Entity, Changed<StatContext>>,
    mut commands: Commands,
) {
    for entity in stat_entity_query.iter() {
        // TODO
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
            // TODO
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


fn create_test_collection() -> StatCollection {
    let mut collection = StatCollection::default();

    collection.insert("health".to_string(), StatInstance::from_f32(100.0));
    collection.insert("armor".to_string(), StatInstance::from_f32(50.0));
    collection.insert("strength".to_string(), StatInstance::from_f32(25.0));
    collection.insert("agility".to_string(), StatInstance::from_f32(30.0));

    let damage_expr = Expression(build_operator_tree("strength * 2 + agility / 2").unwrap());
    collection.insert("damage".to_string(), StatInstance::from_expression(damage_expr));

    let defense_expr = Expression(build_operator_tree("armor + health * 0.1").unwrap());
    collection.insert("defense".to_string(), StatInstance::from_expression(defense_expr));

    collection
}

#[test]
fn test_literal_values() {
    let collection = create_test_collection();

    assert_eq!(collection.get("health").unwrap(), 100.0);
    assert_eq!(collection.get("armor").unwrap(), 50.0);
    assert_eq!(collection.get("strength").unwrap(), 25.0);
    assert_eq!(collection.get("agility").unwrap(), 30.0);
    assert_eq!(collection.get("damage").unwrap(), 65.0);
    assert_eq!(collection.get("defense").unwrap(), 60.0);
}
