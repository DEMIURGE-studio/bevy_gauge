use super::prelude::*;

pub trait Stat {
    fn new(path: &StatPath, config: &Config) -> Self;

    // Called inside of the StatAccessor after a stat is first added. 
    fn initialize(&self, _stats: &mut Stats) {}

    fn add_modifier(&mut self, path: &StatPath, value: ModifierType, config: &Config);
    fn remove_modifier(&mut self, path: &StatPath, value: &ModifierType);
    fn set(&mut self, _path: &StatPath, _value: f32) {}
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32;
}