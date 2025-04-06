use super::prelude::*;

pub trait StatLike {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V);
    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V);
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32;
    fn on_insert(&self, stats: &Stats, stat_path: &StatPath);
}