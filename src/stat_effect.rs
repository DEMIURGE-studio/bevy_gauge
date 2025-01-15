use bevy_utils::HashMap;
use crate::prelude::*;

pub struct BaseStatEffect {
    effects: HashMap<String, Expression>,
}

pub struct StatEffect {
    effects: HashMap<String, Expression>,
}