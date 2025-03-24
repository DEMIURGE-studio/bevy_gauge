use std::collections::HashMap;
use bevy::ecs::component::Component;
use evalexpr::{ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};

#[derive(Debug, Clone, Default)]
pub enum ModType {
    #[default]
    Add,
    Mul,
}

/// A collection of stats keyed by their names.
#[derive(Component)]
pub struct StatDefinitions(pub HashMap<String, Stat>);

impl StatDefinitions {
    /// Evaluates a stat by gathering all its parts and combining their values.
    pub fn evaluate(&self, stat: &str) -> f32 {
        let stat_modifier = self.0.get(stat);
        let Some(stat) = stat_modifier else { return 0.0; };

        return stat.evaluate(self);
    }

    pub fn evaluate_cached(&self, stat: &str, cache: &mut HashMap<String, f32>) -> f32 {
        if let Some(&value) = cache.get(stat) {
            return value;
        }
        let stat_modifier = self.0.get(stat);
        let Some(stat_data) = stat_modifier else { return 0.0; };
        let value = stat_data.evaluate_cache(self, cache);
        cache.insert(stat.to_string(), value);
        value
    }

    pub fn add_modifier<T, S>(&mut self, path: S, value: T)
    where
        S: Into<String>,
        T: Into<ValueType>,
    {
        let path_str = path.into();
        let vt: ValueType = value.into();
        let parts: Vec<&str> = path_str.split('.').collect();
        if parts.len() != 2 {
            panic!("Invalid modifier path, expected format: StatName.ModifierType");
        }
        let stat_name = parts[0];
        let mod_type_name = parts[1];

        let stat_modifier = self.0.entry(stat_name.to_string()).or_insert(Stat::default());

        let part = stat_modifier
            .modifier_types
            .entry(mod_type_name.to_string())
            .or_insert_with(|| StatModifierStep::default());

        match vt {
            ValueType::Literal(num) => part.base += num,
            ValueType::Expression(expr) => {
                part.mods.push(expr.clone()); // TODO fix unnecessary cloning
                for var in expr.0.iter_variable_identifiers() {
                    // If the stat doesn't exist, create an empty StatModifier
                    let dep_stat = self.0.entry(var.to_string()).or_insert(Stat::default());
                    
                    // Register the dependency
                    *dep_stat.dependents.entry(stat_name.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    pub fn remove_modifier<T, S>(&mut self, path: S, value: T)
    where
        S: Into<String>,
        T: Into<ValueType>,
    {
        let path_str = path.into();
        let vt: ValueType = value.into();
        let parts: Vec<&str> = path_str.split('.').collect();
        if parts.len() != 2 {
            panic!("Invalid modifier path, expected format: StatName.ModifierType");
        }
        let stat_name = parts[0];
        let mod_type_name = parts[1];

        if let Some(stat_modifier) = self.0.get_mut(stat_name) {
            if let Some(part) = stat_modifier.modifier_types.get_mut(mod_type_name) {
                match vt {
                    ValueType::Literal(num) => {
                        part.base -= num;
                    }
                    ValueType::Expression(expr) => {
                        let target_debug = format!("{:?}", expr.0);
                        if let Some(pos) = part
                            .mods
                            .iter()
                            .position(|e| format!("{:?}", e.0) == target_debug)
                        {
                            let removed_expr = part.mods.remove(pos);
                            for var in removed_expr.0.iter_variable_identifiers() {
                                if let Some(dep_stat) = self.0.get_mut(var) {
                                    if let Some(count) = dep_stat.dependents.get_mut(stat_name) {
                                        *count -= 1;
                                        if *count == 0 {
                                            dep_stat.dependents.remove(stat_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Default)]
pub struct Stat {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, StatModifierStep>,
    pub dependents: HashMap<String, u32>,
}

impl Stat {
    pub fn evaluate(&self, stat_definitions: &StatDefinitions) -> f32 {
        // Evaluate each modifier part and inject them into the context
        let mut context = HashMapContext::new();
        for (name, part) in &self.modifier_types {
            let part_value = part.evaluate(stat_definitions);
            context.set_value(name.clone(), Value::Float(part_value as f64)).unwrap();
        }

        // Evaluate the total expression
        self
            .total
            .0
            .eval_with_context(&context)
            .unwrap()
            .as_number()
            .unwrap() as f32
    }
    
    pub fn evaluate_cache(&self, stat_definitions: &StatDefinitions, cache: &mut HashMap<String, f32>) -> f32 {
        // Evaluate each modifier part and inject them into the context
        let mut context = HashMapContext::new();
        for (name, part) in &self.modifier_types {
            let part_value = part.evaluate_cache(stat_definitions, cache);
            context.set_value(name.clone(), Value::Float(part_value as f64)).unwrap();
        }

        // Evaluate the total expression
        self
            .total
            .0
            .eval_with_context(&context)
            .unwrap()
            .as_number()
            .unwrap() as f32
    }
}

#[derive(Default)]
pub struct StatModifierStep {
    pub cached: f32,
    pub relationship: ModType,
    pub base: f32,
    pub mods: Vec<Expression>,
}

impl StatModifierStep {
    pub fn evaluate(&self, stat_definitions: &StatDefinitions) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stat_definitions)).collect();

        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => computed.iter().fold(1.0, |acc, &x| acc * x) * self.base,
        }
    }
    
    pub fn evaluate_cache(&self, stat_definitions: &StatDefinitions, cache: &mut HashMap<String, f32>) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate_cache(stat_definitions, cache)).collect();

        let value = match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => computed.iter().fold(1.0, |acc, &x| acc * x) * self.base,
        };

        return value;
    }
}

#[derive(Debug, Clone)]
pub struct Expression(pub Node<DefaultNumericTypes>);

impl Expression {
    pub fn evaluate(&self, stat_definitions: &StatDefinitions) -> f32 {
        let mut context = HashMapContext::new();
        for var_name in self.0.iter_variable_identifiers() {
            let val = stat_definitions.evaluate(var_name);
            context.set_value(var_name.to_string(), Value::Float(val as f64)).unwrap();
        }
        self.0.eval_with_context(&context).unwrap().as_number().unwrap() as f32
    }
    
    pub fn evaluate_cache(&self, stat_definitions: &StatDefinitions, cache: &mut HashMap<String, f32>) -> f32 {
        let mut context = HashMapContext::new();
        for var_name in self.0.iter_variable_identifiers() {
            let val = stat_definitions.evaluate_cached(var_name, cache);
            context.set_value(var_name.to_string(), Value::Float(val as f64)).unwrap();
        }
        self.0.eval_with_context(&context).unwrap().as_number().unwrap() as f32
    }
}

impl Default for Expression {
    fn default() -> Self {
        Self(evalexpr::build_operator_tree("0").unwrap())
    }
}

#[derive(Debug, Clone)]
pub enum ValueType {
    Literal(f32),
    Expression(Expression),
}

impl Default for ValueType {
    fn default() -> Self {
        Self::Literal(0.0)
    }
}

impl From<&str> for ValueType {
    fn from(value: &str) -> Self {
        Self::Expression(Expression(evalexpr::build_operator_tree(value).unwrap()))
    }
}

impl From<String> for ValueType {
    fn from(value: String) -> Self {
        Self::Expression(Expression(evalexpr::build_operator_tree(&value).unwrap()))
    }
}

impl From<f32> for ValueType {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

impl From<u32> for ValueType {
    fn from(value: u32) -> Self {
        Self::Literal(value as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evalexpr::build_operator_tree;

    #[test]
    fn test_evaluate_stat_with_literals() {
        let mut stats = StatDefinitions(HashMap::new());
        // Add two literal modifiers: 50.0 and 25.0
        stats.add_modifier("Damage.Add", 50.0_f32);
        stats.add_modifier("Damage.Add", 25.0_f32);
        // Set total expression to be the value of "Add"
        if let Some(stat) = stats.0.get_mut("Damage") {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
        let value = stats.evaluate("Damage");
        assert_eq!(value, 75.0);
    }

    #[test]
    fn test_evaluate_stat_with_expression() {
        let mut stats = StatDefinitions(HashMap::new());
        // Set Strength to 100.0
        stats.add_modifier("Strength.Add", 100.0_f32);
        // Health has a base of 200.0 and an additional modifier "Strength / 5"
        stats.add_modifier("Health.Add", 200.0_f32);
        stats.add_modifier("Health.Add", ValueType::from("Strength / 5"));
        
        for (_, stat) in stats.0.iter_mut() {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
        let value = stats.evaluate("Health");
        // 200.0 + (100.0 / 5) = 200.0 + 20.0 = 220.0
        assert_eq!(value, 220.0);
    }

    #[test]
    fn test_evaluate_cached() {
        let mut stats = StatDefinitions(HashMap::new());
        stats.add_modifier("Armor.Add", 150.0_f32);
        if let Some(stat) = stats.0.get_mut("Armor") {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
        let mut cache = HashMap::new();
        let value1 = stats.evaluate_cached("Armor", &mut cache);
        let value2 = stats.evaluate_cached("Armor", &mut cache);
        assert_eq!(value1, 150.0);
        assert_eq!(value2, 150.0);
        // Verify that the cache contains the correct entry.
        assert_eq!(cache.get("Armor").unwrap(), &150.0);
    }

    #[test]
    fn test_evaluate_cached_deep_dependencies() {
        let mut stats = StatDefinitions(HashMap::new());
    
        // Base stats (Literals)
        stats.add_modifier("Strength.Add", 100.0_f32);    // Strength = 100.0
        stats.add_modifier("Endurance.Add", 80.0_f32);    // Base Endurance = 80.0
        stats.add_modifier("Vitality.Add", 120.0_f32);    // Vitality = 120.0
    
        // Dependent stat (Expression)
        stats.add_modifier("Endurance.Add", ValueType::from("Strength * 0.1")); // Endurance depends on Strength
    
        // Health calculation based on multiple dependencies
        stats.add_modifier("Health.Add", 200.0_f32);      // Base Health = 200.0
        stats.add_modifier("Health.Add", ValueType::from("Strength * 0.5 + Endurance * 0.2 + Vitality * 0.3"));
    
        // Define total expressions for each stat
        if let Some(stat) = stats.0.get_mut("Strength") {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
        if let Some(stat) = stats.0.get_mut("Endurance") {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
        if let Some(stat) = stats.0.get_mut("Vitality") {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
        if let Some(stat) = stats.0.get_mut("Health") {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
    
        let mut cache = HashMap::new();
    
        // First evaluation (cache is empty)
        let health_value1 = stats.evaluate_cached("Health", &mut cache);
        let strength_value1 = *cache.get("Strength").unwrap();
        let endurance_value1 = *cache.get("Endurance").unwrap();
        let vitality_value1 = *cache.get("Vitality").unwrap();
    
        // Second evaluation (should use the cache)
        let health_value2 = stats.evaluate_cached("Health", &mut cache);
    
        // Check if the results are identical
        assert_eq!(health_value1, health_value2);
    
        // Manually calculate expected values
        let expected_strength = 100.0;
        let expected_endurance = 80.0 + (100.0 * 0.1); // 80.0 + 10.0 = 90.0
        let expected_vitality = 120.0;
    
        let expected_health = 200.0
            + (expected_strength * 0.5)
            + (expected_endurance * 0.2)
            + (expected_vitality * 0.3);
    
        // Compare calculated values with the expected values
        assert_eq!(strength_value1, expected_strength);
        assert_eq!(endurance_value1, expected_endurance);
        assert_eq!(vitality_value1, expected_vitality);
        assert_eq!(health_value1, expected_health);
    
        // Print the results for debugging purposes
        println!("Strength (cached): {}", strength_value1);
        println!("Endurance (cached): {}", endurance_value1);
        println!("Vitality (cached): {}", vitality_value1);
        println!("Health (cached): {}", health_value1);
    
        // Also verify the cache contents
        assert!(cache.contains_key("Strength"));
        assert!(cache.contains_key("Endurance"));
        assert!(cache.contains_key("Vitality"));
        assert!(cache.contains_key("Health"));
    }
    

    #[test]
    fn test_remove_modifier() {
        let mut stats = StatDefinitions(HashMap::new());
        // Add two literal modifiers for Mana: 80.0 and 20.0
        stats.add_modifier("Mana.Add", 80.0_f32);
        stats.add_modifier("Mana.Add", 20.0_f32);

        for (_, stat) in stats.0.iter_mut() {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }

        let initial_value = stats.evaluate("Mana");
        assert_eq!(initial_value, 100.0);
        // Remove the 20.0 literal modifier.
        stats.remove_modifier("Mana.Add", 20.0_f32);
        let value_after_removal = stats.evaluate("Mana");
        assert_eq!(value_after_removal, 80.0);

        // Now add an expression modifier.
        // First, add Strength so the expression can reference it.
        stats.add_modifier("Strength.Add", 50.0_f32);

        // Add an expression modifier: "Strength / 2" which should evaluate to 25.0.
        stats.add_modifier("Mana.Add", ValueType::from("Strength / 2"));
        for (_, stat) in stats.0.iter_mut() {
            stat.total = Expression(build_operator_tree("Add").unwrap());
        }
        let value_with_expr = stats.evaluate("Mana");
        // Expect 80.0 (base) + 25.0 (from expression) = 105.0.
        assert_eq!(value_with_expr, 105.0);

        // Remove the expression modifier.
        stats.remove_modifier("Mana.Add", ValueType::from("Strength / 2"));
        let final_value = stats.evaluate("Mana");
        assert_eq!(final_value, 80.0);
    }
}