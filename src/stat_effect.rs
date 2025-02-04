use bevy_ecs::{component::Component, entity::Entity};
use bevy_utils::HashMap;
use crate::prelude::*;

#[derive(Component, Clone)]
pub struct StatEffect {
    effects: HashMap<String, StatType>,
}

impl StatEffect {
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

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum StatEffectId {
    Entity(Entity),
    String(String),
    UID(u64),
}

#[derive(Component, Clone)]
pub struct PersistentStatEffects {
    pub effects: HashMap<StatEffectId, StatEffect>,
}

impl PersistentStatEffects {
    pub fn new() -> Self {
        Self { effects: HashMap::new() }
    }

    pub fn try_add_persistent_effect(&mut self, id: StatEffectId, stat_effect: &StatEffect, target: Entity, stat_accessor: &mut StatAccessor) {
        if self.effects.contains_key(&id) {
            return;
        }

        stat_accessor.apply_effect(target, stat_effect);

        self.effects.insert(id, stat_effect.clone());
    }
}

/// We have something like persistent stat effects working. Persistent stat effects are things like 
/// > Equipment
/// > Talents
/// > Buffs/debuffs
/// > etc
/// 
/// Anything that lasts some amount of time and which may be removed later. Stat effects should be
/// a first class feature and play niceley with other features. Lets look at some problems with the
/// above implementation. 
/// 
/// 1. It is rigid. Right now there is a component with a hashmap. This -could- work and -could- be
/// good enough. But what if I want to handle my Talents and Equipment on separate components? Why
/// should I maintain an "EquipmentSlots" component AND a "Talents" component that both do the same
/// thing?
/// 
/// 2. Storage - A hashmap of effects is VERY perscribed. Instead the user should be able to have a
/// vec if they want to, like I use for EquipmentSlots.
/// 
/// 3. StatEffectId is bad. Same problem as 2 tho.
/// 
/// So instead we can have a trait that governs PersistentStatEffects. Then you can implement that 
/// trait for different components, and as long as you implement that trait correctly it will just
/// work.
/// 
/// Possible features:
/// 
/// 1. can_add function that checks if certain requirements are met to add a effect. Useful for gear,
/// talents, or anything else that has stat requirements. Also try_add, add
/// 
/// 2. Store persistent effects as entities to avoid cloning StatEffects. 
/// 
/// 3. Allow expression-based persistent effects, which apply diffs to entities they are equipped to.
/// 
/// 4. Create add/remove conditions, that allow effects to be added or removed to entities based on
/// proximity, team, some event firing, etc.


pub trait PersistentStatEffect {

}

/// Defines requirements for a stat effect to be initially applied
pub trait StatEffectRequirement {

}

/// Defines conditions for removing or deactivating a stat effect
pub trait StatEffectCondition {

}

