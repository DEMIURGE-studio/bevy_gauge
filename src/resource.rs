use crate::value_type::ValueBounds;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceInstance {
    current: f32,
    bounds: ValueBounds,
}

impl ResourceInstance {
    pub fn new(current: f32, bounds: ValueBounds) -> Self {
        Self { current, bounds }
    }
    
    pub fn apply_effect(effect: ResourceEffect) {
        
    }
    
    pub fn get_current(&self) -> f32 {
        self.current
    }

}


#[derive(Debug, Clone, PartialEq)]
pub enum ResourceEffect {
    Flat(f32),
    Percent(f32)
}

