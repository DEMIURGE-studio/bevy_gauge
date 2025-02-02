use bevy_ecs::component::Component;
use bevy_utils::HashMap;
use crate::prelude::*;

#[derive(Component)]
pub struct StatEffectTemplate {
    effects: HashMap<String, StatType>,
}

impl StatEffectTemplate {
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
        }
    }

    pub fn build(&self, stats: &StatContextRefs) -> StatEffectInstance {
        let mut instance = StatEffectInstance::new();
        for (stat, value) in self.effects.iter() {
            let value = value.evaluate(stats);
            instance.effects.insert(stat.to_string(), value);
        }
        instance
    }

    pub fn build_instant(&self, stats: &StatContextRefs) -> InstantStatEffectInstance {
        let mut instance = InstantStatEffectInstance::new();
        for (stat, value) in self.effects.iter() {
            let value = value.evaluate(stats);
            instance.effects.insert(stat, value);
        }
        instance
    }
}

pub struct InstantStatEffectInstance<'a> {
    pub effects: HashMap<&'a str, f32>,
}

impl<'a> InstantStatEffectInstance<'a> {
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
        }
    }

    pub fn apply(&self, stats: &mut Stats) {
        for (stat, value) in self.effects.iter() {
            let _ = stats.add(stat, *value);
        }
    }

    pub fn unapply(&self, stats: &mut Stats) {
        for (stat, value) in self.effects.iter() {
            let _ = stats.add(stat, -value);
        }
    }
}

pub struct StatEffectInstance {
    effects: HashMap<String, f32>,
}

impl StatEffectInstance {
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
        }
    }

    pub fn apply(&self, stats: &mut Stats) {
        for (stat, value) in self.effects.iter() {
            let _ = stats.add(stat, *value);
        }
    }

    pub fn unapply(&self, stats: &mut Stats) {
        for (stat, value) in self.effects.iter() {
            let _ = stats.add(stat, -value);
        }
    }
}