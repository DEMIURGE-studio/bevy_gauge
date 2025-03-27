use crate::value_type::{Expression, StatValue, ValueBounds, ValueType};
use std::fmt::Debug;

#[derive(Debug, Clone, Default)]
pub struct AttributeInstance {
    value: StatValue,
}

impl AttributeInstance {
    pub fn new(value: StatValue) -> Self {
        AttributeInstance { value,  }
    }
    
    pub fn value(&self) -> &StatValue {
        &self.value
    }
    
    pub fn value_mut(&mut self) -> &mut StatValue {
        &mut self.value
    }
    
    pub fn get_value_f32(&self) -> f32 {
        self.value.get_value_f32()
    }
}

