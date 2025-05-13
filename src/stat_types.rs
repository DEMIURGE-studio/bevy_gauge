use bevy::utils::HashMap;
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value};
use super::prelude::*;
use std::collections::HashSet;
use dashmap::DashMap;

/// Defines how modifiers are combined (additive or multiplicative).
/// 
/// This enum determines how multiple modifiers to the same stat are combined together.
#[derive(PartialEq, Debug, Clone, Default)]
pub(crate) enum ModType {
    /// Modifiers are added together linearly.
    /// Example: "10% increased damage" + "20% increased damage" = 30% increased damage
    #[default]
    Add,
    /// Modifiers are multiplied together.
    /// Example: "10% more damage" * "20% more damage" = 32% more damage
    Mul,
}

/// The core enum representing different types of stats in the system.
/// Each variant handles different complexity levels and modification patterns.
#[derive(Debug, Clone)]
pub(crate) enum StatType {
    /// Simple numeric value with no complex modification rules.
    /// Example: Base health that just gets directly modified
    Flat(Flat),
    /// Value that can be modified by additive or multiplicative modifiers.
    /// Example: Damage that can be increased by percentage modifiers
    Modifiable(Modifiable),
    /// Stat composed of multiple parts combined through an expression.
    /// Example: Final damage = base * (1 + increased) * (1 + more)
    Complex(Complex),
    /// Stat that can be filtered and queried by tags.
    /// Example: "Increased fire damage with axes" combines fire and axe modifiers
    Tagged(Tagged),
}

impl Stat for StatType {
    fn new(path: &StatPath, config: &Config) -> Self {
        let stat_type = config.get_stat_type(path);
        match stat_type {
            "Flat" => StatType::Flat(Flat::new(path, config)),
            "Modifiable" => StatType::Modifiable(Modifiable::new(path, config)),
            "Complex" => StatType::Complex(Complex::new(path, config)),
            "Tagged" => StatType::Tagged(Tagged::new(path, config)),
            _ => panic!("Invalid stat type!"),
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
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        match self {
            StatType::Flat(flat) => flat.0,
            StatType::Modifiable(simple) => simple.evaluate(path, stats),
            StatType::Complex(modifiable) => modifiable.evaluate(path, stats),
            StatType::Tagged(complex_modifiable) => complex_modifiable.evaluate(path, stats),
        }
    }
}

/// The simplest stat type - just holds a single numeric value.
/// 
/// # Examples
/// 
/// - Base health: Direct numeric value
/// - Resource costs: Simple numbers that get modified directly
/// - Level requirements: Plain numeric values
/// 
/// # Modification Behavior
/// 
/// - Only accepts literal number modifications
/// - Modifications directly add to or subtract from the base value
/// - No support for percentage or complex modifications
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

/// A stat that can be modified by either additive or multiplicative modifiers.
/// 
/// # Fields
/// 
/// * `relationship`: Determines if modifiers are added or multiplied
/// * `base`: The starting value before any modifiers
/// * `mods`: List of expressions that modify the base value
/// 
/// # Examples
/// 
/// Additive (relationship = Add):
/// ```
/// base = 100
/// mod1 = +50% (0.5)
/// mod2 = +30% (0.3)
/// final = 100 * (1 + 0.5 + 0.3) = 180
/// ```
/// 
/// Multiplicative (relationship = Mul):
/// ```
/// base = 100
/// mod1 = 50% more (1.5)
/// mod2 = 30% more (1.3)
/// final = 100 * 1.5 * 1.3 = 195
/// ```
#[derive(Debug, Clone)]
pub(crate) struct Modifiable {
    pub(crate) relationship: ModType,
    pub(crate) base: f32,
    pub(crate) mods: Vec<Expression>,
}

impl Stat for Modifiable {
    fn new(path: &StatPath, config: &Config) -> Self {
        let relationship = config.get_relationship_type(path);
        Self { relationship, base: 0.0, mods: Vec::new() }
    }

    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType, _config: &Config) {
        match modifier {
            ModifierType::Literal(vals) => { 
                match self.relationship {
                    ModType::Add => self.base += vals,
                    ModType::Mul => self.base *= vals,
                }
            }
            ModifierType::Expression(expression) => self.mods.push(expression.clone()),
        }
    }

    fn remove_modifier(&mut self, _path: &StatPath, modifier: &ModifierType) {
        match modifier {
            ModifierType::Literal(vals) => { 
                match self.relationship {
                    ModType::Add => self.base -= vals,
                    ModType::Mul => self.base /= vals,
                }
            }
            ModifierType::Expression(expression) => {
                if let Some(pos) = self.mods.iter().position(|e| e == expression) {
                    self.mods.remove(pos);
                }
            }
        }
    }

    fn evaluate(&self, _path: &StatPath, stats: &Stats) -> f32 {
        let computed: Vec<f32> = self.mods.iter()
            .map(|expr| expr.evaluate(stats.get_context()))
            .collect();
        match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => self.base * computed.iter().product::<f32>(),
        }
    }
}

/// A stat composed of multiple parts that are combined through an expression.
/// 
/// # Fields
/// 
/// * `total`: Expression that defines how to combine the parts
/// * `modifier_steps`: Named parts that can each be modified independently
/// 
/// # Example
/// 
/// Final damage calculation:
/// ```
/// parts:
///   - base_damage: Flat value
///   - increased_damage: Sum of all "increased" modifiers
///   - more_damage: Product of all "more" modifiers
/// total = "base_damage * (1 + increased_damage) * more_damage"
/// ```
/// 
/// Each part can have its own modifiers and they're combined according to the expression.
#[derive(Debug, Clone)]
pub(crate) struct Complex {
    pub(crate) total: Expression,
    pub(crate) modifier_steps: HashMap<String, Modifiable>,
}

impl Stat for Complex {
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
        let Some(part_key) = path.part else { return };
        let part = self.modifier_steps.get_mut(part_key).unwrap();
        part.add_modifier(path, modifier, config);
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) {
        let Some(part_key) = path.part else { return };
        let part = self.modifier_steps.get_mut(part_key).unwrap();
        part.remove_modifier(path, modifier);
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        if let Some(part_key) = path.part {
            let Some(part) = self.modifier_steps.get(part_key) else { return 0.0 };
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

/// A stat that supports tag-based filtering and querying.
/// 
/// Tagged stats handle cases where modifiers can apply broadly but need to be
/// queried specifically. For example, "increased fire damage" applies to all weapons,
/// but we query it as "increased fire damage with axes" for specific calculations.
/// 
/// # Fields
/// 
/// * `total`: Expression combining all parts after tag filtering
/// * `modifier_steps`: Map of parts to their tagged modifiers
/// * `query_cache`: Thread-safe cache of previously computed queries
/// 
/// # Caching
/// 
/// Uses DashMap for thread-safe caching without blocking. Cache entries are
/// invalidated when relevant modifiers change. The cache is eventually consistent,
/// meaning duplicate work might happen but results will be correct.
/// 
/// # Example
/// 
/// ```
/// // Adding modifiers:
/// "50% increased fire damage" -> applies to all weapons
/// "30% increased damage with axes" -> applies to all damage types
/// 
/// // Querying:
/// "increased fire damage with axes" -> combines both modifiers
/// ```
#[derive(Debug, Clone)]
pub(crate) struct Tagged {
    pub(crate) total: Expression,
    pub(crate) modifier_steps: HashMap<String, TaggedEntry>,
    query_cache: DashMap<(String, u32), QueryCacheEntry>,
}

/// Entry in a Tagged stat's modifier map, storing modifiers for a specific combination of tags.
/// The u32 key represents the tag bits, and the Modifiable contains the actual stat modifications.
#[derive(Debug, Clone)]
pub(crate) struct TaggedEntry(pub HashMap<u32, Modifiable>);

impl TaggedEntry {
    fn new() -> Self {
        Self(HashMap::new())
    }
}

/// Cache entry for a Tagged stat query, storing both the computed value and its tag dependencies.
/// 
/// # Fields
/// 
/// * `value`: The computed result of the query
/// * `dependencies`: Set of tags this query depends on, used for cache invalidation
#[derive(Debug, Clone)]
struct QueryCacheEntry {
    value: f32,
    dependencies: HashSet<u32>,
}

impl Tagged {
    fn evaluate_part(&self, part: &str, tag: u32, stats: &Stats) -> f32 {
        let cache_key = (part.to_string(), tag);
        
        if let Some(cache_entry) = self.query_cache.get(&cache_key) {
            return cache_entry.value;
        }

        let mut dependencies = HashSet::new();
        let total: f32 = self.modifier_steps.get(part)
            .map(|modifiers| {
                modifiers.0.iter()
                    .filter_map(|(&mod_tags, value)| {
                        if mod_tags.has_all(tag) {
                            dependencies.insert(mod_tags);
                            Some(value.evaluate(&StatPath::parse(part), stats))
                        } else {
                            None
                        }
                    })
                    .sum()
            })
            .unwrap_or(0.0);

        self.query_cache.insert(cache_key, QueryCacheEntry {
            value: total,
            dependencies,
        });

        total
    }

    fn invalidate_dependent_cache_entries(&self, affected_tag: u32) {
        self.query_cache.retain(|_, entry| {
            !entry.dependencies.iter().any(|&dep_tag| dep_tag.has_any(affected_tag))
        });
    }

    pub fn clear_cache(&self) {
        self.query_cache.clear();
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
            query_cache: DashMap::new(),
        }
    }

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType, config: &Config) {
        let Some(tag) = path.tag else { return };
        let Some(part) = path.part else { return };

        let step_map = self.modifier_steps.entry(part.to_string())
            .or_insert(TaggedEntry(HashMap::new()));
        let step = step_map.0.entry(tag).or_insert(Modifiable::new(path, config));
        step.add_modifier(path, modifier, config);

        self.invalidate_dependent_cache_entries(tag);
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) {
        let Some(tag) = path.tag else { return };
        let Some(part) = path.part else { return };

        if let Some(step_map) = self.modifier_steps.get_mut(part) {
            if let Some(step) = step_map.0.get_mut(&tag) {
                step.remove_modifier(path, modifier);
            }
        }

        self.invalidate_dependent_cache_entries(tag);
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        let Some(tag) = path.tag else { return 0.0 };

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