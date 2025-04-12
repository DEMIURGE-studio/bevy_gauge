use bevy::prelude::*;
use crate::prelude::*;
use evalexpr::{DefaultNumericTypes, Node};

#[derive(Debug, Clone)]
pub struct StatRequirement(pub Node<DefaultNumericTypes>);

impl StatRequirement {
    pub fn met(&self, stats: &Stats) -> bool {
        match self.0.eval_boolean_with_context(stats.get_context()) {
            Ok(result) => return result,
            Err(err) => println!("{:#?}", err),
        }
        false
    }
}

impl From<String> for StatRequirement {
    fn from(value: String) -> Self {
        let expr = evalexpr::build_operator_tree(&value).unwrap();
        Self(expr)
    }
}

impl From<&str> for StatRequirement {
    fn from(value: &str) -> Self {
        let expr = evalexpr::build_operator_tree(&value).unwrap();
        Self(expr)
    }
}

#[derive(Component, Debug, Default, Clone)]
pub struct StatRequirements(pub Vec<StatRequirement>);

impl StatRequirements {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Merges in constraints from another set
    pub fn combine(&mut self, other: &StatRequirements) {
        self.0.append(&mut other.0.clone());
    }

    /// Returns true if all constraints hold.
    pub fn met(&self, stats: &Stats) -> bool {
        for req in self.0.iter() {
            if !req.met(stats) {
                return false;
            }
        }

        return true;
    }
}

impl From<Vec<String>> for StatRequirements {
    fn from(value: Vec<String>) -> Self {
        let mut result: Vec<StatRequirement> = Vec::new();
        for string in value {
            result.push(string.into())
        }
        Self(result)
    }
}

#[cfg(test)]
mod stat_requirement_tests {
    use bevy::prelude::*;
    use crate::prelude::*;

    // Test basic requirement evaluation
    #[test]
    fn test_simple_requirement() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        app.world_mut().spawn(Stats::new());

        // Add some stats to the entity
        let system_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Intelligence.Added", 15.0);
                stat_accessor.add_modifier(entity, "Strength.Added", 10.0);
            }
        });
        let _ = app.world_mut().run_system(system_id);

        // Create and evaluate requirements
        let system_id = app.world_mut().register_system(|query: Query<&Stats>| {
            let stats = query.single();
            
            // Test simple requirements
            let req1 = StatRequirement::from("Intelligence.Added >= 10");
            let req2 = StatRequirement::from("Strength.Added > 15");
            
            // Evaluate requirements
            assert!(req1.met(stats), "Intelligence requirement should be met");
            assert!(!req2.met(stats), "Strength requirement should not be met");
        });
        let _ = app.world_mut().run_system(system_id);
    }

    // Test complex requirements with expressions
    #[test]
    fn test_complex_requirements() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        app.world_mut().spawn(Stats::new());

        // Add some stats to the entity
        let system_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Intelligence.Added", 20.0);
                stat_accessor.add_modifier(entity, "Strength.Added", 15.0);
                stat_accessor.add_modifier(entity, "Dexterity.Added", 12.0);
                
                // Add dependent stats using expressions
                stat_accessor.add_modifier(entity, "SpellPower.Added", "Intelligence.Added * 1.5");
            }
        });
        let _ = app.world_mut().run_system(system_id);

        // Test with complex expressions
        let system_id = app.world_mut().register_system(|query: Query<&Stats>| {
            let stats = query.single();
            
            // Test requirements with expressions
            let req1 = StatRequirement::from("Intelligence.Added + Strength.Added > 30");
            let req2 = StatRequirement::from("Intelligence.Added * 2 > 50");
            let req3 = StatRequirement::from("Dexterity.Added >= 10 && Strength.Added >= 15");
            let req4 = StatRequirement::from("SpellPower.Added > 25");
            
            // Evaluate requirements
            assert!(req1.met(stats), "Combined stat requirement should be met");
            assert!(!req2.met(stats), "Intelligence doubled requirement should not be met");
            assert!(req3.met(stats), "Combined logical condition should be met");
            assert!(req4.met(stats), "Dependent stat requirement should be met");
        });
        let _ = app.world_mut().run_system(system_id);
    }

    // Test StatRequirements collection (all requirements)
    #[test]
    fn test_stat_requirements_collection() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        app.world_mut().spawn(Stats::new());

        // Add some stats to the entity
        let system_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Intelligence.Added", 20.0);
                stat_accessor.add_modifier(entity, "Strength.Added", 10.0);
                stat_accessor.add_modifier(entity, "Level.Added", 5.0);
            }
        });
        let _ = app.world_mut().run_system(system_id);

        // Test StatRequirements collection
        let system_id = app.world_mut().register_system(|query: Query<&Stats>| {
            let stats = query.single();
            
            // Create a collection of requirements
            let mut requirements = StatRequirements::new();
            requirements.0.push(StatRequirement::from("Intelligence.Added >= 15"));
            requirements.0.push(StatRequirement::from("Strength.Added >= 10"));
            requirements.0.push(StatRequirement::from("Level.Added >= 5"));
            
            // Should be met when all requirements are met
            assert!(requirements.met(stats), "All requirements should be met");
            
            // Add a failing requirement
            requirements.0.push(StatRequirement::from("Level.Added > 10"));
            
            // Should fail when any requirement fails
            assert!(!requirements.met(stats), "Requirements should not be met due to level");
        });
        let _ = app.world_mut().run_system(system_id);
    }

    // Test combining StatRequirements
    #[test]
    fn test_combine_requirements() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        app.world_mut().spawn(Stats::new());

        // Add some stats to the entity
        let system_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Intelligence.Added", 20.0);
                stat_accessor.add_modifier(entity, "Strength.Added", 10.0);
                stat_accessor.add_modifier(entity, "Level.Added", 5.0);
            }
        });
        let _ = app.world_mut().run_system(system_id);

        // Test combining requirements
        let system_id = app.world_mut().register_system(|query: Query<&Stats>| {
            let stats = query.single();
            
            // Create base requirements
            let mut base_requirements = StatRequirements::new();
            base_requirements.0.push(StatRequirement::from("Intelligence.Added >= 15"));
            
            // Create additional requirements
            let mut additional_requirements = StatRequirements::new();
            additional_requirements.0.push(StatRequirement::from("Strength.Added >= 10"));
            
            // Combine requirements
            base_requirements.combine(&additional_requirements);
            
            // Should have both requirements
            assert_eq!(base_requirements.0.len(), 2, "Should have 2 requirements after combining");
            assert!(base_requirements.met(stats), "Combined requirements should be met");
            
            // Add a failing requirement
            let mut failing_requirements = StatRequirements::new();
            failing_requirements.0.push(StatRequirement::from("Level.Added > 10"));
            
            // Combine again
            base_requirements.combine(&failing_requirements);
            
            // Should fail now
            assert_eq!(base_requirements.0.len(), 3, "Should have 3 requirements after combining");
            assert!(!base_requirements.met(stats), "Combined requirements should not be met");
        });
        let _ = app.world_mut().run_system(system_id);
    }

    // Test FromVec for StatRequirements
    #[test]
    fn test_from_vec_string() {
        // Setup app
        let mut app = App::new();

        // Spawn an entity with Stats component
        app.world_mut().spawn(Stats::new());

        // Add some stats to the entity
        let system_id = app.world_mut().register_system(|mut stat_accessor: StatAccessor, query: Query<Entity, With<Stats>>| {
            for entity in &query {
                stat_accessor.add_modifier(entity, "Intelligence.Added", 20.0);
                stat_accessor.add_modifier(entity, "Strength.Added", 10.0);
            }
        });
        let _ = app.world_mut().run_system(system_id);

        // Test From<Vec<String>> implementation
        let system_id = app.world_mut().register_system(|query: Query<&Stats>| {
            let stats = query.single();
            
            // Create requirements from a vector of strings
            let requirements_vec = vec![
                "Intelligence.Added >= 15".to_string(),
                "Strength.Added >= 10".to_string(),
            ];
            
            let requirements = StatRequirements::from(requirements_vec);
            
            // Check conversion and evaluation
            assert_eq!(requirements.0.len(), 2, "Should have 2 requirements from vector");
            assert!(requirements.met(stats), "Requirements from vector should be met");
            
            // Test with a failing requirement
            let requirements_vec = vec![
                "Intelligence.Added >= 15".to_string(),
                "Strength.Added > 15".to_string(),  // This will fail
            ];
            
            let requirements = StatRequirements::from(requirements_vec);
            
            assert_eq!(requirements.0.len(), 2, "Should have 2 requirements from vector");
            assert!(!requirements.met(stats), "Requirements should not be met due to strength");
        });
        let _ = app.world_mut().run_system(system_id);
    }
}