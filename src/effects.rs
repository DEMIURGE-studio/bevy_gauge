use std::collections::HashMap;
use crate::value_type::ValueType;

pub struct EffectDefinition {
    pub ctx: EffectContext,
    
}

pub struct EffectContext {
    pub effect_values: HashMap<String, ValueType>,
}

pub struct EffectInstance {
    // TODO implement
}
