use bevy::{prelude::*, utils::HashMap};
use crate::prelude::{StatAccessorMut, ValueType};

#[derive(Clone, Deref, DerefMut)]
pub struct ModifierSet(HashMap<String, Vec<ValueType>>);

impl ModifierSet {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn apply(&self, stat_accessor: &mut StatAccessorMut, target_entity: Entity) {
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.add_modifier_value(target_entity, stat, modifier.clone());
            }
        }
    }

    pub fn remove(&self, stat_accessor: &mut StatAccessorMut, target_entity: Entity) {
        for (stat, modifiers) in self.0.iter() {
            for modifier in modifiers.iter() {
                stat_accessor.remove_modifier_value(target_entity, stat, modifier);
            }
        }
    }
}

#[cfg(test)]
mod modifier_set_tests {
    use bevy::prelude::*;
    use crate::prelude::*;

    // Test basic ModifierSet functionality - applying simple modifiers
    #[test]
    fn test_modifier_set_apply() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to apply a ModifierSet
        let system_id = app.world_mut().register_system(|mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Create a ModifierSet with multiple stats and modifiers
                let mut modifier_set = ModifierSet::new();
                
                // Add some modifiers to the set
                modifier_set.0.entry("Life.Added".to_string())
                    .or_insert_with(Vec::new)
                    .push(ValueType::Literal(10.0));
                
                modifier_set.0.entry("Mana.Added".to_string())
                    .or_insert_with(Vec::new)
                    .push(ValueType::Literal(20.0));
                
                modifier_set.0.entry("Strength.Added".to_string())
                    .or_insert_with(Vec::new)
                    .push(ValueType::Literal(5.0));
                
                // Apply the ModifierSet to the entity
                modifier_set.apply(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify all modifiers in the set were applied
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let life_value = stats.get("Life.Added").unwrap_or(0.0);
        let mana_value = stats.get("Mana.Added").unwrap_or(0.0);
        let strength_value = stats.get("Strength.Added").unwrap_or(0.0);
        
        assert_eq!(life_value, 10.0);
        assert_eq!(mana_value, 20.0);
        assert_eq!(strength_value, 5.0);
    }

    // Test applying a ModifierSet with expressions
    #[test]
    fn test_modifier_set_with_expressions() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Register and run the system to apply a ModifierSet with expressions
        let system_id = app.world_mut().register_system(|mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Create a ModifierSet
                let mut modifier_set = ModifierSet::new();
                
                // Add base stats
                modifier_set.0.entry("Strength.Added".to_string())
                    .or_insert_with(Vec::new)
                    .push(ValueType::Literal(10.0));
                
                modifier_set.0.entry("Intelligence.Added".to_string())
                    .or_insert_with(Vec::new)
                    .push(ValueType::Literal(20.0));
                
                // Add derived stats using expressions
                modifier_set.0.entry("PhysicalDamage.Added".to_string())
                    .or_insert_with(Vec::new)
                    .push(ValueType::from("Strength.Added * 2.0".to_string()));
                
                modifier_set.0.entry("SpellDamage.Added".to_string())
                    .or_insert_with(Vec::new)
                    .push(ValueType::from("Intelligence.Added * 1.5".to_string()));
                
                // Apply the ModifierSet
                modifier_set.apply(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify the expressions were evaluated correctly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let physical_damage = stats.evaluate_by_string("PhysicalDamage.Added");
        let spell_damage = stats.evaluate_by_string("SpellDamage.Added");
        
        assert_eq!(physical_damage, 20.0); // 10.0 * 2.0
        assert_eq!(spell_damage, 30.0);    // 20.0 * 1.5
    }

    // Test removing a ModifierSet
    #[test]
    fn test_modifier_set_remove() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();

        // Create a ModifierSet to test with
        let mut test_modifier_set = ModifierSet::new();
        
        test_modifier_set.0.entry("Life.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::Literal(15.0));
        
        test_modifier_set.0.entry("Mana.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::Literal(25.0));
        
        // Clone the ModifierSet for use in the second system
        let test_modifier_set_clone = test_modifier_set.clone();

        // Register and run a system to apply the ModifierSet
        let apply_system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Apply the test ModifierSet
                test_modifier_set.apply(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(apply_system_id);
        
        // Verify the ModifierSet was applied
        let initial_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let initial_life = initial_stats.get("Life.Added").unwrap_or(0.0);
        let initial_mana = initial_stats.get("Mana.Added").unwrap_or(0.0);
        
        assert_eq!(initial_life, 15.0);
        assert_eq!(initial_mana, 25.0);
        
        // Now run a system to remove the ModifierSet, using the clone
        let remove_system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Remove the test ModifierSet
                test_modifier_set_clone.remove(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(remove_system_id);
        
        // Verify the ModifierSet was removed
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let updated_life = updated_stats.get("Life.Added").unwrap_or(0.0);
        let updated_mana = updated_stats.get("Mana.Added").unwrap_or(0.0);
        
        assert_eq!(updated_life, 0.0);
        assert_eq!(updated_mana, 0.0);
    }

    // Test applying and removing ModifierSets with mixed modifier types
    #[test]
    fn test_modifier_set_mixed_types() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();
        
        // Create a ModifierSet with multiple types of modifiers
        let mut test_modifier_set = ModifierSet::new();
        
        // Add flat modifiers
        test_modifier_set.0.entry("Life.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::Literal(10.0));
        
        // Add expression modifiers
        test_modifier_set.0.entry("Damage.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::from("Life.Added * 0.5".to_string()));
        
        // Add multiple modifiers to the same stat
        test_modifier_set.0.entry("Life.Added".to_string())
            .or_default()
            .push(ValueType::from(5.0));
        
        // Clone for use in the second system
        let test_modifier_set_clone = test_modifier_set.clone();
        
        // Register and run a system to apply the ModifierSet
        let apply_system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                test_modifier_set.apply(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(apply_system_id);
        
        // Verify the ModifierSet was applied correctly
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let life_value = stats.get("Life.Added").unwrap_or(0.0);
        let damage_value = stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(life_value, 15.0);  // 10.0 + 5.0
        assert_eq!(damage_value, 7.5); // 15.0 * 0.5
        
        // Now run a system to remove the ModifierSet
        let remove_system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                test_modifier_set_clone.remove(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(remove_system_id);
        
        // Verify all modifiers were removed
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let updated_life = updated_stats.get("Life.Added").unwrap_or(0.0);
        let updated_damage = updated_stats.evaluate_by_string("Damage.Added");
        
        assert_eq!(updated_life, 0.0);
        assert_eq!(updated_damage, 0.0);
    }

    // Test applying multiple ModifierSets and removing only one
    #[test]
    fn test_multiple_modifier_sets() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        let entity = app.world_mut().spawn(Stats::new()).id();
        
        // Create the first ModifierSet - base stats
        let mut base_modifier_set = ModifierSet::new();
        base_modifier_set.0.entry("Life.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::Literal(100.0));
        
        base_modifier_set.0.entry("Mana.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::Literal(50.0));
        
        // Create the second ModifierSet - buff effect
        let mut buff_modifier_set = ModifierSet::new();
        buff_modifier_set.0.entry("Life.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::Literal(20.0));
        
        buff_modifier_set.0.entry("LifeRegen.Added".to_string())
            .or_insert_with(Vec::new)
            .push(ValueType::Literal(5.0));
        
        // Clone both for the first system
        let base_modifier_set_clone1 = base_modifier_set.clone();
        let buff_modifier_set_clone1 = buff_modifier_set.clone();
        
        // Register and run systems to apply both ModifierSets
        let system_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Apply both ModifierSets
                base_modifier_set_clone1.apply(&mut stat_accessor, entity);
                buff_modifier_set_clone1.apply(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(system_id);
        
        // Verify both ModifierSets were applied
        let stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let life_value = stats.get("Life.Added").unwrap_or(0.0);
        let mana_value = stats.get("Mana.Added").unwrap_or(0.0);
        let regen_value = stats.get("LifeRegen.Added").unwrap_or(0.0);
        
        assert_eq!(life_value, 120.0); // 100.0 + 20.0
        assert_eq!(mana_value, 50.0);  // From base set
        assert_eq!(regen_value, 5.0);  // From buff set
        
        // Now run a system to remove just the buff ModifierSet
        let remove_buff_id = app.world_mut().register_system(move |mut stat_accessor: StatAccessorMut, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                // Remove only the buff ModifierSet
                buff_modifier_set.remove(&mut stat_accessor, entity);
            }
        });
        let _ = app.world_mut().run_system(remove_buff_id);
        
        // Verify only the buff ModifierSet was removed, base stats remain
        let updated_stats = app.world_mut().query::<&Stats>().get(app.world(), entity).unwrap();
        
        let updated_life = updated_stats.get("Life.Added").unwrap_or(0.0);
        let updated_mana = updated_stats.get("Mana.Added").unwrap_or(0.0);
        let updated_regen = updated_stats.get("LifeRegen.Added").unwrap_or(0.0);
        
        assert_eq!(updated_life, 100.0); // Buff removed, only base remains
        assert_eq!(updated_mana, 50.0);  // Unchanged
        assert_eq!(updated_regen, 0.0);  // Buff removed
    }
}