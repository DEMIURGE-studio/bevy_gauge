use bevy::prelude::*;
use bevy_gauge::prelude::StatsMutator;

struct DamageEffect {
    value: f32,
}

// Placeholder components - in real usage these would be proper game systems
struct Rng;

// OLD
// struct DamageEffectContext<'w, 's> {
//     stats_mutator: StatsMutator<'w, 's>,
//     origin: Entity,
//     target: Entity,
//     rng: Rng,
//     commands: Commands<'w, 's>,
// }
// 
// impl<'w, 's> StatEffectContext for DamageEffectContext<'w, 's> {}
// 
// impl<'w, 's> StatEffect<DamageEffectContext<'w, 's>> for DamageEffect {
//     fn apply(&self, context: &mut DamageEffectContext<'w, 's>) {
//         // Access stats through the context
//         let _target_evasion_rating = context.stats_mutator.get(context.target, "Evasion");
//         let _origin_accuracy_rating = context.stats_mutator.get(context.origin, "Accuracy");
//         
//         // Use context.rng with target_evasion and origin_accuracy to decide if the hit goes off
//         let hit = true; // placeholder logic
//         
//         // You can add commands so you can do stuff like fire triggers. 
//         // You could have your entire damage pipeline in here.
//         if hit {
//             context.stats_mutator.add_modifier(context.target, "LifeCurrent", -self.value);
//         }
//     }
// }

struct HealEffect {
    value: f32,
}

// OLD
// Simple heal effect using the default context
// impl<'w, 's> StatEffect<DefaultStatEffectContext<'w, 's>> for HealEffect {
//     fn apply(&self, context: &mut DefaultStatEffectContext<'w, 's>) {
//         // Add healing to the target entity
//         context.stats_mutator.add_modifier(context.target, "LifeCurrent", self.value);
//     }
// }

fn main() {
    // This example shows the pattern for implementing StatEffect with custom contexts
}