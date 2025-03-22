use std::fmt::Debug;
use bevy::{prelude::*, utils::HashMap};
use crate::prelude::*;

/// GOALS: 
/// - Reorganize into a more sensible structure
/// - Figure out modifiers / tributaties
/// - Get rid of unnecessary crates 
/// - Turn the StatContextRefs tree into a graph
/// - Kill "total" in expressions
/// - StatType as a trait?

#[derive(Component, Debug, Default, Clone, Deref, DerefMut)]
#[require(StatContext)]
pub struct StatDefinitions(pub HashMap<String, StatType>);

impl StatDefinitions {
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
    pub fn add_stats(&mut self, stats: &StatDefinitions) -> Result<(), StatError> {
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
    pub fn remove_stats(&mut self, stats: &StatDefinitions) -> Result<(), StatError> {
        for (stat, _) in &stats.0 {
            self.remove(stat)?;
        }
        Ok(())
    }
}