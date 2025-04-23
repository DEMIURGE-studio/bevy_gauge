use crate::prelude::Stats;

pub trait StatDerived {
    fn from_stats(stats: &Stats) -> Self;

    fn should_update(&self, stats: &Stats) -> bool;

    fn update_from_stats(&mut self, stats: &Stats);

    fn is_valid(stats: &Stats) -> bool;
}

pub trait WriteBack {
    fn write_back(&self, stats: &mut Stats);
}