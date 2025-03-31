use crate::value_type::ValueBounds;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResourceInstance {
    pub current: f32,
    pub bounds: ValueBounds,
}
