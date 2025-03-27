use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub, SubAssign};
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::*;
use crate::stats::{StatCollection, StatType};
use crate::value_type::{Expression, StatValue, ValueType};

#[derive(Debug, Clone)]
pub enum ModifierStorage {
    Single(SimpleStatModifierStorage),
    BitMasked(BitMaskedStatModifierStorage),
}

#[derive(Debug, Clone, Default)]
pub struct SimpleStatModifierStorage {
    modifiers: EntityHashMap<ModifierValue>,
    modifier_total: ModifierValueTotal,
}

#[derive(Debug, Clone, Default)]
pub struct BitMaskedStatModifierStorage {

    pub tags: HashMap<u32, IntermediateModifierValue>, // the cached modifier value for a target tag
    pub modifiers: EntityHashMap<HashSet<u32>>,
}

impl Default for ModifierStorage {
    fn default() -> Self {
        Self::Single(SimpleStatModifierStorage::default())
    }
}

impl ModifierStorage {
    pub fn add_or_replace_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        match self {
            ModifierStorage::Single(storage) => {
                let ModifierStorageType::Single = &modifier.modifier_stat_target else { return };
                if let Some(modifier_value) = storage.modifiers.get_mut(&modifier_entity) {
                    storage.modifier_total -= modifier_value;
                    *modifier_value = modifier.value.clone();
                }

                storage.modifier_total += &modifier.value;
            }
            ModifierStorage::BitMasked(storage) => {

                let ModifierStorageType::BitMasked(source_tag) = &modifier.modifier_stat_target else { return };
                if !storage.tags.contains_key(&source_tag){
                    storage.tags.insert(*source_tag, IntermediateModifierValue::default());
                }

                for (tag, intermediate_modifier_value) in storage.tags.iter_mut() {
                    if tag & source_tag > 1 {
                        intermediate_modifier_value.entities.insert(modifier_entity);
                        intermediate_modifier_value.modifier_value += &modifier.value;
                        storage.modifiers.entry(modifier_entity).or_insert(HashSet::new()).insert(*tag);
                    }
                }
            }
        }

    }

    pub fn remove_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity) {
        match self {
            ModifierStorage::Single(storage) => {
                storage.modifiers.remove(&modifier_entity);
            }
            ModifierStorage::BitMasked(storage) => {
                if let Some(target_tags) = storage.modifiers.get_mut(&modifier_entity) { // set of target tags that this modifier effects
                    for target_tag in target_tags.drain() { // as we clear this take each target tag
                        if let Some(intermediate_modifier_value) = storage.tags.get_mut(&target_tag) { // find that target tag in the tags map
                            intermediate_modifier_value.entities.remove(&modifier_entity); // remove the modifier entity from the tag maps hashset of entities
                            intermediate_modifier_value.modifier_value -= &modifier.value; // decrement the modifier total
                            if intermediate_modifier_value.entities.is_empty() {
                                storage.tags.remove(&target_tag);
                            }
                        };
                    }
                }
                storage.modifiers.remove(&modifier_entity);
            }
        }
    }
}


#[derive(Debug, Clone)]
pub enum ModifierValue {
    Flat(ValueType),
    Increased(ValueType),
    More(ValueType)
}

impl ModifierValue {
    pub fn get_value_type(&self) -> ValueType {
        match self {
            ModifierValue::Flat(vt) => {vt.clone()}
            ModifierValue::Increased(vt) => {vt.clone()}
            ModifierValue::More(vt) => {vt.clone()}
        }
    }
}

impl Default for ModifierValue {
    fn default() -> Self {
        ModifierValue::Flat(ValueType::default())
    }
}


#[derive(Debug, Clone, PartialEq, Default)]
pub struct Intermediate/*<T: BitAnd + BitOr + BitAndAssign + BitOrAssign + Clone>*/ {
    pub tags: HashMap<u32, IntermediateModifierValue>, // the cached modifier value for a target tag
    pub modifiers: EntityHashMap<HashSet<u32>>,
    // DAMAGE.FIRE.SWORD.ONE_HANDED - DAMAGE, FIRE, SWORD, 1H
    // DAMAGE.ICE.SWORD.ONE_HANDED - DAMAGE, SWORD, 1H
}


#[derive(Debug, Clone, PartialEq, Default)]
pub struct IntermediateModifierValue {
    pub entities: EntityHashSet,
    pub modifier_value: ModifierValueTotal
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierValueTotal {
    flat: f32,
    increased: f32,
    more: f32,
}

impl ModifierValueTotal {
    pub fn get_total(&self) -> f32 {
        self.flat * self.increased * self.more
    }
}

impl Default for ModifierValueTotal {
    fn default() -> Self {
        Self {
            flat: 0.0,
            increased: 0.0,
            more: 1.0,
        }
    }
}

impl Add for ModifierValueTotal {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            flat: self.flat + other.flat,
            increased: self.increased + self.increased,
            more: (1.0 + self.more) * (1.0 + other.more)
        }
        
    }
}

impl Sub for ModifierValueTotal {
    type Output = Self;
    
    fn sub(self, other: Self) -> Self {
        Self {
            flat: self.flat - other.flat,
            increased: self.increased - self.increased,
            more: (1.0 + self.more) / (1.0 + other.more)
        }
    }
}


impl AddAssign<&ModifierValue> for ModifierValueTotal {
    fn add_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => { self.flat += val.evaluate()}
            ModifierValue::Increased(val) => { self.increased += val.evaluate() }
            ModifierValue::More(val) => { self.more *= 1.0 + val.evaluate() }
        }
    }
}

impl SubAssign<&ModifierValue> for ModifierValueTotal {
    fn sub_assign(&mut self, rhs: &ModifierValue) {
        match rhs {
            ModifierValue::Flat(val) => { self.flat -= val.evaluate()}
            ModifierValue::Increased(val) => { self.increased -= val.evaluate() }
            ModifierValue::More(val) => { self.more /= 1.0 + val.evaluate() }
        }
    }
}


/// An instance that lives as a component, or rather a single component entity that exists as a child on a tree of a Stat Entity.
#[derive(Component, Debug)]
#[require(ModifierInstance)]
#[relationship(relationship_target = ModifierCollectionRefs)]
pub struct ModifierTarget {
    pub modifier_collection: Entity,
}


#[derive(Component, Debug, Default)]
pub struct ModifierInstance {
    pub target_stat: String,
    pub modifier_stat_target: ModifierStorageType,
    pub value: ModifierValue,
    pub dependencies: HashSet<String>
}

#[derive(Debug, Default)]
pub enum ModifierStorageType {
    #[default]
    Single,
    BitMasked(u32),
}


// /// A component on an entity to keep track of all Modifier Entities affecting this entity
#[derive(Component, Debug, Default)]
#[require(StatCollection)]
#[relationship_target(relationship = ModifierTarget)]
pub struct ModifierCollectionRefs {
    #[relationship]
    modifiers: Vec<Entity>,
}


fn on_modifier_added(
    trigger: Trigger<OnAdd, ModifierInstance>,
    modifier_query: Query<(&ModifierInstance, &ModifierTarget)>,
    mut stat_query: Query<&mut StatCollection, With<ModifierCollectionRefs>>,
) {

    if let Ok((modifier, stat_entity)) = modifier_query.get(trigger.target()) {
        if let Ok(mut stat_collection) = stat_query.get_mut(stat_entity.modifier_collection) {
            let primary_target = &modifier.primary_target;
            let intermediate_name = format!("{}_mods", primary_target);

            // Check if the intermediate exists, create it if not
            if !stat_collection.stats.contains_key(&intermediate_name) {
                // Create a new intermediate
                let mut intermediate = Intermediate::default();

                // Add the tag from this modifier
                intermediate.tags.insert(
                    modifier.source_tag,
                    (EntityHashSet::default(), ModifierValueTotal::default())
                );
                
                let Some(&mut (ref mut entities, mut modifier_total)) = intermediate.tags.get_mut(&modifier.source_tag) else { todo!() };
                entities.insert(trigger.target());
                modifier_total += &modifier.value;
                
                // intermediate.tags.entry(modifier.modifier_definition.source_tag).and_modify(|(m, x)|);
                
                intermediate.modifiers.insert(
                    trigger.target(),
                    HashSet::new()
                );
                
                let Some(tags) = intermediate.modifiers.get_mut(&trigger.target()) else { todo!() };
                tags.insert(modifier.source_tag);
                
                

                // Insert the intermediate into the stat collection
                stat_collection.insert(&intermediate_name, StatType::Intermediate(intermediate));


            } else {
                // Update existing intermediate to include this tag if needed
                if let Some(StatType::Intermediate(intermediate)) = stat_collection.stats
                    .get_mut(&intermediate_name)
                    .map(|stat_instance| &mut stat_instance.stat)
                {
                    // Add the tag if it doesn't exist
                    let tag = modifier.source_tag;
                    if !intermediate.tags.contains_key(&tag) {
                        intermediate.tags.insert(tag, (EntityHashSet::default(), ModifierValueTotal::default()));
                    }
                }
            }

            // Apply the modifier to the intermediate
            if let Some(StatType::Intermediate(intermediate)) = stat_collection.stats
                .get_mut(&intermediate_name)
                .map(|stat_instance| &mut stat_instance.stat)
            {
                intermediate.add_modifier(modifier, trigger.target());
            }

            // Recalculate the total stat
            let total_name = format!("{}_total", primary_target);
            if stat_collection.stats.contains_key(&total_name) {
                stat_collection.recalculate(&total_name);
            }
        }
    }
}

/// Triggered when a modifier is removed from an entity
fn on_modifier_removed(
    trigger: Trigger<OnRemove, ModifierInstance>,
    modifier_query: Query<&ModifierInstance>,
    mut stat_query: Query<(Entity, &mut StatCollection), With<ModifierCollectionRefs>>,
) {
    if let Ok((entity, mut stat_collection)) = stat_query.single_mut() {
        // For each removed modifier
        if let Ok(modifier) = modifier_query.get(trigger.target()) {

            let primary_target = &modifier.primary_target;
            let intermediate_name = format!("{}_mods", primary_target);

            // Remove the modifier from the intermediate if it exists
            if let Some(StatType::Intermediate(intermediate)) = stat_collection.stats
                .get_mut(&intermediate_name)
                .map(|stat_instance| &mut stat_instance.stat)
            {
                // Remove the modifier
                intermediate.remove_modifier(&modifier, trigger.target().entity());

                // Recalculate the total stat
                let total_name = format!("{}_total", primary_target);
                if stat_collection.stats.contains_key(&total_name) {
                    stat_collection.recalculate(&total_name);
                }
            }
        }
    }
}

fn on_stat_changed(
    mut stat_updates: EventReader<StatUpdatedEvent>,
    mut modifier_query: Query<(Entity, &mut ModifierInstance, &ModifierTarget)>,
    stat_query: Query<&StatCollection, With<ModifierCollectionRefs>>,
) {
    // Process each stat update
    for update in stat_updates.iter() {
        // Find all modifiers that depend on this stat
        let dependent_modifiers: Vec<_> = modifier_query
            .iter_mut()
            .filter(|(_, modifier, _)| modifier.dependencies.contains(&update.stat_name))
            .collect();

        // Update each dependent modifier
        for (_, mut modifier, _) in dependent_modifiers {
            if let Ok(stat_collection) = stat_query.get(modifier.modifier_collection) {
                // Update the modifier value
                update_modifier_value(&mut modifier.modifier_definition.value, stat_collection);

                // Get the intermediate that contains this modifier
                let primary_target = &modifier.modifier_definition.primary_target;
                let intermediate_name = format!("{}_mods", primary_target);

                // Recalculate the total stat
                let total_name = format!("{}_total", primary_target);
                if let Ok(mut stat_collection) = stat_query.get_mut(modifier.modifier_collection) {
                    stat_collection.recalculate(&total_name);
                }
            }
        }
    }
}

/// Event sent when a stat is updated
#[derive(Event)]
pub struct StatUpdatedEvent {
    pub stat_name: String,
}

// fn update_modifier_value(modifier_value: &mut ModifierValue, stat_collection: &StatCollection) {
//     match modifier_value {
//         ModifierValue::Flat(value_type) => {
//             if let ValueType::Expression(expr) = value_type {
//                 update_expression_with_stats(expr, stat_collection);
//             }
//         },
//         ModifierValue::Increased(value_type) => {
//             if let ValueType::Expression(expr) = value_type {
//                 update_expression_with_stats(expr, stat_collection);
//             }
//         },
//         ModifierValue::More(value_type) => {
//             if let ValueType::Expression(expr) = value_type {
//                 update_expression_with_stats(expr, stat_collection);
//             }
//         },
//     }
// }

// fn update_expression_with_stats(expr: &mut Expression, stat_collection: &StatCollection) {
//     // Create a context with stat values
//     let mut context = evalexpr::HashMapContext::new();
// 
//     // Get all variables in the expression
//     for var_name in expr.iter_variable_identifiers() {
//         // Try to get the stat value from the collection
//         let value = stat_collection.stats
//             .get(var_name)
//             .map(|stat_instance| stat_instance.stat.get_value())
//             .unwrap_or(0.0);
// 
//         // Add to context
//         context
//             .set_value(var_name.to_string(), evalexpr::Value::from_float(value as f64))
//             .unwrap_or_default();
//     }
// 
//     // Update the cached value
//     expr.cached_value = expr
//         .eval_with_context_mut(&mut context)
//         .unwrap_or(evalexpr::Value::from_float(0.0))
//         .as_number()
//         .unwrap_or(0.0) as f32;
// }
// 
// /// Register the trigger handlers
// pub fn register_modifier_triggers(app: &mut App) {
//     app
//         .add_event::<StatUpdatedEvent>()
//         .add_systems(Update, (
//             on_modifier_added.run_if(on_event::<OnAdd<ModifierInstance>>()),
//             on_modifier_removed.run_if(on_event::<OnRemove<ModifierInstance>>()),
//             on_stat_changed,
//         ));
// }
