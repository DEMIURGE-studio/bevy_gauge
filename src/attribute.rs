use std::fmt::Debug;
use crate::value_type::{Expression, ValueBounds, ValueType};




#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttributeInstance {
    pub value: ValueType,
    pub bounds: Option<ValueBounds>
}

impl AttributeInstance {
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


// fn update_attributes(
//     attribute_entity_query: Query<Entity, Changed<StatContext>>,
//     mut commands: Commands,
// ) {
//     for entity in attribute_entity_query.iter() {
//         // TODO
//     }
// }


// pub(crate) fn plugin(app: &mut App) {
//     app.add_systems(AddStatComponent, (
//         update_attributes,
//     ));
// }
