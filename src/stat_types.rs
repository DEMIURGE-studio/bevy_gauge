use bevy::utils::HashMap;
use evalexpr::{ContextWithMutableVariables, Value, IterateVariablesContext, Context};

use super::prelude::*;
use dashmap::DashMap;

/// Defines how modifiers are combined, typically for a specific part of a stat.
///
/// When multiple modifiers apply to the same stat part (e.g., multiple sources of "increased damage"),
/// `ModType` determines if their effects are additive or multiplicative.
#[derive(PartialEq, Debug, Clone, Default)]
pub enum ModType {
    /// Modifiers are summed together. This is common for "increased" or "added" effects.
    /// For example, +10% damage and +20% damage result in +30% damage.
    #[default]
    Add,
    /// Modifiers are multiplied together. This is common for "more" or "less" effects.
    /// For example, a 10% "more" multiplier (1.1x) and a 20% "more" multiplier (1.2x)
    /// result in a total multiplier of 1.1 * 1.2 = 1.32x (or 32% more).
    Mul,
}

/// The core internal enum representing different kinds of stat structures and behaviors.
/// Each variant dictates how a stat stores its data, processes modifiers, and calculates its final value.
/// This is primarily used internally by the stat system based on configurations provided in `Config`.
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
    fn new(path: &StatPath) -> Self {
        let stat_type_name = Konfig::get_stat_type(path.name);
        match stat_type_name.as_str() {
            "Flat" => StatType::Flat(Flat::new(path)),
            "Modifiable" => StatType::Modifiable(Modifiable::new(path)),
            "Complex" => StatType::Complex(Complex::new(path)),
            "Tagged" => StatType::Tagged(Tagged::new(path)),
            _ => panic!("Invalid stat type: {}", stat_type_name),
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

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType) {
        match self {
            StatType::Flat(flat) => flat.add_modifier(path, modifier),
            StatType::Modifiable(simple) => simple.add_modifier(path, modifier),
            StatType::Complex(modifiable) => modifiable.add_modifier(path, modifier),
            StatType::Tagged(complex_modifiable) => complex_modifiable.add_modifier(path, modifier),
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

    fn clear_internal_cache(&mut self, path: &StatPath) -> Vec<String> {
        match self {
            StatType::Flat(_) => Vec::new(), /* No internal cache for Flat */
            StatType::Modifiable(_) => Vec::new(), /* No internal cache for Modifiable */
            StatType::Complex(_) => Vec::new(), /* No internal cache for Complex currently, or it's handled by parts */
            StatType::Tagged(tagged) => tagged.clear_internal_cache(path),
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
    fn new(_path: &StatPath) -> Self { Self(0.0) }

    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType) {
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
/// ```text
/// base = 100
/// mod1 = +50% (0.5)
/// mod2 = +30% (0.3)
/// final = 100 * (1 + 0.5 + 0.3) = 180
/// ```
/// 
/// Multiplicative (relationship = Mul):
/// ```text
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
    fn new(path: &StatPath) -> Self {
        let relationship = Konfig::get_relationship_type(path.name);
        let base = if relationship == ModType::Mul { 1.0 } else { 0.0 };
        Self { relationship, base, mods: Vec::new() }
    }

    fn add_modifier(&mut self, _path: &StatPath, modifier: ModifierType) {
        match modifier {
            ModifierType::Literal(vals) => { 
                match self.relationship {
                    ModType::Add => self.base += vals,
                    ModType::Mul => {
                        // If base is 0.0 from a previous Add context or uninitialized for Mul, treat this literal as the new base for multiplication
                        if self.base == 0.0 && self.mods.is_empty() { 
                            self.base = vals;
                        } else {
                            self.base *= vals;
                        }
                    }
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
            .map(|expr| {
                // Use Stats::evaluate_expression which handles missing variables properly
                stats.evaluate_expression(&expr.definition, None).unwrap_or(0.0)
            })
            .collect();
        
        let result = match self.relationship {
            ModType::Add => self.base + computed.iter().sum::<f32>(),
            ModType::Mul => self.base * computed.iter().product::<f32>(),
        };
        result
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
/// ```text
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
    fn new(path: &StatPath) -> Self {
        let total_expression_str = Konfig::get_total_expression(path.name);
        let compiled_expression = Expression::new(&total_expression_str).unwrap_or_else(|e| panic!("Failed to compile total_expression for {}: {} - Error: {}", path.name, total_expression_str, e));

        let mut modifier_steps = HashMap::new();
        for part in compiled_expression.compiled.iter_identifiers() {
            let part_path = &StatPath::parse(part);
            let step = Modifiable::new(part_path);
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

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType) {
        let Some(part_key) = path.part else { return };
        let part = self.modifier_steps.get_mut(part_key).unwrap();
        part.add_modifier(path, modifier);
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
            let mut expression_context = evalexpr::HashMapContext::new();

            for (part_name, _modifiable_part_definition) in &self.modifier_steps {
                let part_full_path_str = format!("{}.{}", path.name, part_name);
                let part_value = stats.evaluate_by_string(&part_full_path_str);
                expression_context.set_value(part_name.clone(), evalexpr::Value::Float(part_value as f64))
                    .map_err(|e| StatError::Internal{details: format!("Failed to set part '{}' in Complex eval context: {}", part_name, e)})
                    .unwrap();
            }
            
            let main_cache_context = stats.get_context();
            for (var_key, var_val) in main_cache_context.iter_variables() {
                if expression_context.get_value(&var_key).is_none() {
                    expression_context.set_value(var_key.clone().into(), var_val.clone())
                        .map_err(|e| StatError::Internal{details: format!("Failed to merge var '{}' in Complex eval context: {}", var_key, e)})
                        .unwrap();
                }
            }
            
            let total_expr_str = self.total.definition.as_str();
            let total = self.total.compiled
                .eval_with_context(&expression_context)
                .map_err(|e| StatError::ExpressionError { expression: total_expr_str.to_string(), details: e.to_string() })
                .unwrap()
                .as_number()
                .unwrap_or(0.0) as f32;

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
/// ```text
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
    pub(crate) query_tracker: DashMap<(String, u32), ()>, // Track queries made, but don't cache results
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

impl Tagged {
    fn evaluate_part(&self, part: &str, tag: u32, stats: &Stats) -> f32 {
        let Some(tagged_entry) = self.modifier_steps.get(part) else {
            return 0.0;
        };

        let mod_type = Konfig::get_relationship_type(part);

        let mut relevant_mod_values = Vec::new();
        for (mod_tag_key, modifiable_stat_for_tag) in &tagged_entry.0 {
            // Debug output
            println!("Checking modifier tag {} against query tag {}", mod_tag_key, tag);
            
            // Check if a permissive modifier applies to a strict query
            // Permissive modifiers start as u32::MAX and have category bits cleared, then specific bits set
            // Strict queries are just the combination of specific bits (e.g., FIRE | AXE = 65)
            // A permissive modifier applies if the strict query "satisfies" what the modifier requires
            let modifier_applies = if tag == 0 {
                true // tag 0 means "match everything"
            } else if *mod_tag_key == u32::MAX {
                false // u32::MAX means no valid tags were resolved, shouldn't match anything
            } else {
                // For permissive tags: check if the strict query has all the bits that the permissive modifier requires
                // The permissive modifier has the required bits set and "don't care" bits as 1
                // We need to check if (query & modifier) == query, meaning the modifier covers the query
                (tag & mod_tag_key) == tag
            };
            
            println!("  (tag & mod_tag_key) == tag: ({} & {}) == {} -> {} == {} -> {}", 
                     tag, mod_tag_key, tag, tag & mod_tag_key, tag, modifier_applies);
            
            if modifier_applies {
                let mod_value = modifiable_stat_for_tag.evaluate(&StatPath::parse(""), stats);
                relevant_mod_values.push(mod_value);
                println!("  -> Modifier applies, value: {}", mod_value);
            } else {
                println!("  -> Modifier does not apply");
            }
        }

        if relevant_mod_values.is_empty() {
            return if mod_type == ModType::Mul { 1.0 } else { 0.0 };
        }
        
        let final_value = match mod_type {
            ModType::Add => relevant_mod_values.iter().sum(),
            ModType::Mul => relevant_mod_values.iter().product(),
        };
        final_value
    }


}

impl Stat for Tagged {
    fn new(path: &StatPath) -> Self {
        let total_expression_str = Konfig::get_total_expression(path.name);
        let compiled_expression = Expression::new(&total_expression_str).unwrap_or_else(|e| panic!("Failed to compile total_expression for {}: {} - Error: {}", path.name, total_expression_str, e));

        let mut modifier_steps = HashMap::new();
        for part in compiled_expression.compiled.iter_identifiers() {
            let step = TaggedEntry::new();
            modifier_steps.insert(part.to_string(), step);
        }

        Self {
            total: compiled_expression,
            modifier_steps,
            query_tracker: DashMap::new(),
        }
    }

    fn add_modifier(&mut self, path: &StatPath, modifier: ModifierType) {
        let Some(tag) = path.tag else { return };
        let Some(part) = path.part else { return };

        let step_map = self.modifier_steps.entry(part.to_string())
            .or_insert(TaggedEntry(HashMap::new()));
        let step = step_map.0.entry(tag).or_insert(Modifiable::new(path));
        step.add_modifier(path, modifier);

        // Note: We can't call invalidate_dependent_cache_entries here because we don't have access to Stats
        // The invalidation will be handled by the StatsMutator when it calls clear_internal_cache_for_path
    }

    fn remove_modifier(&mut self, path: &StatPath, modifier: &ModifierType) {
        let Some(tag) = path.tag else { return };
        let Some(part) = path.part else { return };

        if let Some(step_map) = self.modifier_steps.get_mut(part) {
            if let Some(step) = step_map.0.get_mut(&tag) {
                step.remove_modifier(path, modifier);
            }
        }

        // Note: We can't call invalidate_dependent_cache_entries here because we don't have access to Stats
        // The invalidation will be handled by the StatsMutator when it calls clear_internal_cache_for_path
    }
    
    fn evaluate(&self, path: &StatPath, stats: &Stats) -> f32 {
        if let (Some(part_name), Some(tag_val)) = (&path.part, path.tag) {
            // Track this query for future invalidation, but don't cache the result
            let query_key = (part_name.to_string(), tag_val);
            self.query_tracker.insert(query_key, ());
            
            // Always compute the value fresh (Stats component will handle caching)
            let value = self.evaluate_part(part_name, tag_val, stats);
            return value;
        } 
        // There is no (part.part.is_some() && path.tag.is_none()) case, because a tag is required for a tagged stat.
        else if path.part.is_none() && path.tag.is_some() {
            let tag_val = path.tag.unwrap();
            let mut context = stats.cached_stats.context().clone();
            for (part_name_in_total_expr, _step_definition) in &self.modifier_steps {
                let part_value = self.evaluate_part(part_name_in_total_expr, tag_val, stats);
                context.set_value(part_name_in_total_expr.clone(), Value::Float(part_value as f64)).unwrap();
            }
            let total_val = self.total.evaluate(&context);
            stats.set_cached(&path.full_path, total_val);
            return total_val;
        }
        0.0
    }

    fn clear_internal_cache(&mut self, path: &StatPath) -> Vec<String> {
        let mut paths_to_invalidate = Vec::new();
        
        if let Some(tag) = path.tag {
            // Find all tracked queries that would be affected by this tag change
            self.query_tracker.retain(|(part, query_tag_from_key), _| {
                let should_invalidate = if *query_tag_from_key == 0 {
                    true // query tag 0 means "match everything", so any change affects it
                } else if tag == u32::MAX {
                    false // u32::MAX means no valid tags, shouldn't affect anything
                } else {
                    // Check if the affected permissive modifier would apply to this tracked query
                    (*query_tag_from_key & tag) == *query_tag_from_key
                };
                
                if should_invalidate {
                    // Build the full path for this query to invalidate in Stats cache
                    let full_path = format!("{}.{}.{}", path.name, part, query_tag_from_key);
                    paths_to_invalidate.push(full_path);
                }
                
                !should_invalidate // retain returns true for items to keep, false for items to remove
            });
        } else {
            // If no specific tag, clear all tracked queries for this stat
            self.query_tracker.clear();
        }
        
        paths_to_invalidate
    }
}