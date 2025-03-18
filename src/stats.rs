use std::fmt::Debug;
use bevy::prelude::*;

#[derive(Debug)]
pub enum FieldType {
    Min,
    Max,
    Base,
    Current
}

#[derive(Debug)]
pub enum DurationType {
    Instant,
    Infinite,
    Duration(f32)
}

#[derive(Debug)]
pub enum ModifierType {
    Flat(f32),
    Increased(f32)
}


#[derive(Component, Debug)]
pub struct StatModifier {
    pub field_type: FieldType,
    pub modifier_type: ModifierType,
    pub duration_type: DurationType
}

#[bevy_trait_query::queryable]
pub trait EntityStat: Debug
{
    fn get_stat_type(&self) -> &str;
    fn get_min(&self) -> Option<f32>;
    fn get_max(&self) -> Option<f32>;
    fn get_base_value(&self) -> f32;
    fn get_current_value(&self) -> f32;
    fn set_base_value(&mut self, value: f32);
    fn set_current_value(&mut self, value: f32);
    fn set_min_value(&mut self, value: Option<f32>);
    fn set_max_value(&mut self, value: Option<f32>);

    fn evaluate(&mut self, modifiers: &Query<&StatModifier>) {
        for modifier in modifiers.iter() {
            match modifier.field_type {
                FieldType::Min => {
                    if let Some(mut value) = self.get_min() {
                        value = apply_modifier(value, &modifier.modifier_type);
                        self.set_min_value(Some(value));
                    }
                }
                FieldType::Max => {
                    if let Some(mut value) = self.get_max() {
                        value = apply_modifier(value, &modifier.modifier_type);
                        self.set_max_value(Some(value));
                    }
                }
                FieldType::Base => {
                    let mut value = self.get_base_value();
                    value = apply_modifier(value, &modifier.modifier_type);
                    self.set_base_value(value);
                }
                FieldType::Current => {
                    let mut value = self.get_current_value();
                    value = apply_modifier(value, &modifier.modifier_type);
                    if let (Some(min), Some(max)) = (self.get_min(), self.get_max()) {
                        self.set_current_value(value.clamp(min, max));
                    } else {
                        self.set_current_value(value);
                    }
                }
            }
        }
    }
}

fn apply_modifier(value: f32, modifier: &ModifierType) -> f32 {
    match modifier {
        ModifierType::Flat(amount) => value + amount,
        ModifierType::Increased(percent) => value * (percent),
    }
}
