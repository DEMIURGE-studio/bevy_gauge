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
    // StatType needs to use the path and config to derive the subtype of the stat
    fn new(path: &StatPath, config: &Config) -> Self {
        let stat_type = config.get_stat_type(path);
        match stat_type {
            "Flat" => { StatType::Flat(Flat::new(path, config)) },
            "Modifiable" => { StatType::Modifiable(Modifiable::new(path, config)) },
            "Complex" => { StatType::Complex(Complex::new(path, config)) },
            "Tagged" => { StatType::Tagged(Tagged::new(path, config)) },
            _ => { panic!("Invalid stat type!"); },
        }
    }

    fn initialize(&self, path: &StatPath, stats: &mut Stats) {
        match self {
            StatType::Flat(flat) => flat.initialize(path, stats),
            StatType::Modifiable(modifiable) => modifiable.initialize(path, stats),
            StatType::Complex(complex) => complex.initialize(path, stats),
            StatType::Tagged(tagged) => tagged.initialize(path, stats),
        }
    }

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType, config: &Config) {
        match self {
            StatType::Flat(flat) => flat.add_modifier(path, modifier, config),
            StatType::Modifiable(simple) => simple.add_modifier(path, modifier, config),
            StatType::Complex(modifiable) => modifiable.add_modifier(path, modifier, config),
            StatType::Tagged(complex_modifiable) => complex_modifiable.add_modifier(path, modifier, config),
        }
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) {
        match self {
            StatType::Flat(flat) => flat.remove_modifier(path, modifier),
            StatType::Modifiable(simple) => simple.remove_modifier(path, modifier),
            StatType::Complex(modifiable) => modifiable.remove_modifier(path, modifier),
            StatType::Tagged(complex_modifiable) => complex_modifiable.remove_modifier(path, modifier),
        }
    }
    
    fn set(&mut self, path: &StatPath, value: f32) {
        match self {
            StatType::Flat(flat) => flat.set(path, value),
            StatType::Modifiable(simple) => simple.set(path, value),
            StatType::Complex(modifiable) => modifiable.set(path, value),
            StatType::Tagged(complex_modifiable) => complex_modifiable.set(path, value),
        }
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        match self {
            StatType::Flat(flat) => flat.0,
            StatType::Modifiable(simple) => simple.evaluate(path, stats),
            StatType::Complex(modifiable) => modifiable.evaluate(path, stats),
            StatType::Tagged(complex_modifiable) => complex_modifiable.evaluate(path, stats),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Flat(f32);

impl Stat for Flat {
    fn new(_path: &StatPath, _config: &Config) -> Self { Self(0.0) }

    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType, _config: &Config) {
        if let ModifierType::Literal(value) = modifier {
            self.0 += value;
        }
    }

    fn remove_modifier(&mut self, _path: &StatPath, modifier: &ModifierType) {
        if let ModifierType::Literal(value) = modifier {
            self.0 -= value;
        }
    }

    fn set(&mut self, _path: &StatPath, value: f32) { self.0 = value; }

    fn evaluate(&self, _path: &StatPath, _stats: &Stats) -> f32 { self.0 }
}

#[derive(Debug, Clone)]
pub(crate) struct Modifiable {
    pub(crate) relationship: ModType,
    pub(crate) base: f32,
    pub(crate) mods: Vec<Expression>,
}

impl Stat for Modifiable {
    // Should derive the relationship from the path and the config.
    // "Added" or "Increased" stats should have a ModType::Add
    // "More" should have ModType::Mul
    fn new(path: &StatPath, config: &Config) -> Self {
        let relationship = config.get_relationship_type(path);
        Self { relationship, base: 0.0, mods: Vec::new() }
    }

    // Should probably change add and remove for literals to take the relationship into account.
    // For instance, ModType::Mul should have a *= for adding and reverse that for removing
    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType, _config: &Config) {
        match modifier {
            ModifierType::Literal(vals) => { 
                match self.relationship {
                    ModType::Add => {
                        self.base += vals;
                    },
                    ModType::Mul => {
                        self.base *= vals;
                    },
                }
            }
            ModifierType::Expression(expression) => { self.mods.push(expression.clone()); }
        }
    }

    fn remove_modifier(&mut self, _path: &StatPath, modifier: &ModifierType) {
        match modifier {
            ModifierType::Literal(vals) => { 
                match self.relationship {
                    ModType::Add => {
                        self.base -= vals;
                    },
                    ModType::Mul => {
                        self.base /= vals;
                    },
                }
            }
            ModifierType::Expression(expression) => {
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
    fn new(path: &StatPath, config: &Config) -> Self {
        let total_expression = config.get_total_expression(path);
        let compiled_expression = Expression::new(total_expression).unwrap();

        let mut modifier_steps = HashMap::new();
        for part in compiled_expression.compiled.iter_identifiers() {
            let part_path = &StatPath::parse(part);
            let step = Modifiable::new(part_path, config);
            modifier_steps.insert(part.to_string(), step);
        }

        Self {
            total: compiled_expression,
            modifier_steps,
        }
    }

    fn initialize(&self, path: &StatPath, stats: &mut Stats) {
        for part in self.total.compiled.iter_identifiers() {
            let part_path = format!("{}.{}", path.name, part);
            stats.add_dependent(path.name, DependentType::LocalStat(part_path));
        }
    }

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType, config: &Config) {
        let Some(part_key) = path.part else {
            return;
        };
        let part = self.modifier_steps.get_mut(part_key).unwrap();
        part.add_modifier(path, modifier, config);
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) {
        let Some(part_key) = path.part else {
            return;
        };
        let part = self.modifier_steps.get_mut(part_key).unwrap();
        part.remove_modifier(path, modifier);
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        if let Some(part_key) = path.part {
            let Some(part) = self.modifier_steps.get(part_key) else { return 0.0; };
            let part_total = part.evaluate(path, stats);
            stats.set_cached(path.full_path, part_total);
            return part_total;
        } else {
            let total = self.total.compiled
                .eval_with_context(stats.get_context())
                .unwrap()
                .as_number()
                .unwrap() as f32;
            stats.set_cached(&path.full_path, total);
            return total;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TaggedEntry(pub HashMap<u32, Modifiable>);

impl TaggedEntry {
    fn new() -> Self {
        TaggedEntry(HashMap::new())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Tagged {
    pub(crate) total: Expression,
    pub(crate) modifier_steps: HashMap<String, TaggedEntry>,
}

impl Tagged {
    fn evaluate_part(&self, part: &str, tag: u32, stats: &Stats) -> f32 {
        let Some(modifiers) = self.modifier_steps.get(part) else {
            return 0.0;
        };

        let total: f32 = modifiers.0.iter()
            .filter_map(|(&mod_tags, value)| {
                if mod_tags.has_all(tag) {
                    Some(value.evaluate(&StatPath::parse(part), stats))
                } else {
                    None
                }
            })
            .sum();

        total
    }
}

impl Stat for Tagged {
    fn new(path: &StatPath, config: &Config) -> Self {
        let total_expression = config.get_total_expression(path);
        let compiled_expression = Expression::new(total_expression).unwrap();

        let mut modifier_steps = HashMap::new();
        for part in compiled_expression.compiled.iter_identifiers() {
            let step = TaggedEntry::new();
            modifier_steps.insert(part.to_string(), step);
        }

        Self {
            total: compiled_expression,
            modifier_steps,
        }
    }

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType, config: &Config) {
        let Some(tag) = path.tag else {
            return;
        };

        let Some(part) = path.part else {
            return;
        };

        let step_map = self.modifier_steps.entry(part.to_string())
            .or_insert(TaggedEntry(HashMap::new()));

        let step = step_map.0.entry(tag).or_insert(Modifiable::new(path, config));

        step.add_modifier(path, modifier, config);
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) {
        let Some(tag) = path.tag else {
            return;
        };

        let Some(part) = path.part else {
            return;
        };

        let Some(step_map) = self.modifier_steps.get_mut(part) else { return; };
        
        let Some(step) = step_map.0.get_mut(&tag) else { return; };

        step.remove_modifier(path, modifier);
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        let Some(tag) = path.tag else {
            return 0.0;
        };

        if let Some(part) = path.part {
            return self.evaluate_part(part, tag, stats);
        } else {
            let mut context = HashMapContext::new();
            for part_id in self.total.compiled.iter_identifiers() {
                let value = Value::Float(self.evaluate_part(part_id, tag, stats) as f64);
                context.set_value(part_id.to_string(), value);
            }
            let total = self.total.compiled
                .eval_with_context(&context)
                .unwrap()
                .as_number()
                .unwrap() as f32;
            stats.set_cached(path.full_path, total);
            return total;
        }
    }
}