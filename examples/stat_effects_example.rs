use bevy::prelude::*; // Assuming Bevy is used for Entity and Commands
use bevy_gauge::prelude::{StatEffect, StatEffectContext, StatAccessor}; // Assuming this path for your crate's items

struct DamageEffect {
    value: f32,
}

struct Rng {}

struct DamageEffectContext<'a> {
    origin: &'a Entity,
    target: &'a Entity,
    rng: &'a Rng,
    commands: &'a mut Commands<'a, 'a>,
}

impl<'a> StatEffectContext for DamageEffectContext<'a> {}

impl<'a> StatEffect for &'a DamageEffect {
    type Context = DamageEffectContext<'a>;
   
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target_evasion_rating = stat_accessor.get(*context.target, "Evasion");
        let origin_accuracy_rating = stat_accessor.get(*context.origin, "Accuracy");
        let hit = true; // use context.rng with target_evasion and origin_accuracy to decide if the hit goes off
        // you can add commands so you can do stuff like fire triggers. You could have your entire damage-pipeline in here.

        if hit {
            stat_accessor.add_modifier(*context.target, "LifeCurrent", -self.value);
        }
    }
}

struct HealEffect {
    value: f32,
}

impl StatEffect for HealEffect {
    fn apply(&self, stat_accessor: &mut StatAccessor, context: &Self::Context) {
        let target = context;
        todo!()
    }
}

fn main() {}