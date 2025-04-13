use bevy::utils::HashMap;
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value};
use super::prelude::*;

#[derive(Debug, Clone, Default)]
pub(crate) enum ModType {
    #[default]
    Add,
    Mul,
}

#[derive(Debug)]
pub(crate) enum StatType {
    Simple(Simple),
    Modifiable(Modifiable),
    Complex(ComplexModifiable),
}

impl StatType {
    pub(crate) fn new<V>(stat_path: &str, value: V) -> Self
    where
        V: Into<ValueType> + Clone,
    {
        let stat_path = StatPath::parse(stat_path);
        match stat_path.segments.len() {
            1 => {
                let mut stat = Simple::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value.into());
                StatType::Simple(stat)
            },
            2 => {
                let mut stat = Modifiable::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value.into());
                StatType::Modifiable(stat)
            },
            3 => {
                let mut stat = ComplexModifiable::new(&stat_path.segments[0]);
                stat.add_modifier(&stat_path, value.into());
                StatType::Complex(stat)
            },
            _ => panic!("Invalid stat path format: {:#?}", stat_path)
        }
    }
}

impl StatLike for StatType {
    fn add_modifier(&mut self, stat_path: &StatPath, value: ValueType) {
        match self {
            StatType::Simple(simple) => simple.add_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.add_modifier(stat_path, value),
        }
    }

    fn remove_modifier(&mut self, stat_path: &StatPath, value: &ValueType) {
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
pub(crate) struct Simple {
    pub(crate) relationship: ModType,
    pub(crate) base: f32,
    pub(crate) mods: Vec<Expression>,
}

impl Simple {
    pub(crate) fn new(name: &str) -> Self {
        //let base = get_initial_value_for_modifier(name);
        Self { relationship: ModType::Add, base: 0.0, mods: Vec::new() }
    }
}

impl StatLike for Simple {
    fn add_modifier(&mut self, _stat_path: &StatPath, value: ValueType) {
        match value {
            ValueType::Literal(vals) => { self.base += vals; }
            ValueType::Expression(expression) => { self.mods.push(expression.clone()); }
        }
    }

    fn remove_modifier(&mut self, _stat_path: &StatPath, value: &ValueType) {
        match value {
            ValueType::Literal(vals) => { self.base -= vals; }
            ValueType::Expression(expression) => {
                let Some(pos) = self.mods.iter().position(|e| e == expression) else { return; };
                self.mods.remove(pos);
            }
        }
    }

    fn evaluate(&self, _stat_path: &StatPath, stats: &Stats) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stats.get_context())).collect();
        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => self.base * computed.iter().product::<f32>(),
        }
    }

    fn on_insert(&self, _stats: &Stats, _stat_path: &StatPath) { }
}

#[derive(Debug)]
pub(crate) struct Modifiable {
    pub(crate) total: Expression, // "(Added * Increased * More) override"
    pub(crate) modifier_steps: HashMap<String, Simple>,
}

impl Modifiable {
    pub(crate) fn new(name: &str) -> Self {
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
                    expr.replace(word, &format!("{}.{}", name, word))
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
    fn add_modifier(&mut self, stat_path: &StatPath, value: ValueType) {
        if stat_path.len() != 2 { return; }
        let key = stat_path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.add_modifier(stat_path, value);
    }

    fn remove_modifier(&mut self, stat_path: &StatPath, value: &ValueType) {
        if stat_path.len() != 2 { return; }
        let key = stat_path.segments[1].to_string();
        let part = self.modifier_steps.entry(key.clone()).or_insert(Simple::new(&key));
        part.remove_modifier(stat_path, value);
    }
    
    fn evaluate(&self, stat_path: &StatPath, stats: &Stats) -> f32 {
        match stat_path.len() {
            1 => {
                self.total.value
                    .eval_with_context(stats.get_context())
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
            let full_modifier_path = format!("{}.{}", base_name, modifier_name);
            if stats.get(&full_modifier_path).is_err() {
                let val = self.modifier_steps.get(modifier_name).unwrap().evaluate(stat_path, stats);
                stats.set_cached(&full_modifier_path, val);
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct ComplexEntry(f32, HashMap<u32, Simple>);

/// The problem with ComplexModifiable is that any given query is a dependent of every flag in it and there are many
/// possible queries. So in order to prevent storing a million potential queries and their dependencies for every possible
/// query, we should only store queries and dependent entries for queries that are made by the user. For instance
/// Damage.FIRE|AXE|1H query entry in the cache is made the first time the query is made, and all of the stats that
/// query is dependent on (Damage.FIRE, Damage.AXE, Damage.ANY, etc) is put in the stat dependents. I.e., the query 
/// Damage.FIRE|AXE|1H is dependent on the stats Damage.FIRE, Damage.ANY, etc.
/// 
/// So when the query is made, every damage stat is iterated over and checked to see if the flags match the query. If they
/// do, the stat is collated into a generic category and the query is added as a dependent of that stat. Then the query 
/// and its final value are cached.
/// 
/// What happens if we later add a stat like "Damage.ELEMENTAL"? Elemental is a meta tag representing FIRE|ICE|LIGHTNING, 
/// so Damage.FIRE|AXE|1H would benefit from it. However, the query Damage.FIRE|AXE|1H was not added as a dependent of 
/// Damage.ELEMENTAL because the character did not have a Damage.ELEMENTAL stat at the moment the query was made. 
/// 
/// I think it is becoming clear that ComplexModifiable needs to store more data. Perhaps a cache of every query that has
/// been made. Then when a new stat is added, the list of made queries can be iterated over and cached query values can
/// be properly updated where appropriate. ComplexModifiable could also have a dirty bool that represents whether or not
/// it has been internally updated since the last time it was evaluated. If it has been, the bit is dirty and the query
/// value must be fully re-evaluated. Otherwise it just returns the cached value.

#[derive(Debug)]
pub(crate) struct ComplexModifiable {
    pub(crate) total: Expression, // "(Added * Increased * More) override"
    pub(crate) modifier_types: HashMap<String, ComplexEntry>,
}

impl ComplexModifiable {
    pub(crate) fn new(name: &str) -> Self {
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
    fn add_modifier(&mut self, stat_path: &StatPath, value: ValueType) {
        if stat_path.len() != 3 { return; }
        let modifier_type = &stat_path.segments[1];
        let Ok(tag) = stat_path.segments[2].parse::<u32>() else { return; };
        let step_map = self.modifier_types.entry(modifier_type.to_string())
            .or_insert(ComplexEntry(get_initial_value_for_modifier(modifier_type), HashMap::new()));
        let step = step_map.1.entry(tag).or_insert(Simple::new(modifier_type));
        step.add_modifier(stat_path, value);
    }

    fn remove_modifier(&mut self, stat_path: &StatPath, value: &ValueType) {
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
                            let dep_path = format!("{}.{}.{}", stat_path.segments[0], category, mod_tags.to_string());
                            stats.add_dependent(&dep_path, DependentType::LocalStat(full_path.to_string()));
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
                        let dep_path = format!("{}.{}.{}", stat_path.segments[0], category, mod_tags.to_string());
                        stats.add_dependent(&dep_path, DependentType::LocalStat(full_path.to_string()));
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