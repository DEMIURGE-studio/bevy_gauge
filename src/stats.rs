use std::collections::{HashMap, HashSet};
use bevy::prelude::*;
use crate::modifiers::{ModifierValueTotal};
use crate::prelude::AttributeInstance;
use crate::resource::{ResourceInstance};
use crate::tags::ValueTag;
use crate::value_type::{StatError, ValueType};

#[derive(Debug, Clone)]
pub enum StatInstance {
    Attribute(AttributeInstance),
    Resource(ResourceInstance),
    Intermediate(ModifierValueTotal)
}

impl StatInstance {
    pub fn get_value(&self, stat_collection: &StatCollection) -> f32 {
        match self {
            StatInstance::Attribute(attr) => {
                let val = attr.value.evaluate(stat_collection);
                val
            }
            StatInstance::Resource(resource) => {
                resource.current
            }
            StatInstance::Intermediate(intermediate) => {
                intermediate.get_total()
            }
        }
    }
}

#[derive(Component, Debug, Default, Clone, DerefMut, Deref)]
pub struct StatCollection {
    #[deref]
    pub stats: HashMap<ValueTag, StatInstance>,
    
    pub stat_dependencies: HashMap<ValueTag, HashSet<ValueTag>>,
    pub stat_dependents: HashMap<ValueTag, HashSet<ValueTag>>,
    
    pub pending_stats: HashMap<ValueTag, HashSet<ValueTag>>, // Key is attribute that is hanging, hashset is collection of missing attributes
}

impl StatCollection {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            stat_dependencies: Default::default(),
            stat_dependents: Default::default(),
            pending_stats: HashMap::new(),
        }
    }

    // Insert a attribute and update resolution order if needed
    pub fn insert(&mut self, tag: ValueTag, instance: StatInstance) {
        // If this is an expression, check if all dependencies are available
        match instance {
            StatInstance::Attribute(attribute) => {
                if let ValueType::Expression(expr) = &attribute.value {
                    let deps = expr.extract_dependencies();

                    // Identify missing dependencies
                    let missing_deps: HashSet<ValueTag> = deps
                        .iter()
                        .filter(|&dep| !self.stats.contains_key(dep))
                        .cloned()
                        .collect();


                    // If there are missing dependencies, add to pending attributes
                    if !missing_deps.is_empty() {
                        self.pending_stats.insert(tag.clone(), missing_deps);
                    }
                }

                self.stats.insert(tag, StatInstance::Attribute(attribute));

                self.resolve_pending_stats();
            }
            StatInstance::Resource(resource) => {
                self.stats.insert(tag.clone(), StatInstance::Resource(resource));
            }
            StatInstance::Intermediate(intermediate) => { 
                self.stats.insert(tag.clone(), StatInstance::Intermediate(intermediate));
            }
        }
    }

    // Get the value of a attribute by name, evaluating it if necessary
    pub fn get_str(&self, stat: &ValueTag) -> Result<f32, StatError> {
        match self.stats.get(stat) {
            Some(stat) => Ok(stat.get_value(self)),
            None => Err(StatError::NotFound(stat.stringify())),
        }
    }

    // Get the value of a attribute by name, evaluating it if necessary
    pub fn get<S: AsRef<str>>(&self, stat: S) -> Result<f32, StatError> {
        let parse_result = ValueTag::parse(stat.as_ref());
        if let Ok(parsed_tag) = parse_result {
            self.get_str(&parsed_tag)
        } else {
            Err(StatError::NotFound(stat.as_ref().to_string()))
        }
    }

    // Resolve any pending attributes when a new attribute is added
    fn resolve_pending_stats(&mut self) {
    }

    // Get a list of hanging attributes with their missing dependencies
    pub fn get_hanging_attributes(&self) -> &HashMap<ValueTag, HashSet<ValueTag>> {
        &self.pending_stats
    }

    // Add a batch of attributes at once
    pub fn batch_insert(&mut self, attributes: Vec<(ValueTag, AttributeInstance)>) {
    }
}
