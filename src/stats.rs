use crate::modifiers::{ModifierCollectionRefs, ModifierInstance, ModifierValueTotal, AttributeUpdatedEvent, ModifierUpdatedEvent, ModifierTarget};
use std::collections::{HashMap, HashSet};
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::*;
use crate::prelude::AttributeInstance;
use crate::resource::ResourceInstance;
use crate::tags::TagRegistry;
use crate::value_type::{Expression, StatValue};

#[derive(Debug)]
pub enum StatError {
    BadOpp(String),
    NotFound(String),
}



#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AttributeId {
    pub group: String,
    pub tag: u32,
}

impl AttributeId {
    pub fn new(group: String, tag: u32) -> Self {
        Self { group, tag }
    }
}


#[derive(Component, Debug, Default, DerefMut, Deref)]
#[require(ModifierCollectionRefs)]
pub struct StatCollection {
    #[deref]
    pub attributes: HashMap<String, HashMap<u32, AttributeInstance>>, // primary group -> instance
    pub attribute_modifiers: EntityHashMap<HashSet<AttributeId>>,
    pub resources: HashMap<String, ResourceInstance>,
    pub pending_attributes: HashMap<AttributeId, HashSet<AttributeId>>,
}

impl StatCollection {
    pub fn new() -> Self {
        Self {
            attributes: HashMap::new(),
            resources: Default::default(),
            attribute_modifiers: EntityHashMap::default(),
            pending_attributes: HashMap::new(),
        }
    }

    fn mark_pending_attr_dependent(&mut self, dependent: AttributeId, missing: AttributeId) {
        // Get or create the HashSet for this missing attribute
        let dependents = self.pending_attributes
            .entry(missing)
            .or_insert_with(HashSet::new);

        // Add the dependent attribute to the set
        dependents.insert(dependent);
    }

    // Simplified resolve_pending_dependents function
    fn resolve_pending_dependents(&mut self, resolved: &AttributeId) {
        // Early return if no pending dependents
        let Some(dependents) = self.pending_attributes.remove(resolved) else {
            return;
        };

        // Get resolved attribute first (outside the loop)
        let resolved_attr_exists = self.attributes
            .get(&resolved.group)
            .and_then(|group| group.get(&resolved.tag))
            .is_some();

        // Early return if resolved attribute doesn't exist
        if !resolved_attr_exists {
            return;
        }

        // Process each dependent
        for dependent in dependents {
            // Skip if the dependent attribute doesn't exist
            let Some(group_attrs) = self.attributes.get_mut(&dependent.group) else {
                continue;
            };

            let Some(_) = group_attrs.get(&dependent.tag) else {
                continue;
            };

            // Update dependency relationship
            self.add_dependent_relationship(resolved, &dependent);
        }
    }

    // Helper to add a dependent relationship between two attributes
    fn add_dependent_relationship(&mut self, attribute_id: &AttributeId, dependent_id: &AttributeId) {
        // First, get the attribute
        if let Some(attr_group) = self.attributes.get_mut(&attribute_id.group) {
            if let Some(attribute) = attr_group.get_mut(&attribute_id.tag) {
                // Get or create the dependent_attributes map
                let attr_dependents = attribute.dependent_attributes
                    .get_or_insert_with(HashMap::new);

                // Get or create the HashSet for this dependent group
                let dependent_set = attr_dependents
                    .entry(dependent_id.group.clone())
                    .or_insert_with(HashSet::new);

                // Add the dependent tag to the set
                dependent_set.insert(dependent_id.tag);
            }
        }
    }


    // Updated add_attribute function to use the simplified structure
    pub fn add_attribute(&mut self, group: &str, attr_name: &str, value: StatValue, tag_registry: &Res<TagRegistry>) {
        let Some(bit_tag) = tag_registry.get_id(group, attr_name) else {
            panic!("Attribute group {} or tag {} is not registered", group, attr_name);
        };

        // Normalize the group name to lowercase
        let group_lowercase = group.to_lowercase();
        let this_attr_id = AttributeId::new(group_lowercase.clone(), bit_tag);

        // Create the attribute instance with default values
        let mut attribute = AttributeInstance {
            value: value.clone(),
            dependencies: None,
            dependent_attributes: None,
            modifier_collection: EntityHashMap::default(),
            dependent_modifiers: EntityHashSet::default(),
            modifier_total: ModifierValueTotal::default(),
        };

        // Process dependencies if they exist
        if let Some(dependency_list) = value.extract_dependencies() {
            // Convert dependencies of new attr to <group, HashSet<tag>>
            let mut dependencies_converted: HashMap<String, HashSet<u32>> = HashMap::new();

            for (dep_group, dep_name) in dependency_list {
                // Look up the tag ID for this dependency
                if let Some(dep_tag) = tag_registry.get_id(&dep_group, &dep_name) {
                    // Get or create HashSet for this group
                    let group_deps = dependencies_converted
                        .entry(dep_group.to_lowercase())
                        .or_insert_with(HashSet::new);

                    // Add this tag to the HashSet
                    group_deps.insert(dep_tag);
                } else {
                    // Log a warning for missing dependency tag
                    println!("Warning: Dependency tag {}.{} not found in registry", dep_group, dep_name);
                    continue;
                }
            }

            // Set the dependencies in the attribute
            attribute.dependencies = Some(dependencies_converted.clone());

            // For each dependency group
            for (dependency_group, dependency_tags) in &dependencies_converted {
                // For each tag in the group
                for &dependency_mask in dependency_tags {
                    let missing_attr_id = AttributeId::new(dependency_group.clone(), dependency_mask);

                    // If attr group doesn't exist yet or attr bit doesn't exist yet, mark as pending
                    let dependency_exists = self.attributes.get(dependency_group)
                        .and_then(|group| group.get(&dependency_mask))
                        .is_some();

                    if !dependency_exists {
                        self.mark_pending_attr_dependent(this_attr_id.clone(), missing_attr_id);
                        continue;
                    }

                    // Dependency exists, update its dependents list
                    if let Some(dependent_attribute) = self.attributes
                        .get_mut(dependency_group)
                        .and_then(|group| group.get_mut(&dependency_mask))
                    {
                        // Mark this attr as a dependent in its dependency
                        if let Some(other_attr_dependents) = &mut dependent_attribute.dependent_attributes {
                            // Get or create the HashSet for this dependent group
                            let dependent_set = other_attr_dependents
                                .entry(group_lowercase.clone())
                                .or_insert_with(HashSet::new);

                            // Add the dependent tag to the set
                            dependent_set.insert(bit_tag);
                        } else {
                            // Create a new mapping with a HashSet containing this tag
                            let mut dependent_map = HashMap::new();
                            dependent_map.insert(group_lowercase.clone(), HashSet::from([bit_tag]));
                            dependent_attribute.dependent_attributes = Some(dependent_map);
                        }
                    }
                }
            }
        }

        // Add the attribute to the collection
        self.attributes
            .entry(group_lowercase.clone())
            .or_insert_with(HashMap::new)
            .insert(bit_tag, attribute);

        // Resolve any attributes that were waiting for this one
        self.resolve_pending_dependents(&this_attr_id);
        self.recalculate_attribute_and_dependents(this_attr_id, &tag_registry)
    }
    
    
    pub fn insert_resource(&mut self, resource_name: &str,resource_instance: ResourceInstance) {
        
    }


    pub fn recalculate_attribute_and_dependents(&mut self, attribute_id: AttributeId, registry: &Res<TagRegistry>) {
        // Simple set to track which stats we've processed
        let mut processed: HashSet<AttributeId> = HashSet::new();
        // Start from the given attribute and walk outward to dependents
        self.tree_walk_calculate(attribute_id, &mut processed, &registry);
    }

    // Tree walk to recalculate attributes and their dependents
    fn tree_walk_calculate(&mut self, attr_id: AttributeId, processed: &mut HashSet<AttributeId>, tag_registry: &Res<TagRegistry>) {
        // Skip if already processed
        if processed.contains(&attr_id) {
            return;
        }

        // Mark as processed to avoid cycles
        processed.insert(attr_id.clone());

        // First collect dependents to avoid borrow issues during recursion
        let dependents = if let Some(attr_group) = self.attributes.get(&attr_id.group) {
            if let Some(attribute) = attr_group.get(&attr_id.tag) {
                if let Some(ref dependent_attrs) = attribute.dependent_attributes {
                    // Collect all dependent attributes
                    let mut all_dependents = Vec::new();
                    for (dep_group, dep_tags) in dependent_attrs {
                        for &dep_tag in dep_tags {
                            all_dependents.push(AttributeId::new(dep_group.clone(), dep_tag));
                        }
                    }
                    all_dependents
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Update this attribute's value
        self.update_dependent_attribute(attr_id, tag_registry);

        // Process dependents without holding mutable borrow
        for dependent in dependents {
            self.tree_walk_calculate(dependent, processed, tag_registry);
        }
    }

    // Get a stat value by group and tag
    pub fn get(&self, group: &str, tag: u32) -> Result<f32, StatError> {
        let group = group.to_lowercase();

        // Try to get the attribute group
        match self.attributes.get(&group) {
            Some(attr_group) => {
                // Try to get the specific attribute
                match attr_group.get(&tag) {
                    Some(attribute) => {
                        Ok(attribute.get_value_f32())
                    }
                    None => Err(StatError::NotFound(format!("{}.{}", group, tag)))
                }
            }
            None => Err(StatError::NotFound(group))
        }
    }

    // Get a stat value with bit masking
    pub fn get_with_mask(&self, group: &str, target_mask: u32) -> Result<f32, StatError> {
        let group = group.to_lowercase();

        // Try to get the attribute group
        match self.attributes.get(&group) {
            Some(attr_group) => {
                let mask_value = target_mask;
                let mut total_value = 0.0;
                let mut found = false;

                // For bit masking, we need to check each attribute that matches the mask
                for (&tag, attribute) in attr_group {
                    if tag & mask_value > 0 {
                        total_value += &attribute.get_value_f32();
                        found = true;
                    }
                }

                if found {
                    Ok(total_value)
                } else {
                    Err(StatError::NotFound(format!("{} with mask {}", group, mask_value)))
                }
            }
            None => Err(StatError::NotFound(group))
        }
    }

    // Update a dependent attribute's value
    pub fn update_dependent_attribute(&mut self, attribute_id: AttributeId, tag_registry: &TagRegistry) {
        // Two-step process to avoid borrowing issues:
        // 1. Clone the StatValue from the attribute (if it exists)
        let mut stat_value = if let Some(attr_group) = self.attributes.get(&attribute_id.group) {
            if let Some(attribute) = attr_group.get(&attribute_id.tag) {
                Some(attribute.value.clone())
            } else {
                None
            }
        } else {
            None
        };

        // 2. If we have a stat value, update it with the current stat collection
        if let Some(ref mut value) = stat_value {
            value.set_value_with_context(self, tag_registry);

            // 3. Now place the updated value back in the attribute
            if let Some(attr_group) = self.attributes.get_mut(&attribute_id.group) {
                if let Some(attribute) = attr_group.get_mut(&attribute_id.tag) {
                    attribute.value = value.clone();
                }
            }
        }
    }

    // Recalculate all stats using tree-walking
    pub fn recalculate_all(&mut self, registry: &Res<TagRegistry>) {
        // Find all attributes with no dependencies (roots of the tree)
        let mut root_attrs = Vec::new();

        for (group, attr_group) in &self.attributes {
            for (&tag, attribute) in attr_group {
                if attribute.dependencies.is_none() || attribute.dependencies.as_ref().unwrap().is_empty() {
                    root_attrs.push(AttributeId::new(group.clone(), tag));
                }
            }
        }

        // Process each root attribute first
        let mut processed = HashSet::new();
        for root_attr in root_attrs {
            self.tree_walk_calculate(root_attr, &mut processed, registry);
        }

        // Then process any remaining attributes that weren't reached
        let mut all_attrs = Vec::new();
        for (group, attr_group) in &self.attributes {
            for (&tag, _) in attr_group {
                all_attrs.push(AttributeId::new(group.clone(), tag));
            }
        }

        for attr in all_attrs {
            if !processed.contains(&attr) {
                self.tree_walk_calculate(attr, &mut processed, registry);
            }
        }
    }



    pub fn get_hanging_attributes(&self) -> &HashMap<AttributeId, HashSet<AttributeId>> {
        &self.pending_attributes
    }


    pub fn add_or_replace_modifier(&mut self, modifier: &ModifierInstance, modifier_entity: Entity, tag_registry: &Res<TagRegistry>) {
        if let Some(target_group) = self.attributes.get_mut(&modifier.target_stat.group) {
            for (key, value) in target_group {
                if key & modifier.target_stat.tag > 0 {
                    value.add_or_replace_modifier(modifier, modifier_entity);
                    self.attribute_modifiers.entry(modifier_entity).or_insert_with(HashSet::new).insert(modifier.target_stat.clone());
                }
            }
            self.recalculate_attribute_and_dependents(modifier.target_stat.clone(), tag_registry )
        }
    }

    pub fn remove_modifier(&mut self, modifier_entity: Entity, tag_registry: &Res<TagRegistry>) {
        let mut attributes_to_recalculate = Vec::new();
        if let Some(attribute_ids) = self.attribute_modifiers.get_mut(&modifier_entity) {
            for attribute_id in attribute_ids.drain() {
                if let Some(attribute_tags) = self.attributes.get_mut(&attribute_id.group) {
                    if let Some(attribute) = attribute_tags.get_mut(&attribute_id.tag) {
                        attribute.remove_modifier(modifier_entity);
                    }
                }
                attributes_to_recalculate.push(attribute_id.clone());
            }
        }
        self.attribute_modifiers.remove(&modifier_entity);
        for attribute_id in attributes_to_recalculate {
            self.recalculate_attribute_and_dependents(attribute_id, tag_registry);
        }
    }

    // Updated on_modifier_change to handle potential missing stats
    pub fn on_modifiers_change(&mut self, modifier_entity: Entity, registry: &Res<TagRegistry>) {
        let mut attributes_to_recalculate = Vec::new();
        
        if let Some(attribute_ids) = self.attribute_modifiers.get(&modifier_entity) {
            for attribute_id in attribute_ids {
                attributes_to_recalculate.push(attribute_id.clone());
            }
        }
        for attribute_id in attributes_to_recalculate {
            self.recalculate_attribute_and_dependents(attribute_id.clone(), &registry);
        }
    }
}

pub fn on_modifier_change(
    trigger: Trigger<ModifierUpdatedEvent>,
    mut stat_query: Query<&mut StatCollection, With<ModifierCollectionRefs>>,
    registry: Res<TagRegistry>
    
) {
    if let Ok(mut stats) = stat_query.get_mut(trigger.target()) {
        stats.on_modifiers_change(trigger.modifier_entity, &registry);
    }
}


fn add_stat_to_collection(
    stat_collection: &mut StatCollection,
    group: &str,
    name: &str,
    value: StatValue,
    tag_registry: &Res<TagRegistry>
) {
    stat_collection.add_attribute(
        group,
        name,
        value,
        tag_registry
    );
}
pub fn on_stat_added(
    trigger: Trigger<AttributeAddedEvent>,
    mut stat_query: Query<&mut StatCollection>,
    registry: Res<TagRegistry>,
) {
    let mut stat_collection = stat_query.get_mut(trigger.target()).unwrap();
    
    add_stat_to_collection(&mut stat_collection, &trigger.attribute_group, &trigger.attribute_name, trigger.value.clone(), &registry);
}

#[derive(Event, Debug)]
pub struct AttributeAddedEvent {
    pub attribute_name: String,
    pub attribute_group: String,
    pub value: StatValue,
}


pub fn register_stat_triggers(app: &mut App) {
    app.add_event::<AttributeUpdatedEvent>()
        .add_event::<ModifierUpdatedEvent>()
        .add_observer(on_modifier_change);
}


#[cfg(test)]
mod stat_tests {

}
