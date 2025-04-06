use bevy::{ecs::system::SystemParam, prelude::*, utils::HashMap};
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value};
use super::prelude::*;

#[derive(Debug, Clone, Default)]
pub enum ModType {
    #[default]
    Add,
    Mul,
}

#[derive(Debug)]
pub enum StatType {
    Simple(Simple),
    Modifiable(Modifiable),
    Complex(ComplexModifiable),
}

impl StatType {
    pub fn new<V>(stat_path: &str, value: V) -> Self
    where
        V: Into<ValueType> + Clone,
    {
        let stat_path = StatPath::parse(stat_path);
        match stat_path.segments.len() {
            1 => {
                let mut stat = Simple::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value);
                StatType::Simple(stat)
            },
            2 => {
                let mut stat = Modifiable::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value);
                StatType::Modifiable(stat)
            },
            3 => {
                let mut stat = ComplexModifiable::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value);
                StatType::Complex(stat)
            },
            _ => panic!("Invalid stat path format: {:#?}", stat_path)
        }
    }
}

impl StatLike for StatType {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        match self {
            StatType::Simple(simple) => simple.add_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.add_modifier(stat_path, value),
        }
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        match self {
            StatType::Simple(simple) => simple.remove_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.remove_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.remove_modifier(stat_path, value),
        }
    }
    
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32 {
        match self {
            StatType::Simple(simple) => simple.evaluate(stat_path, stats),
            StatType::Modifiable(modifiable) => modifiable.evaluate(stat_path, stats),
            StatType::Complex(complex_modifiable) => complex_modifiable.evaluate(stat_path, stats),
        }
    }
    
    fn on_insert(&self, stats: &Stats, stat_path: &StatPath) {
        match self {
            StatType::Simple(simple) => simple.on_insert(stats, stat_path),
            StatType::Modifiable(modifiable) => modifiable.on_insert(stats, stat_path),
            StatType::Complex(complex_modifiable) => complex_modifiable.on_insert(stats, stat_path),
        }
    }
}

#[derive(Debug)]
pub struct Simple {
    pub relationship: ModType,
    pub base: f32,
    pub mods: Vec<Expression>,
}

impl Simple {
    pub fn new(name: &str) -> Self {
        //let base = get_initial_value_for_modifier(name);
        Self { relationship: ModType::Add, base: 0.0, mods: Vec::new() }
    }
}

impl StatLike for Simple {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, _stat_path: &StatPath, value: V) {
        let vt: ValueType = value.into();

        match vt {
            ValueType::Literal(vals) => { self.base += vals; }
            ValueType::Expression(expression) => { self.mods.push(expression.clone()); }
        }
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, _stat_path: &StatPath, value: V) {
        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(vals) => { self.base -= vals; }
            ValueType::Expression(expression) => {
                let Some(pos) = self.mods.iter().position(|e| *e == expression) else { return; };
                self.mods.remove(pos);
            }
        }
    }

    fn evaluate(&self, _stat_path: &StatPath, stats: &Stats) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stats.cached_context())).collect();
        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => self.base * computed.iter().product::<f32>(),
        }
    }

    fn on_insert(&self, _stats: &Stats, _stat_path: &StatPath) { }
}

#[derive(Debug)]
pub struct Modifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_steps: HashMap<String, Simple>,
}

impl Modifiable {
    pub fn new(name: &str) -> Self {
        let original_expr = get_total_expr_from_name(name);
        let mut modifier_steps = HashMap::new();
        let modifier_names: Vec<&str> = original_expr.split(|c: char| !c.is_alphabetic())
            .filter(|s| !s.is_empty())
            .collect();
        for modifier_name in modifier_names.iter() {
            let step = Simple::new(modifier_name);
            modifier_steps.insert(modifier_name.to_string(), step);
        }
        let transformed_expr = original_expr.split(|c: char| !c.is_alphabetic())
            .fold(original_expr.to_string(), |expr, word| {
                if modifier_names.contains(&word) {
                    expr.replace(word, &format!("{}_{}", name, word))
                } else {
                    expr
                }
            });
        Modifiable { 
            total: Expression { 
                string: transformed_expr.clone(),
                value: evalexpr::build_operator_tree(&transformed_expr).unwrap(),
            },
            modifier_steps,
        }
    }
}

impl StatLike for Modifiable  {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 2 { return; }
        let key = stat_path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.add_modifier(stat_path, value);
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 2 { return; }
        let key = stat_path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.remove_modifier(stat_path, value);
    }
    
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32 {
        match stat_path.len() {
            1 => {
                self.total.value
                    .eval_with_context(stats.cached_context())
                    .unwrap()
                    .as_number()
                    .unwrap() as f32
            }
            2 => {
                let Some(part) = self.modifier_steps.get(&stat_path.segments[1]) else { return 0.0; };
                part.evaluate(stat_path, stats)
            }
            _ => 0.0
        }
    }
    
    fn on_insert(&self, stats: &Stats, stat_path: &StatPath) {
        if stat_path.segments.is_empty() { return; }
        let base_name = &stat_path.segments[0];
        for (modifier_name, _) in self.modifier_steps.iter() {
            let full_modifier_path = format!("{}_{}", base_name, modifier_name);
            if stats.get_cached(&full_modifier_path).is_err() {
                let val = self.modifier_steps.get(modifier_name).unwrap().evaluate(stat_path, stats);
                stats.set_cached(&full_modifier_path, val);
            }
        }
    }
}

#[derive(Debug)]
pub struct ComplexEntry(f32, HashMap<u32, Simple>);

#[derive(Debug)]
pub struct ComplexModifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, ComplexEntry>,
}

impl ComplexModifiable {
    pub fn new(name: &str) -> Self {
        Self {
            total: Expression { 
                string: get_total_expr_from_name(name).to_string(), 
                value: evalexpr::build_operator_tree(get_total_expr_from_name(name)).unwrap() 
            }, 
            modifier_types: HashMap::new(),
        }
    }
}

impl StatLike for ComplexModifiable {
    fn add_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 3 { return; }
        let modifier_type = &stat_path.segments[1];
        let Ok(tag) = stat_path.segments[2].parse::<u32>() else { return; };
        let step_map = self.modifier_types.entry(modifier_type.to_string())
            .or_insert(ComplexEntry(get_initial_value_for_modifier(modifier_type), HashMap::new()));
        let step = step_map.1.entry(tag).or_insert(Simple::new(modifier_type));
        step.add_modifier(stat_path, value);
    }

    fn remove_modifier<V: Into<ValueType> + Clone>(&mut self, stat_path: &StatPath, value: V) {
        if stat_path.len() != 3 { return; }
        let Some(step_map) = self.modifier_types.get_mut(&stat_path.segments[1]) else { return; };
        let Ok(tag) = stat_path.segments[2].parse::<u32>() else { return; };
        let Some(step) = step_map.1.get_mut(&tag) else { return; };
        step.remove_modifier(stat_path, value);
    }
    
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32 {
        let full_path = &stat_path.path;

        if let Ok(search_tags) = stat_path.segments.get(1).unwrap().parse::<u32>() {
            let mut context = HashMapContext::new();
            for name in self.total.value.iter_variable_identifiers() {
                let val = get_initial_value_for_modifier(name);
                context.set_value(name.to_string(), Value::Float(val as f64)).unwrap();
            }
            for (category, values) in &self.modifier_types {
                let category_sum: f32 = values.1
                    .iter()
                    .filter_map(|(&mod_tags, value)| {
                        if mod_tags.has_all(search_tags) {
                            let dep_path = format!("{}_{}_{}", stat_path.segments[0], category, mod_tags.to_string());
                            stats.add_dependent_internal(&dep_path, DependentType::LocalStat(full_path.to_string()));
                            Some(value.evaluate(stat_path, stats))
                        } else {
                            None
                        }
                    })
                    .sum();
                context.set_value(category.clone(), Value::Float((category_sum + values.0) as f64)).ok();
            }
            let total = self.total.value
                .eval_with_context(&context)
                .unwrap()
                .as_number()
                .unwrap() as f32;
            stats.set_cached(&full_path, total);
            return total;
        }

        if let Ok(search_tags) = stat_path.segments.get(2).unwrap().parse::<u32>() {
            let category = &stat_path.segments[1];
            let Some(values) = self.modifier_types.get(category) else {
                return 0.0;
            };

            return values.1
                .iter()
                .filter_map(|(&mod_tags, value)| {
                    if mod_tags.has_all(search_tags) {
                        let dep_path = format!("{}_{}_{}", stat_path.segments[0], category, mod_tags.to_string());
                        stats.add_dependent_internal(&dep_path, DependentType::LocalStat(full_path.to_string()));
                        Some(value.evaluate(stat_path, stats))
                    } else {
                        None
                    }
                })
                .sum();
        }

        return 0.0;
    }
    
    fn on_insert(&self, _stats: &Stats, _stat_path: &StatPath) { }
}