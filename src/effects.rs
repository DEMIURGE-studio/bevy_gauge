use std::collections::HashMap;
use crate::modifiers::ModifierDefinition;
use crate::prelude::ValueType;
use crate::tags::ValueTag;


pub struct EffectDefinition {
    pub ctx: EffectContext,
    pub modifiers: Vec<ModifierDefinition>
    
}

pub struct EffectContext {
    pub effect_values: HashMap<ValueTag, ValueType>,
}

pub struct EffectInstance {
    // TODO implement
}
