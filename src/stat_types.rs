use bevy::utils::HashMap;
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value};
use super::prelude::*;

#[derive(Debug, Clone, Default)]
pub(crate) enum ModType {
    #[default]
    Add,
    Mul,
}

#[derive(Debug, Clone)]
pub(crate) enum StatType {
    Flat(Flat),
    Modifiable(Modifiable),
    Complex(Complex),
    Tagged(Tagged),
}

impl Stat for StatType {
    fn new(path: &StatPath, config: &StatConfig) -> Self {
        match path.segments.len() {
            1 => {
                let mut stat = Modifiable::new(&path.segments[0]);
                stat.add_modifier(path, value.into());
                StatType::Modifiable(stat)
            },
            2 => {
                let mut stat = Complex::new(path.segments[0]);
                stat.add_modifier(&path, value.into());
                StatType::Complex(stat)
            },
            3 => {
                let mut stat = Tagged::new(&path.segments[0]);
                stat.add_modifier(&path, value.into());
                StatType::Tagged(stat)
            },
            _ => panic!("Invalid stat path format: {:#?}", path)
        }
    }

    fn add_modifier(&mut self, path: &StatPath, value: ValueType, config: &StatConfig) {
        match self {
            StatType::Flat(flat) => todo!(),
            StatType::Modifiable(simple) => simple.add_modifier(path, value),
            StatType::Complex(modifiable) => modifiable.add_modifier(path, value),
            StatType::Tagged(complex_modifiable) => complex_modifiable.add_modifier(path, value),
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ValueType, config: &StatConfig) {
        match self {
            StatType::Flat(flat) => todo!(),
            StatType::Modifiable(simple) => simple.remove_modifier(path, value),
            StatType::Complex(modifiable) => modifiable.remove_modifier(path, value),
            StatType::Tagged(complex_modifiable) => complex_modifiable.remove_modifier(path, value),
        }
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        match self {
            StatType::Flat(flat) => todo!(),
            StatType::Modifiable(simple) => simple.evaluate(path, stats),
            StatType::Complex(modifiable) => modifiable.evaluate(path, stats),
            StatType::Tagged(complex_modifiable) => complex_modifiable.evaluate(path, stats),
        }
    }
    
    fn set(&mut self, path: &StatPath, value: f32, config: &StatConfig) {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Flat(f32);

impl Stat for Flat {
    fn new(path: &StatPath, config: &StatConfig) -> Self {
        todo!()
    }

    fn add_modifier(&mut self, path: &StatPath, value: ValueType, config: &StatConfig) {
        todo!()
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ValueType, config: &StatConfig) {
        todo!()
    }

    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        todo!()
    }

    fn set(&mut self, _path: &StatPath, _value: f32, _config: &StatConfig) {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Modifiable {
    pub(crate) relationship: ModType,
    pub(crate) base: f32,
    pub(crate) mods: Vec<Expression>,
}

impl Modifiable {
}

impl Stat for Modifiable {
    fn new(path: &StatPath, _config: &StatConfig) -> Self {
        todo!()
        if path.path.to_lowercase().contains("more") {
            Self { relationship: ModType::Mul, base: 0.0, mods: Vec::new() }
        } else {
            Self { relationship: ModType::Add, base: 0.0, mods: Vec::new() }
        }
    }

    fn add_modifier(&mut self, _path: &StatPath, value: ValueType, _config: &StatConfig) {
        match value {
            ValueType::Literal(vals) => { self.base += vals; }
            ValueType::Expression(expression) => { self.mods.push(expression.clone()); }
        }
    }

    fn remove_modifier(&mut self, _path: &StatPath, value: &ValueType, _config: &StatConfig) {
        match value {
            ValueType::Literal(vals) => { self.base -= vals; }
            ValueType::Expression(expression) => {
                let Some(pos) = self.mods.iter().position(|e| e == expression) else { return; };
                self.mods.remove(pos);
            }
        }
    }

    fn evaluate(&self, _path: &StatPath, stats: &Stats) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stats.get_context())).collect();
        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => self.base * computed.iter().product::<f32>(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Complex {
    pub(crate) total: Expression,
    pub(crate) modifier_steps: HashMap<String, Modifiable>,
}

impl Stat for Complex  {
    fn new(path: &StatPath, config: &StatConfig) -> Self {
        let original_expr = get_total_expr_from_name(name);
        let mut modifier_steps = HashMap::new();
        let modifier_names: Vec<&str> = original_expr.split(|c: char| !c.is_alphabetic())
            .filter(|s| !s.is_empty())
            .collect();
        for modifier_name in modifier_names.iter() {
            let step = Modifiable::new(modifier_name);
            modifier_steps.insert(modifier_name.to_string(), step);
        }
        let transformed_expr = original_expr.split(|c: char| !c.is_alphabetic())
            .fold(original_expr.to_string(), |expr, word| {
                if modifier_names.contains(&word) {
                    expr.replace(word, &format!("{}.{}", name, word))
                } else {
                    expr
                }
            });
        Complex { 
            total: Expression { 
                definition: transformed_expr.clone(),
                compiled: evalexpr::build_operator_tree(&transformed_expr).unwrap(),
            },
            modifier_steps,
        }
    }

    fn add_modifier(&mut self, path: &StatPath, value: ValueType, config: &StatConfig) {
        if path.len() != 2 { return; }
        let key = path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Modifiable::new(StatPath::parse(&key), config));
        part.add_modifier(path, value);
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ValueType, _config: &StatConfig) {
        if path.len() != 2 { return; }
        let key = path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Modifiable::new(&key));
        part.remove_modifier(path, value);
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        match path.len() {
            1 => {
                let total = self.total.compiled
                    .eval_with_context(stats.get_context())
                    .unwrap()
                    .as_number()
                    .unwrap() as f32;
                stats.set_cached(&path.path, total);
                total
            }
            2 => {
                let Some(part) = self.modifier_steps.get(&path.segments[1]) else { return 0.0; };
                let part_total = part.evaluate(path, stats);
                stats.set_cached(&path.path, part_total);
                part_total
            }
            _ => 0.0
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TaggedEntry(pub HashMap<u32, Modifiable>);

#[derive(Debug, Clone)]
pub(crate) struct Tagged {
    pub(crate) total: Expression,
    pub(crate) modifier_types: HashMap<String, TaggedEntry>,
}

impl Stat for Tagged {
    fn new(path: &StatPath, config: &StatConfig) -> Self {
        // TODO get default expression from the config based on the stat path
        Self {
            total: Expression::new("").unwrap(), 
            modifier_types: HashMap::new(),
        }
    }

    fn add_modifier(&mut self, path: &StatPath, value: ValueType, config: &StatConfig) {
        if path.len() != 3 { return; }
        let modifier_type = &path.segments[1];
        let Ok(tag) = path.segments[2].parse::<u32>() else { return; };
        let step_map = self.modifier_types.entry(modifier_type.to_string())
            .or_insert(TaggedEntry(HashMap::new()));
        let step = step_map.0.entry(tag).or_insert(Modifiable::new(&StatPath::parse(modifier_type), config));
        step.add_modifier(path, value, config);
    }

    fn remove_modifier(&mut self, path: &StatPath, value: &ValueType, config: &StatConfig) {
        if path.len() != 3 { return; }
        let Some(step_map) = self.modifier_types.get_mut(&path.segments[1]) else { return; };
        let Ok(tag) = path.segments[2].parse::<u32>() else { return; };
        let Some(step) = step_map.0.get_mut(&tag) else { return; };
        step.remove_modifier(path, value, config);
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        let full_path = &path.path;

        if let Ok(search_tags) = path.segments.get(1).unwrap().parse::<u32>() {
            let mut context = HashMapContext::new();
            for name in self.total.compiled.iter_variable_identifiers() {
                let val = get_initial_value_for_modifier(name);
                context.set_value(name.to_string(), Value::Float(val as f64)).unwrap();
            }
            for (category, values) in &self.modifier_types {
                let category_sum: f32 = values.0
                    .iter()
                    .filter_map(|(&mod_tags, value)| {
                        if mod_tags.has_all(search_tags) {
                            let dep_path = format!("{}.{}.{}", path.segments[0], category, mod_tags.to_string());
                            stats.add_dependent(&dep_path, DependentType::LocalStat(full_path.to_string()));
                            Some(value.evaluate(path, stats))
                        } else {
                            None
                        }
                    })
                    .sum();
                context.set_value(category.clone(), Value::Float(category_sum as f64)).ok();
            }
            let total = self.total.compiled
                .eval_with_context(&context)
                .unwrap()
                .as_number()
                .unwrap() as f32;
            stats.set_cached(&full_path, total);
            return total;
        }

        if let Ok(search_tags) = path.segments.get(2).unwrap().parse::<u32>() {
            let category = &path.segments[1];
            let Some(values) = self.modifier_types.get(category) else {
                return 0.0;
            };

            return values.0
                .iter()
                .filter_map(|(&mod_tags, value)| {
                    if mod_tags.has_all(search_tags) {
                        let dep_path = format!("{}.{}.{}", path.segments[0], category, mod_tags.to_string());
                        stats.add_dependent(&dep_path, DependentType::LocalStat(full_path.to_string()));
                        Some(value.evaluate(path, stats))
                    } else {
                        None
                    }
                })
                .sum();
        }

        return 0.0;
    }
}