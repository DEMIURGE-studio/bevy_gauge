
use bevy::app::App;

pub mod app_extension;
pub mod components;
pub mod dirty;
pub mod eval_context;
pub mod macros;
pub mod prelude;
pub mod requirements;
pub mod schedule;
pub mod serialization;
pub mod stat_effect;
pub mod systems;
pub mod traits;
pub mod stats;
pub mod effects;
pub mod tag_tree;

#[cfg(test)]
mod tests {
    use crate::effects::ModifierValue;
    use crate::tag_tree::*;

    #[test]
    fn test_structured_tags() {
        let mut registry = ModifierRegistry::new();

        // Register modifiers
        let fire_spell_mod = ModifierValue { flat: 10.0, increased: 0.2, more: 1.5 };
        let ranged_attack_mod = ModifierValue { flat: 5.0, increased: 0.1, more: 1.2 };
        let any_fire_mod = ModifierValue { flat: 2.0, increased: 0.05, more: 1.1 };

        // 50% increased damage for all ranged elemental attacks
        registry.register(
            TagQuery::new("Damage")
                .with_attr("AbilityType", "Attack")
                .with_any("Element")
                .with_attr("Origin", "Ranged"),
            ranged_attack_mod.clone()
        );

        // 50% increased damage for all fire spells
        registry.register(
            TagQuery::new("Damage")
                .with_attr("AbilityType", "Spell")
                .with_attr("Element", "Fire"),
            fire_spell_mod.clone()
        );

        // Flat bonus to any fire damage
        registry.register(
            TagQuery::new("Damage")
                .with_attr("Element", "Fire"),
            any_fire_mod.clone()
        );

        // Query: Fire spell damage
        let spell_query = TagQuery::new("Damage")
            .with_attr("AbilityType", "Spell")
            .with_attr("Element", "Fire");

        let result = registry.query(&spell_query);

        // Should match fire_spell_mod and any_fire_mod
        let expected = ModifierValue {
            flat: fire_spell_mod.flat + any_fire_mod.flat,
            increased: fire_spell_mod.increased + any_fire_mod.increased,
            more: fire_spell_mod.more * any_fire_mod.more,
        };

        assert_eq!(result.flat, expected.flat);
        assert_eq!(result.increased, expected.increased);
        assert!((result.more - expected.more).abs() < f32::EPSILON);
    }

    #[test]
    fn test_multivalued_attributes() {
        let mut registry = ModifierRegistry::new();

        // Define a modifier for fire and ice
        let fire_ice_mod = ModifierValue { flat: 15.0, increased: 0.25, more: 1.4 };

        // 50% increased damage for all fire and ice attacks
        registry.register(
            TagQuery::new("Damage")
                .with_attr("AbilityType", "Attack")
                .with_attrs("Element", vec!["Fire", "Ice"]),
            fire_ice_mod.clone()
        );

        // Query: Fire attack damage
        let fire_query = TagQuery::new("Damage")
            .with_attr("AbilityType", "Attack")
            .with_attr("Element", "Fire");

        let result = registry.query(&fire_query);

        // Should match fire_ice_mod
        assert_eq!(result.flat, fire_ice_mod.flat);
        assert_eq!(result.increased, fire_ice_mod.increased);
        assert!((result.more - fire_ice_mod.more).abs() < f32::EPSILON);

        // Query: Lightning attack damage
        let lightning_query = TagQuery::new("Damage")
            .with_attr("AbilityType", "Attack")
            .with_attr("Element", "Lightning");

        let result = registry.query(&lightning_query);

        // Should not match any modifiers
        assert_eq!(result.flat, 0.0);
        assert_eq!(result.increased, 0.0);
        assert_eq!(result.more, 1.0);
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins((
        schedule::plugin,
        components::plugin,
    ));
}