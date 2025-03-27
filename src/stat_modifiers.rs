use bevy::{ecs::component::Component, utils::HashMap};
use evalexpr::{Context, ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};

#[derive(Debug, Clone, Default)]
pub enum ModType {
    #[default]
    Add,
    Mul,
}

// TODO eval_expr doesn't know about my query types. It doesn't know what to do with Damage.101010.Added. It just assumes it's junk. I need to do
// some custom parsing.

// TODO Fix overuse of .unwrap(). It's fine for now (maybe preferable during development) but in the future we'll want proper errors, panics, and 
// warnings.

/// A collection of stats keyed by their names.
#[derive(Component, Debug)]
pub struct StatDefinitions(pub HashMap<String, StatType>);

impl StatDefinitions {
    /// Evaluates a stat by gathering all its parts and combining their values.
    pub fn evaluate<S>(&self, path: S) -> f32 
    where
        S: Into<String>,
    {
        let path_str = path.into();
        let segments: Vec<&str> = path_str.split("_").collect();
        let head = segments[0];

        let stat_type = self.0.get(head);
        let Some(stat_type) = stat_type else { return 0.0; };

        stat_type.evaluate(&segments, self)
    }

    pub fn add_modifier<V, S>(&mut self, path: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType> + Clone,
    {
        let path_str = path.into();
        let segments: Vec<&str> = path_str.split("_").collect();
        let base_path = segments[0].to_string();

        if let Some(stat) = self.0.get_mut(&base_path) {
            stat.add_modifier(&segments, value.clone());
        } else {
            self.0.insert(base_path.clone(), StatType::new(&path_str, value.clone()));
        }

        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(_) => (),
            ValueType::Expression(expression) => {
                self.add_dependent(&base_path, &expression);
            },
        }
    }

    pub fn remove_modifier<V, S>(&mut self, path: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType> + Clone,
    {
        let path_str = path.into();
        let segments: Vec<&str> = path_str.split("_").collect();
        let base_path = segments[0].to_string();

        if let Some(stat) = self.0.get_mut(&base_path) {
            stat.remove_modifier(&segments[1..], value.clone());
        }

        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(_) => (),
            ValueType::Expression(expression) => {
                self.remove_dependent(&base_path, &expression);
            },
        }
    }

    fn add_dependent(&mut self, dependent_path: &str, expression: &Expression) {
        for var in expression.value.iter_variable_identifiers() {
            let segments: Vec<&str> = var.split("_").collect();
            let var_base = segments[0]; // The root stat name
    
            // If the stat doesn't exist, create it as DependentOnly
            if !self.0.contains_key(var_base) {
                self.0.insert(var_base.to_string(), StatType::DependentOnly(DependentOnly::default()));
            }
    
            // Add the dependent relationship
            if let Some(dep_stat) = self.0.get_mut(var_base) {
                dep_stat.add_dependent(dependent_path.to_string());
            }
        }
    }

    fn remove_dependent(&mut self, dependent_path: &str, expression: &Expression) {
        for var in expression.value.iter_variable_identifiers() {
            let segments: Vec<&str> = var.split("_").collect();
            let var_base = segments[0]; // The root stat name

            if let Some(dep_stat) = self.0.get_mut(var_base) {
                dep_stat.remove_dependent(dependent_path);
            }
        }
    }
}

pub trait StatLike {
    fn add_dependent(&mut self, dependent: String);
    fn remove_dependent(&mut self, dependent: &str);
}

#[derive(Debug)]
pub enum StatType {
    Simple(Simple),
    Modifiable(Modifiable),
    Complex(ComplexModifiable),
    DependentOnly(DependentOnly),
}

impl StatType {
    pub fn new<V>(path: &str, value: V) -> Self
    where
        V: Into<ValueType>,
    {
        let segments: Vec<&str> = path.split("_").collect();
        let vt: ValueType = value.into();
        
        match segments.len() {
            1 => {
                // Simple stat
                match vt {
                    ValueType::Literal(v) => StatType::Simple(Simple::new(v)),
                    ValueType::Expression(_) => panic!(),
                }
            },
            2 => {
                // Modifiable stat
                let mut stat = Modifiable::new(segments[0]);
                stat.add_modifier(segments[1], vt);
                StatType::Modifiable(stat)
            },
            3 => {
                // Complex stat
                let mut stat = ComplexModifiable::new(segments[0]);
                stat.add_modifier(
                    segments[1],
                    segments[2].parse::<u32>().unwrap(),
                    vt
                );
                StatType::Complex(stat)
            },
            _ => panic!("Invalid stat path format: {}", path)
        }
    }

    pub fn evaluate(&self, path: &[&str], stat_definitions: &StatDefinitions) -> f32 {
        match self {
            StatType::Simple(simple) => simple.value,
            StatType::Modifiable(modifiable) => {
                if path.len() == 1 {
                    modifiable.evaluate(stat_definitions)
                } else if path.len() == 2 {
                    modifiable.evaluate_part(path[1], stat_definitions)
                } else {
                    0.0 // invalid query
                }
            },
            StatType::Complex(complex_modifiable) => {
                complex_modifiable.evaluate(path, stat_definitions)
            },
            StatType::DependentOnly(_) => 0.0,  // Always return
        }
    }

    pub fn add_modifier<V>(&mut self, path: &[&str], value: V)
    where
        V: Into<ValueType>,
    {
        match self {
            StatType::Simple(simple) => {
                let vt: ValueType = value.into();
                simple.value += match vt {
                    ValueType::Literal(value) => value,
                    ValueType::Expression(_expression) => 0.0, // Still not sure what to do when you try to apply an expr to a simple. Do you panic? Fail forward? Warning?
                }
            },
            StatType::Modifiable(modifiable) => {
                modifiable.add_modifier(path[1], value);
            },
            StatType::Complex(complex_modifiable) => {
                complex_modifiable.add_modifier(path[1], path[2].parse::<u32>().unwrap(), value);
            },
            StatType::DependentOnly(_) => {}
        }
    }

    pub fn remove_modifier<V>(&mut self, path: &[&str], value: V)
    where
        V: Into<ValueType>,
    {
        match self {
            StatType::Simple(simple) => {
                let vt: ValueType = value.into();
                simple.value -= match vt {
                    ValueType::Literal(value) => value,
                    ValueType::Expression(_expression) => 0.0, // Still not sure what to do when you try to apply an expr to a simple. Do you panic? Fail forward? Warning?
                }
            },
            StatType::Modifiable(modifiable) => {
                modifiable.remove_modifier(path[0], value);
            },
            StatType::Complex(complex_modifiable) => {
                complex_modifiable.remove_modifier(path[0], path[1].parse::<u32>().unwrap(), value);
            },
            StatType::DependentOnly(_) => {}
        }
    }
}

impl StatLike for StatType {
    fn add_dependent(&mut self, dependent: String) {
        match self {
            StatType::Simple(simple) => simple.add_dependent(dependent),
            StatType::Modifiable(modifiable) => modifiable.add_dependent(dependent),
            StatType::Complex(complex_modifiable) => complex_modifiable.add_dependent(dependent),
            StatType::DependentOnly(placeholder) => placeholder.add_dependent(dependent),
        }
    }

    fn remove_dependent(&mut self, dependent: &str) {
        match self {
            StatType::Simple(simple) => simple.remove_dependent(dependent),
            StatType::Modifiable(modifiable) => modifiable.remove_dependent(dependent),
            StatType::Complex(complex_modifiable) => complex_modifiable.remove_dependent(dependent),
            StatType::DependentOnly(placeholder) => placeholder.remove_dependent(dependent),
        }
    }
}

#[derive(Default, Debug)]
pub struct Simple {
    pub value: f32,
    pub dependents: HashMap<String, u32>,
}

impl Simple {
    pub fn new(value: f32) -> Self {
        Self { value, dependents: HashMap::new() }
    }

    pub fn get_value(&self) -> f32 {
        self.value
    } 

    pub fn add(&mut self, value: f32) {
        self.value += value;
    }

    pub fn remove(&mut self, value: f32) {
        self.value -= value;
    }
}

impl StatLike for Simple {
    fn add_dependent(&mut self, dependent: String) {
        *self.dependents.entry(dependent).or_insert(0) += 1;
    }

    fn remove_dependent(&mut self, dependent: &str) {
        if let Some(count) = self.dependents.get_mut(dependent) {
            *count -= 1;
            if *count == 0 {
                self.dependents.remove(dependent);
            }
        }
    }
}

#[derive(Debug)]
pub struct Modifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, StatModifierStep>,
    pub dependents: HashMap<String, u32>,
}

impl Modifiable {
    pub fn new(name: &str) -> Self {
        Modifiable { 
            total: Expression { 
                string: get_total_expr_from_name(name).to_string(), 
                value: evalexpr::build_operator_tree(get_total_expr_from_name(name)).unwrap() 
            }, 
            modifier_types: HashMap::new(), 
            dependents: HashMap::new(),
        }
    }
    
    pub fn add_modifier<V, S>(&mut self, segment: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType>,
    {
        let key = segment.into();
        let part = self
            .modifier_types
            .entry(key.clone())
            .or_insert(StatModifierStep::new(&key));

        part.add_modifier(value);
    }

    pub fn remove_modifier<V, S>(&mut self, segment: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType>,
    {
        let key = segment.into();
        let part = self
            .modifier_types
            .entry(key.clone())
            .or_insert(StatModifierStep::new(&key));

        part.remove_modifier(value);
    }

    pub fn evaluate(&self, stat_definitions: &StatDefinitions) -> f32 {
        // Evaluate each modifier part and inject them into the context
        let mut context = HashMapContext::new();
        for name in self.total.value.iter_variable_identifiers() {
            let val = get_initial_value_for_modifier(name);
            context.set_value(name.to_string(), Value::Float(val as f64)).unwrap();
        }

        for (name, part) in &self.modifier_types {
            let part_value = part.evaluate(stat_definitions);
            context.set_value(name.clone(), Value::Float(part_value as f64)).unwrap();
        }

        // Evaluate the total expression
        self
            .total
            .value
            .eval_with_context(&context)
            .unwrap()
            .as_number()
            .unwrap() as f32
    }

    pub fn evaluate_part(&self, part: &str, stat_definitions: &StatDefinitions) -> f32 {
        let Some(part) = self.modifier_types.get(part) else {
            return 0.0;
        };

        part.evaluate(stat_definitions)
    } 
}

impl StatLike for Modifiable  {
    fn add_dependent(&mut self, dependent: String) {
        *self.dependents.entry(dependent).or_insert(0) += 1;
    }

    fn remove_dependent(&mut self, dependent: &str) {
        if let Some(count) = self.dependents.get_mut(dependent) {
            *count -= 1;
            if *count == 0 {
                self.dependents.remove(dependent);
            }
        }
    }
}

#[derive(Debug)]
pub struct ComplexModifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, HashMap<u32, StatModifierStep>>,
    pub dependents: HashMap<String, u32>, // Added simple dependents map like other types
}

impl ComplexModifiable {
    pub fn new(name: &str) -> Self {
        Self {
            total: Expression { 
                string: get_total_expr_from_name(name).to_string(), 
                value: evalexpr::build_operator_tree(get_total_expr_from_name(name)).unwrap() 
            }, 
            modifier_types: HashMap::new(),
            dependents: HashMap::new(), // Initialize empty dependents
        }
    }

    pub fn evaluate(&self, path: &[&str], stat_definitions: &StatDefinitions) -> f32 {
        // Attempt to parse the query from the first segment.
        let search_bitflags = match path.get(1) {
            Some(query_str) => query_str.parse::<u32>().unwrap_or(0),
            None => return 0.0,
        };
    
        let mut context = HashMapContext::new();
        
        // Initialize all variables in the expression with their default values
        for name in self.total.value.iter_variable_identifiers() {
            let val = get_initial_value_for_modifier(name);
            context.set_value(name.to_string(), Value::Float(val as f64)).unwrap();
        }
    
        // For each category in the complex modifier, sum up all matching contributions.
        for (category, values) in &self.modifier_types {
            let category_sum: f32 = values
                .iter()
                .filter_map(|(&mod_bitflags, value)| {
                    // Only include modifiers that match ALL the requested flags
                    if (mod_bitflags & search_bitflags) == search_bitflags {
                        Some(value.evaluate(stat_definitions))
                    } else {
                        None
                    }
                })
                .sum();
    
            // Set the value in the context (ignoring errors).
            context.set_value(category.clone(), Value::Float(category_sum as f64)).ok();
        }
    
        // Evaluate the total expression with the built-up context.
        self
            .total
            .value
            .eval_with_context(&context)
            .ok()
            .and_then(|v| v.as_number().ok())
            .map(|num| num as f32)
            .unwrap_or(0.0)
    }

    pub fn add_modifier<V>(&mut self, path: &str, tag: u32, value: V)
    where 
        V: Into<ValueType>,
    {
        if let Some(map) = self.modifier_types.get_mut(path) {
            if let Some(stat_modifier_step) = map.get_mut(&tag) {
                stat_modifier_step.add_modifier(value);
            } else {
                let mut new_stat_mod_step = StatModifierStep::new(path);
                new_stat_mod_step.add_modifier(value);

                map.insert(tag, new_stat_mod_step);
            }
        } else {
            let mut map = HashMap::new();
            let mut new_stat_mod_step = StatModifierStep::new(path);
            new_stat_mod_step.add_modifier(value);

            map.insert(tag, new_stat_mod_step);
            self.modifier_types.insert(path.to_string(), map);
        }
    }

    pub fn remove_modifier<V>(&mut self, path: &str, tag: u32, value: V) 
    where
        V: Into<ValueType>,
    {
        // Get the modifier map for this path
        if let Some(map) = self.modifier_types.get_mut(path) {
            // Get the specific tag entry
            if let Some(stat_modifier_step) = map.get_mut(&tag) {
                // Remove the modifier from the step
                stat_modifier_step.remove_modifier(value);
                
                // If the step has no more modifiers and no dependents, clean it up
                if stat_modifier_step.base == 0.0 && stat_modifier_step.mods.is_empty() {
                    map.remove(&tag);
                }
            }
            
            // If the map is now empty, clean it up
            if map.is_empty() {
                self.modifier_types.remove(path);
            }
        }
        // If the path doesn't exist, do nothing (consistent with add_modifier behavior)
    }
}

impl StatLike for ComplexModifiable {
    fn add_dependent(&mut self, dependent: String) {
        *self.dependents.entry(dependent).or_insert(0) += 1;
    }

    fn remove_dependent(&mut self, dependent: &str) {
        if let Some(count) = self.dependents.get_mut(dependent) {
            *count -= 1;
            if *count == 0 {
                self.dependents.remove(dependent);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct DependentOnly {
    pub dependents: HashMap<String, u32>,
}

impl StatLike for DependentOnly {
    fn add_dependent(&mut self, dependent: String) {
        *self.dependents.entry(dependent).or_insert(0) += 1;
    }

    fn remove_dependent(&mut self, dependent: &str) {
        if let Some(count) = self.dependents.get_mut(dependent) {
            *count -= 1;
            if *count == 0 {
                self.dependents.remove(dependent);
            }
        }
    }
}

#[derive(Debug)]
pub struct StatModifierStep {
    pub relationship: ModType,
    pub base: f32,
    pub mods: Vec<Expression>,
}

impl StatModifierStep {
    pub fn new(name: &str) -> Self {
        let base = 0.0; // get_initial_value_for_modifier(name);
        Self { relationship: ModType::Add, base, mods: Vec::new() }
    }

    pub fn evaluate(&self, stat_definitions: &StatDefinitions) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stat_definitions)).collect();

        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => 1.0 + (self.base + computed.iter().sum::<f32>()),
        }
    }

    pub fn add_modifier<V>(&mut self, modifier: V) 
    where
        V: Into<ValueType>,
    {
        let vt: ValueType = modifier.into();
        match vt {
            ValueType::Literal(vals) => {
                self.base += vals;
            }
            ValueType::Expression(expression) => {
                self.mods.push(expression.clone());
            }
        }
    }

    pub fn remove_modifier<V>(&mut self, modifier: V) 
    where
        V: Into<ValueType>,
    {
        let vt: ValueType = modifier.into();
        match vt {
            ValueType::Literal(vals) => {
                self.base -= vals;
            }
            ValueType::Expression(expression) => {
                if let Some(pos) = self
                    .mods
                    .iter()
                    .position(|e| *e == expression)
                {
                    self.mods.remove(pos);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Expression {
    pub string: String,
    pub value: Node<DefaultNumericTypes>,
}

impl Expression {
    pub fn evaluate(&self, stat_definitions: &StatDefinitions) -> f32 {
        let mut context = HashMapContext::new();
        for var_name in self.value.iter_variable_identifiers() {
            let val = stat_definitions.evaluate(var_name);
            context.set_value(var_name.to_string(), Value::Float(val as f64)).unwrap();
        }
        self.value.eval_with_context(&context).unwrap().as_number().unwrap() as f32
    }
}

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        self.string == other.string
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

// TODO Consider parsing numeric literal strings (i.e., "0.0") as literal value types.
impl From<&str> for ValueType {
    fn from(value: &str) -> Self {
        Self::Expression(Expression {
            string: value.to_string(),
            value: evalexpr::build_operator_tree(value).unwrap(),
        })
    }
}

impl From<String> for ValueType {
    fn from(value: String) -> Self {
        Self::Expression(Expression {
            string: value.clone(),
            value: evalexpr::build_operator_tree(&value).unwrap(),
        })
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

pub fn get_total_expr_from_name(name: &str) -> &'static str {
    match name {
        "Damage" => "Added * Increased * More",
        "Life" => "Added * Increased * More",
        _ => "",
    }
}

fn get_initial_value_for_modifier(modifier_type: &str) -> f32 {
    match modifier_type {
        "Added" | "Base" | "Flat" => 0.0,
        "Increased" | "More" | "Multiplier" => 1.0,
        "Override" => 1.0, // Special case
        _ => 0.0, // Default case
    }
}

stat_macros::define_tags! {
    damage {
        damage_type {
            elemental { fire, cold, lightning },
            physical,
            chaos,
        },
        weapon_type {
            melee { sword, axe },
            ranged { bow, wand },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::utils::HashMap;

    fn assert_approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < f32::EPSILON * 100.0, "left: {}, right: {}", a, b);
    }

    // Helper function to create a fresh StatDefinitions with common stats
    fn test_stats() -> StatDefinitions {
        let mut stats = StatDefinitions(HashMap::new());
        
        // Simple stat
        stats.add_modifier("Movespeed", 10.0);
        
        // Modifiable stats
        stats.add_modifier("Life_Added", 20.0);
        stats.add_modifier("Life_Increased", 1.1); // 10% increase
        
        // Complex stats with damage types - using Damage enum variants
        stats.add_modifier(&format!("Damage_Added_{}", (u32::MAX &!Damage::DAMAGE_TYPE) | Damage::FIRE), 5.0);
        stats.add_modifier(&format!("Damage_Added_{}", (u32::MAX &!Damage::DAMAGE_TYPE) | Damage::CHAOS), 8.0);
        stats.add_modifier(&format!("Damage_Added_{}", (u32::MAX &!Damage::WEAPON_TYPE) | Damage::SWORD), 3.0);
        stats.add_modifier(&format!("Damage_Increased_{}", (u32::MAX &!Damage::DAMAGE_TYPE) | Damage::FIRE), 1.2);
        stats.add_modifier(&format!("Damage_Increased_{}", (u32::MAX &!Damage::WEAPON_TYPE) | Damage::SWORD), 1.15);
        
        stats
    }

    #[test]
    fn test_simple_stat() {
        let stats = test_stats();
        
        // Basic evaluation
        assert_eq!(stats.evaluate("Movespeed"), 10.0);
        
        // Modification
        let mut stats = test_stats();
        stats.add_modifier("Movespeed", 5.0);
        assert_eq!(stats.evaluate("Movespeed"), 15.0);
        
        stats.remove_modifier("Movespeed", 3.0);
        assert_eq!(stats.evaluate("Movespeed"), 12.0);
    }

    #[test]
    fn test_modifiable_stat() {
        let stats = test_stats();
        
        // Full evaluation (Added * Increased)
        assert_approx_eq(stats.evaluate("Life"), 20.0 * 1.1);
        
        // Part evaluation
        assert_approx_eq(stats.evaluate("Life_Added"), 20.0);
        assert_approx_eq(stats.evaluate("Life_Increased"), 1.1);
        
        // Modification
        let mut stats = test_stats();

        stats.add_modifier("Life_Added", 10.0);
        stats.add_modifier("Life_More", 1.1);
        assert_approx_eq(stats.evaluate("Life"), 30.0 * 1.1 * 1.1);
        
        stats.remove_modifier("Life_Increased", 0.05);
        assert_approx_eq(stats.evaluate("Life_Increased"), 1.05);
    }

    #[test]
    fn test_complex_stat() {
        let stats = test_stats();
        
        // Test combined damage types (bitwise OR)
        assert_approx_eq(
            stats.evaluate(&format!("Damage_{}", Damage::FIRE | Damage::SWORD)), 
            (5.0 + 3.0) * (1.2 + 1.15)
        );
    }
    
    #[test]
    fn test_expression_stats() {
        let mut stats = test_stats();
        
        // Add expression-based modifier to a modifiable stat
        stats.add_modifier("Life_More", "Life_Added / 2.0"); // 5% more life per point of Life_Added
        
        // Should be: (20.0) * 1.1 * (1 + (20.0 * 0.05))
        assert_approx_eq(stats.evaluate("Life"), 20.0 * 1.1 * (20.0 / 2.0));
    }
    
    #[test]
    fn test_dependent_stats() {
        let mut stats = test_stats();
        
        // Add dependent expression
        stats.add_modifier("Life_More", "Movespeed * 0.1"); // 10% more per movespeed
        
        // Verify dependency was registered
        if let StatType::Simple(movespeed) = stats.0.get_mut("Movespeed").unwrap() {
            assert!(movespeed.dependents.contains_key("Life"));
        } else {
            panic!("Movespeed stat not found");
        }
        
        // Verify change propagates
        stats.add_modifier("Movespeed", 10.0);
        assert_eq!(stats.evaluate("Life"), 20.0 * 1.1 * 2.0);
    }

    #[test]
    fn test_stat_removal() {
        let mut stats = test_stats();
        
        // Remove a simple modifier
        stats.remove_modifier("Movespeed", 5.0);
        assert_approx_eq(stats.evaluate("Movespeed"), 5.0);
        
        // Remove a modifiable part
        stats.remove_modifier("Life_Added", 10.0);
        assert_approx_eq(stats.evaluate("Life_Added"), 10.0);
        
        // Remove a complex modifier using enum variant
        stats.remove_modifier(&format!("Damage_Added_{}", (u32::MAX &!Damage::DAMAGE_TYPE) | Damage::FIRE), 3.0);
        assert_approx_eq(
            stats.evaluate(&format!("Damage_{}", Damage::FIRE | Damage::SWORD)), 
            (5.0 - 3.0 + 3.0) * (1.2 + 1.15)
        );
    }

    #[test]
    fn test_empty_stats() {
        let stats = StatDefinitions(HashMap::new());
        
        // Evaluate non-existent stats
        assert_eq!(stats.evaluate("Nonexistent"), 0.0);
        assert_eq!(stats.evaluate("Damage_1"), 0.0);
        assert_eq!(stats.evaluate("Life_Added"), 0.0);
    }

    #[test]
    fn test_stat_type_creation() {
        // Test simple stat creation
        let simple = StatType::new("Test", 10.0);
        assert!(matches!(simple, StatType::Simple(_)));
        
        // Test modifiable stat creation
        let modifiable = StatType::new("Test_Added", 5.0);
        assert!(matches!(modifiable, StatType::Modifiable(_)));
        
        // Test complex stat creation
        let complex = StatType::new("Test_Added_1", 3.0);
        assert!(matches!(complex, StatType::Complex(_)));
    }

    #[test]
    #[should_panic]
    fn test_invalid_expression_on_simple() {
        // Should panic when trying to create simple stat with expression
        StatType::new("Test", "Other * 2");
    }
}