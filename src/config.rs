use bevy::prelude::*;
use super::prelude::*;

#[derive(Resource)]
pub struct Config {

}

impl Config {
    pub(crate) fn get_stat_type(&self, path: &StatPath) -> &str {
        todo!()
    }
    
    pub(crate) fn get_relationship_type(&self, path: &StatPath) -> ModType {
        todo!()
    }
    
    pub(crate) fn get_total_expression(&self, path: &StatPath) -> &str {
        todo!()
    }
}