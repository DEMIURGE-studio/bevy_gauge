use bevy::{ecs::component::Component, utils::{HashMap, HashSet}};
use evalexpr::{ContextWithMutableVariables, DefaultNumericTypes, HashMapContext, Node, Value};

#[derive(Debug, Clone, Default)]
pub enum ModType {
    #[default]
    Add,
    Mul,
}

// TODO Fix overuse of .unwrap(). It's fine for now (maybe preferable during development) but in the future we'll want proper errors, panics, and 
// warnings.

// TODO ContextDrivenStats type that wraps stats, but contains a context (Hashmap of strings to entities). Can only call evaluate on it if you pass
// in a StatContextRefs

/// A collection of stats keyed by their names.
#[derive(Component, Debug, Default)]
pub struct Stats {
    // Holds the definitions of stats. This includes default values, their modifiers, and their dependents
    pub definitions: HashMap<String, StatType>,
    pub cached_stats: HashMapContext,
}

impl Stats {
    pub fn new() -> Self {
        Self { definitions: HashMap::new(), cached_stats: HashMapContext::new() }
    }

    /// Evaluates a stat by gathering all its parts and combining their values.
    pub fn evaluate<S>(&self, path: S) -> f32 
    where
        S: Into<String>,
    {
        let path_str = path.into();
        let segments: Vec<&str> = path_str.split("_").collect();
        let head = segments[0];

        let stat_type = self.definitions.get(head);
        let Some(stat_type) = stat_type else { return 0.0; };

        stat_type.evaluate(&segments, self)
    }

    /// Updates a stat's cached value and propagates to dependents
    pub fn update_stat(&mut self, stat_path: &str) {
        // Get the current value
        let value = self.evaluate(stat_path);
        
        // Update the cached value
        self.cached_stats.set_value(stat_path.to_string(), Value::Float(value as f64)).unwrap();
        
        // Gather all dependents
        let segments: Vec<&str> = stat_path.split("_").collect();
        let mut dependents = HashSet::new();
        
        if let Some(stat_type) = self.definitions.get(segments[0]) {
            stat_type.gather_dependents(&segments, &mut dependents);
        }
        
        // Propagate to dependents
        for dependent in dependents {
            self.update_stat(&dependent);
        }
    }

    pub fn add_modifier<V, S>(&mut self, stat_path: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType> + Clone,
    {
        let stat_path_str = stat_path.into();
        let stat_path_segments: Vec<&str> = stat_path_str.split("_").collect();
        let base_stat = stat_path_segments[0].to_string();

        if let Some(stat) = self.definitions.get_mut(&base_stat) {
            stat.add_modifier(&stat_path_segments, value.clone());
        } else {
            self.definitions.insert(base_stat.clone(), StatType::new(&stat_path_str, value.clone()));
        }

        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(_) => (),
            ValueType::Expression(depends_on_expression) => {
                self.add_dependent(&base_stat, &depends_on_expression);
            },
        }
        
        self.update_stat(&base_stat);
    }

    pub fn remove_modifier<V, S>(&mut self, stat_path: S, value: V)
    where
        S: Into<String>,
        V: Into<ValueType> + Clone,
    {
        let stat_path_str = stat_path.into();
        let stat_path_segments: Vec<&str> = stat_path_str.split("_").collect();
        let base_stat = stat_path_segments[0].to_string();

        if let Some(stat) = self.definitions.get_mut(&base_stat) {
            stat.remove_modifier(&stat_path_segments, value.clone());
        }

        let vt: ValueType = value.into();
        match vt {
            ValueType::Literal(_) => (),
            ValueType::Expression(expression) => {
                self.remove_dependent(&base_stat, &expression);
            },
        }
        
        self.update_stat(&base_stat);
    }

    fn add_dependent(&mut self, stat_path: &str, depends_on_expression: &Expression) {
        for depends_on in depends_on_expression.value.iter_variable_identifiers() {
            let depends_on_segments: Vec<&str> = depends_on.split("_").collect();
            let depends_on_base = depends_on_segments[0]; // The root stat name
    
            // If the stat doesn't exist, create it as DependentOnly
            if !self.definitions.contains_key(depends_on_base) {
                self.definitions.insert(stat_path.to_string(), StatType::Placeholder(Placeholder::default()));
            }
    
            // Add the dependent relationship
            if let Some(dep_stat) = self.definitions.get_mut(depends_on_base) {
                dep_stat.add_dependent(&depends_on_segments, stat_path);
            }
        }
    }

    fn remove_dependent(&mut self, stat_path: &str, depends_on_expression: &Expression) {
        for depends_on in depends_on_expression.value.iter_variable_identifiers() {
            let depends_on_segments: Vec<&str> = depends_on.split("_").collect();
            let depends_on_base = depends_on_segments[0]; // The root stat name

            if let Some(dep_stat) = self.definitions.get_mut(depends_on_base) {
                dep_stat.remove_dependent(&depends_on_segments, stat_path);
            }
        }
    }
}

pub trait StatLike {
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V);
    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V);
    fn add_dependent(&mut self, stat_path: &[&str], depends_on: &str);
    fn remove_dependent(&mut self, stat_path: &[&str], depends_on: &str);
    fn gather_dependents(&self, stat_path: &[&str], dependents: &mut HashSet<String>);
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32;
}

#[derive(Debug)]
pub enum StatType {
    Simple(Simple),
    Modifiable(Modifiable),
    Complex(ComplexModifiable),
    Placeholder(Placeholder),
}

impl StatType {
    pub fn new<V>(stat_path: &str, value: V) -> Self
    where
        V: Into<ValueType>,
    {
        let stat_path_segments: Vec<&str> = stat_path.split("_").collect();
        
        match stat_path_segments.len() {
            1 => {
                // Simple stat
                let vt: ValueType = value.into();
                match vt {
                    ValueType::Literal(v) => StatType::Simple(Simple::new(v)),
                    ValueType::Expression(_) => panic!(),
                }
            },
            2 => {
                // Modifiable stat
                let mut stat = Modifiable::new(stat_path_segments[0]);
                stat.add_modifier(&stat_path_segments, value);
                StatType::Modifiable(stat)
            },
            3 => {
                // Complex stat
                let mut stat = ComplexModifiable::new(stat_path_segments[0]);
                stat.add_modifier(&stat_path_segments, value);
                StatType::Complex(stat)
            },
            _ => panic!("Invalid stat path format: {}", stat_path)
        }
    }

    pub fn evaluate(&self, path: &[&str], stats: &Stats) -> f32 {
        match self {
            StatType::Simple(simple) => simple.value,
            StatType::Modifiable(modifiable) => {
                if path.len() == 1 {
                    modifiable.evaluate(stats)
                } else if path.len() == 2 {
                    modifiable.evaluate_part(path[1], stats)
                } else {
                    0.0 // invalid query
                }
            },
            StatType::Complex(complex_modifiable) => {
                complex_modifiable.evaluate(path, stats)
            },
            StatType::Placeholder(_) => 0.0,  // Always return
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
                modifiable.add_modifier(path, value);
            },
            StatType::Complex(complex_modifiable) => {
                complex_modifiable.add_modifier(path, value);
            },
            StatType::Placeholder(_) => {}
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
                modifiable.remove_modifier(path, value);
            },
            StatType::Complex(complex_modifiable) => {
                complex_modifiable.remove_modifier(path, value);
            },
            StatType::Placeholder(_) => {}
        }
    }
}

impl StatLike for StatType {
    fn add_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        match self {
            StatType::Simple(simple) => simple.add_dependent(stat_path, depends_on),
            StatType::Modifiable(modifiable) => modifiable.add_dependent(stat_path, depends_on),
            StatType::Complex(complex_modifiable) => complex_modifiable.add_dependent(stat_path, depends_on),
            StatType::Placeholder(dependent_only) => dependent_only.add_dependent(stat_path, depends_on),
        }
    }
    
    fn remove_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        match self {
            StatType::Simple(simple) => simple.remove_dependent(stat_path, depends_on),
            StatType::Modifiable(modifiable) => modifiable.remove_dependent(stat_path, depends_on),
            StatType::Complex(complex_modifiable) => complex_modifiable.remove_dependent(stat_path, depends_on),
            StatType::Placeholder(placeholder) => placeholder.remove_dependent(stat_path, depends_on),
        }
    }

    fn gather_dependents(&self, stat_path: &[&str], dependents: &mut HashSet<String>) {
        match self {
            StatType::Simple(simple) => simple.gather_dependents(stat_path, dependents),
            StatType::Modifiable(modifiable) => modifiable.gather_dependents(stat_path, dependents),
            StatType::Complex(complex_modifiable) => complex_modifiable.gather_dependents(stat_path, dependents),
            StatType::Placeholder(placeholder) => placeholder.gather_dependents(stat_path, dependents),
        }
    }
    
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        match self {
            StatType::Simple(simple) => simple.add_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.add_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.add_modifier(stat_path, value),
            StatType::Placeholder(_) => {}
        }
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        match self {
            StatType::Simple(simple) => simple.remove_modifier(stat_path, value),
            StatType::Modifiable(modifiable) => modifiable.remove_modifier(stat_path, value),
            StatType::Complex(complex_modifiable) => complex_modifiable.remove_modifier(stat_path, value),
            StatType::Placeholder(_) => {}
        }
    }
    
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32 {
        match self {
            StatType::Simple(simple) => simple.evaluate(stat_path, stats),
            StatType::Modifiable(modifiable) => modifiable.evaluate(stat_path, stats),
            StatType::Complex(complex_modifiable) => complex_modifiable.evaluate(stat_path, stats),
            StatType::Placeholder(_) => 0.0,
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
}

impl StatLike for Simple {
    fn add_dependent(&mut self, _stat_path: &[&str], depends_on: &str) {
        *self.dependents.entry(depends_on.to_string()).or_insert(0) += 1;
    }

    fn remove_dependent(&mut self, _stat_path: &[&str], depends_on: &str) {
        if let Some(count) = self.dependents.get_mut(depends_on) {
            *count -= 1;
            if *count == 0 {
                self.dependents.remove(depends_on);
            }
        }
    }
    
    fn gather_dependents(&self, stat_path: &[&str], dependents: &mut HashSet<String>) {
        if stat_path.len() == 1 {
            dependents.extend(self.dependents.keys().cloned());
        }
    }
    
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() == 1 {
            let vt: ValueType = value.into();
            if let ValueType::Literal(val) = vt {
                self.value += val;
            }
        }
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() == 1 {
            let vt: ValueType = value.into();
            if let ValueType::Literal(val) = vt {
                self.value -= val;
            }
        }
    }
    
    fn evaluate(&self, _stat_path: &[&str], _stats: &Stats) -> f32 { self.value }
}

#[derive(Debug)]
pub struct Modifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, StatModifierStep>,
    pub dependents: HashMap<String, u32>,
}

impl Modifiable {
    pub fn new(name: &str) -> Self {
        let total_expr = get_total_expr_from_name(name);
        let mut modifier_types = HashMap::new();
        
        // Parse the total expression to get all modifier names
        let modifier_names: Vec<&str> = total_expr.split(|c: char| !c.is_alphabetic())
            .filter(|s| !s.is_empty())
            .collect();
        
        // Create modifier steps for each name
        for modifier_name in modifier_names {
            let mut step = StatModifierStep::new(modifier_name);
            // Set initial base value
            step.base = get_initial_value_for_modifier(modifier_name);
            // Add parent as dependent
            step.add_dependent(name);
            
            modifier_types.insert(modifier_name.to_string(), step);
        }
        
        Modifiable { 
            total: Expression { 
                string: total_expr.to_string(),
                value: evalexpr::build_operator_tree(total_expr).unwrap(),
            },
            modifier_types,
            dependents: HashMap::new(),
        }
    }
}

impl StatLike for Modifiable  {
    fn add_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        match stat_path.len() {
            1 => {
                // Depending on total stat
                *self.dependents.entry(depends_on.to_string()).or_insert(0) += 1;
            }
            2 => {
                // Depending on specific modifier type
                let step = stat_path[1];
                let step_entry = self.modifier_types
                    .entry(step.to_string())
                    .or_insert(StatModifierStep::new(step));
                step_entry.add_dependent(depends_on);
            }
            _ => {
                // Invalid path length - could log a warning here
            }
        }
    }

    fn remove_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        match stat_path.len() {
            1 => {
                if let Some(count) = self.dependents.get_mut(depends_on) {
                    *count -= 1;
                    if *count == 0 {
                        self.dependents.remove(depends_on);
                    }
                }
            }
            2 => {
                if let Some(step) = self.modifier_types.get_mut(stat_path[1]) {
                    step.remove_dependent(depends_on);
                }
            }
            _ => {
                // Invalid path length
            }
        }
    }
    
    fn gather_dependents(&self, stat_path: &[&str], dependents: &mut HashSet<String>) {
        match stat_path.len() {
            1 => {
                // Total stat dependents
                dependents.extend(self.dependents.keys().cloned());
            }
            2 => {
                // Specific modifier step dependents
                if let Some(step) = self.modifier_types.get(stat_path[1]) {
                    dependents.extend(step.dependents.keys().cloned());
                }
            }
            _ => {}
        }
    }
    
    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() == 2 {
            let key = stat_path[1].to_string();
            let part = self.modifier_types.entry(key.clone())
                .or_insert(StatModifierStep::new(&key));
            part.add_modifier(value);
        }
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() == 2 {
            let key = stat_path[1].to_string();
            let part = self.modifier_types.entry(key.clone())
                .or_insert(StatModifierStep::new(&key));
            part.remove_modifier(value);
        }
    }
    
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32 {
        match stat_path.len() {
            1 => {
                self
                    .total
                    .value
                    .eval_with_context(&stats.cached_stats)
                    .unwrap()
                    .as_number()
                    .unwrap() as f32
            }
            2 => {
                let Some(part) = self.modifier_types.get(stat_path[1]) else {
                    return 0.0;
                };
        
                part.evaluate(stats)
            }
            _ => 0.0
        }
    }
}

#[derive(Debug)]
pub struct ComplexModifiable {
    pub total: Expression, // "(Added * Increased * More) override"
    pub modifier_types: HashMap<String, HashMap<u32, StatModifierStep>>,
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
    fn add_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        if stat_path.len() != 3 {
            return;
        } // log a warning?
        
        // Format should be "Damage_Added_123" where 123 is the tag
        let modifier_type = stat_path[1];
        if let Ok(tag) = stat_path[2].parse::<u32>() {
            let step_map = self.modifier_types
                .entry(modifier_type.to_string())
                .or_insert(HashMap::new());
            
            let step = step_map
                .entry(tag)
                .or_insert(StatModifierStep::new(modifier_type));
            
            step.add_dependent(depends_on);
        }
    }

    fn remove_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        if stat_path.len() != 3 {
            return;
        } // log a warning?

        let Some(step_map) = self.modifier_types.get_mut(stat_path[1]) else {
            return;
        }; // log a warning?
        
        let Ok(tag) = stat_path[2].parse::<u32>() else {
            return;
        }; // log a warning?
        
        let Some(step) = step_map.get_mut(&tag) else {
            return;
        }; // log a warning?

        step.remove_dependent(depends_on);
    }

    fn gather_dependents(&self, stat_path: &[&str], dependents: &mut HashSet<String>) {
        if stat_path.len() == 3 {
            if let Some(step_map) = self.modifier_types.get(stat_path[1]) {
                if let Ok(tag) = stat_path[2].parse::<u32>() {
                    if let Some(step) = step_map.get(&tag) {
                        dependents.extend(step.dependents.keys().cloned());
                    }
                }
            }
        }
    }

    fn add_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() == 3 {
            let modifier_type = stat_path[1];
            if let Ok(tag) = stat_path[2].parse::<u32>() {
                let step_map = self.modifier_types.entry(modifier_type.to_string())
                    .or_insert(HashMap::new());
                
                let step = step_map.entry(tag)
                    .or_insert(StatModifierStep::new(modifier_type));
                
                step.add_modifier(value);
            }
        }
    }

    fn remove_modifier<V: Into<ValueType>>(&mut self, stat_path: &[&str], value: V) {
        if stat_path.len() == 3 {
            if let Some(step_map) = self.modifier_types.get_mut(stat_path[1]) {
                if let Ok(tag) = stat_path[2].parse::<u32>() {
                    if let Some(step) = step_map.get_mut(&tag) {
                        step.remove_modifier(value);
                    }
                }
            }
        }
    }
    
    fn evaluate(&self, stat_path: &[&str], stats: &Stats) -> f32 {
        // Attempt to parse the query from the first segment.
        let search_bitflags = match stat_path.get(2) {
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
                        Some(value.evaluate(stats))
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
}

#[derive(Debug, Default)]
pub struct Placeholder {
    pub dependents: HashMap<String, u32>,
}

impl StatLike for Placeholder {
    fn add_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        // Placeholder stats only track dependents at the root level
        if stat_path.len() == 1 {
            *self.dependents.entry(depends_on.to_string()).or_insert(0) += 1;
        }
    }

    fn remove_dependent(&mut self, stat_path: &[&str], depends_on: &str) {
        if stat_path.len() == 1 {
            if let Some(count) = self.dependents.get_mut(depends_on) {
                *count -= 1;
                if *count == 0 {
                    self.dependents.remove(depends_on);
                }
            }
        }
    }
    
    fn gather_dependents(&self, stat_path: &[&str], dependents: &mut HashSet<String>) {
        if stat_path.len() == 1 {
            dependents.extend(self.dependents.keys().cloned());
        }
    }
    
    fn add_modifier<V: Into<ValueType>>(&mut self, _stat_path: &[&str], _value: V) { /* do nothing */ }
    
    fn remove_modifier<V: Into<ValueType>>(&mut self, _stat_path: &[&str], _value: V) { /* do nothing */ }
    
    fn evaluate(&self, _stat_path: &[&str], _stats: &Stats) -> f32 { 0.0 }
}

#[derive(Debug)]
pub struct StatModifierStep {
    pub relationship: ModType,
    pub base: f32,
    pub mods: Vec<Expression>,
    pub dependents: HashMap<String, u32>,
}

impl StatModifierStep {
    pub fn new(_name: &str) -> Self {
        let base = 0.0; // get_initial_value_for_modifier(name);
        Self { relationship: ModType::Add, base, mods: Vec::new(), dependents: HashMap::new() }
    }

    pub fn evaluate(&self, stats: &Stats) -> f32 {
        let computed: Vec<f32> = self.mods.iter().map(|expr| expr.evaluate(stats)).collect();

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

    fn add_dependent(&mut self, dependent: &str) {
        *self.dependents.entry(dependent.to_string()).or_insert(0) += 1;
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

#[derive(Debug, Clone)]
pub struct Expression {
    pub string: String,
    pub value: Node<DefaultNumericTypes>,
}

impl Expression {
    pub fn evaluate(&self, stats: &Stats) -> f32 {
        self.value
            .eval_with_context(&stats.cached_stats)
            .unwrap_or(Value::Float(0.0))
            .as_number()
            .unwrap_or(0.0) as f32
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

    fn assert_approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < f32::EPSILON * 100.0, "left: {}, right: {}", a, b);
    }

    // Helper function to create a fresh StatDefinitions with common stats
    fn test_stats() -> Stats {
        let mut stats = Stats::new();
        
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
        if let StatType::Simple(movespeed) = stats.definitions.get_mut("Movespeed").unwrap() {
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
        let stats = Stats::new();
        
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