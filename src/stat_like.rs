use super::prelude::*;

pub trait StatLike {
    fn add_modifier(&mut self, stat_path: &StatPath, value: ValueType);
    fn remove_modifier(&mut self, stat_path: &StatPath, value: &ValueType);
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32;
    fn on_insert(&self, stats: &Stats, stat_path: &StatPath);
}