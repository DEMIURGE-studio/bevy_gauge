use bevy::prelude::*; // Assuming Bevy is used for Entity and Commands
use bevy_gauge::prelude::{StatEffect, StatEffectContext, StatsMutator}; // Assuming this path for your crate's items

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
   
    fn apply(&self, stats_mutator: &mut StatsMutator, context: &Self::Context) {
        let target_evasion_rating = stats_mutator.get(*context.target, "Evasion");
        let origin_accuracy_rating = stats_mutator.get(*context.origin, "Accuracy");
        let hit = true; // use context.rng with target_evasion and origin_accuracy to decide if the hit goes off
        // you can add commands so you can do stuff like fire triggers. You could have your entire damage-pipeline in here.

        if hit {
            stats_mutator.add_modifier(*context.target, "LifeCurrent", -self.value);
        }
    }
}

struct HealEffect {
    value: f32,
}

impl StatEffect for HealEffect {
    fn apply(&self, stats_mutator: &mut StatsMutator, context: &Self::Context) {
        let target = context;
        todo!()
    }
}

fn main() {}