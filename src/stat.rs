use super::prelude::*;

pub trait Stat {
    // Most of the stat can be configured from elements in the path and the config.
    // This is how default values can be assigned for example.
    fn new(path: &StatPath, config: &StatConfig) -> Self;

    // Called inside of the StatAccessor after a stat is first added. 
    fn initialize(&self, _stats: &mut Stats) {}

    fn add_modifier(&mut self, path: &StatPath, value: ValueType, config: &StatConfig);
    fn remove_modifier(&mut self, path: &StatPath, value: &ValueType, config: &StatConfig);
    fn set(&mut self, _path: &StatPath, _value: f32, _config: &StatConfig) {}
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32;
}