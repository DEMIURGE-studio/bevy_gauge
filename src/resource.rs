use crate::value_type::ValueBounds;

#[derive(Debug, Clone, Default)]
pub struct ResourceInstance {
    pub current: f32,
    pub bounds: ValueBounds
}