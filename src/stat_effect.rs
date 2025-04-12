use bevy::ecs::entity::Entity;
use crate::prelude::StatAccessor;

// The base context trait
trait StatEffectContext {}

// Default context with target entity
struct DefaultContext {
    target: Entity,
}

impl StatEffectContext for DefaultContext {}

// Define the trait with a lifetime parameter
trait StatEffect {
    // The Context type is defined without explicit lifetimes in the trait
    type Context: StatEffectContext;
   
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context);
}

struct DamageEffect {
    value: f32,
}

struct Rng {}

struct DamageEffectContext<'a> {
    origin: Entity,
    target: Entity,
    rng: &'a Rng,
}

impl<'a> StatEffectContext for DamageEffectContext<'a> {}

impl<'a> StatEffect for &'a DamageEffect {
    type Context = DamageEffectContext<'a>;
   
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        todo!()
    }
}

struct HealEffect {
    value: f32,
}

impl StatEffect for HealEffect {
    type Context = DefaultContext;
    
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target = context.target;
        todo!()
    }
}