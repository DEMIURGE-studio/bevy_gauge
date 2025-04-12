use bevy::ecs::entity::Entity;
use crate::prelude::StatAccessorMut;

trait StatEffectContext {}

struct DefaultContext {
    pub target: Entity,
}

impl StatEffectContext for DefaultContext {}

trait StatEffect {
    type Context: StatEffectContext = DefaultContext;
    
    fn apply(&self, stat_accessor: &mut StatAccessorMut, context: &Self::Context);
}

struct DamageEffect {
    value: f32,
}

struct DamageEffectContext {
    origin: Entity,
    target: Entity,
}

impl StatEffectContext for DamageEffectContext {}

impl StatEffect for DamageEffect {
    type Context = DamageEffectContext;
    
    fn apply(&self, stat_accessor: &mut StatAccessorMut, context: &Self::Context) {
        todo!()
    }
}

struct HealEffect {
    value: f32,
}

impl StatEffect for HealEffect {
    fn apply(&self, stat_accessor: &mut StatAccessorMut, context: &Self::Context) {
        let target = context.target;
        todo!()
    }
}