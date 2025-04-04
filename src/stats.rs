use crate::modifier_events::ModifierUpdatedEvent;
use crate::modifiers::{ModifierCollectionRefs, ModifierInstance, ModifierValueTotal};
use crate::prelude::AttributeInstance;
use crate::resource::ResourceInstance;
use crate::stat_value::StatValue;
use crate::tags::{attribute_tag_matches, TagRegistry};
use bevy::ecs::entity::hash_map::EntityHashMap;
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::prelude::*;
use rand::{thread_rng, Rng};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use crate::stat_events::AttributeShouldRecalculate;

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

    pub fn to_string(&self, tag_registry: &TagRegistry) -> String {
        let attribute_string = format!(
            "{}.{}",
            self.group,
            tag_registry.get_tag(&self.group, self.tag).unwrap()
        );
        attribute_string
    }
}

#[derive(Component, Debug, Default, DerefMut, Deref)]
#[require(ModifierCollectionRefs)]
pub struct StatCollection {
    #[deref]
    pub attributes: HashMap<String, HashMap<u32, Arc<RwLock<AttributeInstance>>>>, // primary group -> instance
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
        let dependents = self
            .pending_attributes
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
        let resolved_attr_exists = self
            .attributes
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
    fn add_dependent_relationship(
        &mut self,
        attribute_id: &AttributeId,
        dependent_id: &AttributeId,
    ) {
        // First, get the attribute
        if let Some(attr_group) = self.attributes.get_mut(&attribute_id.group) {
            if let Some(attribute) = attr_group.get_mut(&attribute_id.tag) {
                let mut attribute_write = attribute.write().unwrap();
                // Get or create the dependent_attributes map
                let attr_dependents = attribute_write
                    .dependent_attributes
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
    pub fn add_attribute(
        &mut self,
        group: &str,
        attr_name: &str,
        value: StatValue,
        tag_registry: &Res<TagRegistry>,
        commands: &mut Commands,
        entity: Entity
    ) {
        let Some(bit_tag) = tag_registry.get_id(group, attr_name) else {
            panic!(
                "Attribute group {} or tag {} is not registered",
                group, attr_name
            );
        };
        
        println!("Attribute added");

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
        
        let wrapped_attribute = Arc::new(RwLock::new(attribute));

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
                    println!(
                        "Warning: Dependency tag {}.{} not found in registry",
                        dep_group, dep_name
                    );
                    continue;
                }
            }

            // Set the dependencies in the attribute
            wrapped_attribute.write().unwrap().dependencies = Some(dependencies_converted.clone());
            // attribute.dependencies = Some(dependencies_converted.clone());

            // For each dependency group
            for (dependency_group, dependency_tags) in &dependencies_converted {
                // For each tag in the group
                for &dependency_mask in dependency_tags {
                    let missing_attr_id =
                        AttributeId::new(dependency_group.clone(), dependency_mask);

                    // If attr group doesn't exist yet or attr bit doesn't exist yet, mark as pending
                    let dependency_exists = self
                        .attributes
                        .get(dependency_group)
                        .and_then(|group| group.get(&dependency_mask))
                        .is_some();

                    if !dependency_exists {
                        self.mark_pending_attr_dependent(this_attr_id.clone(), missing_attr_id);
                        continue;
                    }

                    // Dependency exists, update its dependents list
                    if let Some(dependent_attribute) = self
                        .attributes
                        .get_mut(dependency_group)
                        .and_then(|group| group.get_mut(&dependency_mask))
                    {
                        let mut dependent_attribute_write = dependent_attribute.write().unwrap();
                        
                        // Mark this attr as a dependent in its dependency
                        if let Some(other_attr_dependents) =
                            &mut dependent_attribute_write.dependent_attributes
                        {
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
                            dependent_attribute_write.dependent_attributes = Some(dependent_map);
                        }
                    }
                }
            }
        }

        // Add the attribute to the collection
        self.attributes
            .entry(group_lowercase.clone())
            .or_insert_with(HashMap::new)
            .insert(bit_tag, wrapped_attribute);

        // Resolve any attributes that were waiting for this one
        self.resolve_pending_dependents(&this_attr_id);
        self.recalculate_attribute_and_dependents(this_attr_id, &tag_registry, commands, entity);
    }

    pub fn insert_resource(&mut self, resource_name: &str, resource_instance: ResourceInstance) {}
    
    
    pub fn recalculate_attributes(
        &mut self,
        attribute_id: &AttributeId,
        tag_registry: &Res<TagRegistry>,
        commands: &mut Commands,
        entity: Entity
    ) {
        let attributes = self.get_qualified_tags(attribute_id.clone());
        
        for attribute in attributes {
            self.recalculate_attribute(&attribute, tag_registry, commands, entity);
        }
    }
    
    pub fn recalculate_attribute(
        &mut self, 
        attribute_id: &AttributeId, 
        tag_registry: &Res<TagRegistry>, 
        commands: &mut Commands,
        entity: Entity,
    ) {

        let mut dependent_attributes = Vec::new();
        let mut dependent_modifiers = Vec::new();

        if let Some(attribute_instance) = self.get_attribute_instance_mut(attribute_id.clone()) {
            let mut attribute_write = attribute_instance.write().unwrap();
            
            if let Some(dependent_attribute_list) = attribute_write.dependent_attributes.as_mut()
            {
                for (dependent_attribute_group, dependent_attribute_tags) in
                    dependent_attribute_list
                {
                    for dependent_attribute in dependent_attribute_tags.iter() {
                        dependent_attributes.push(AttributeId::new(
                            dependent_attribute_group.clone(),
                            *dependent_attribute,
                        ));
                    }
                }
            }

            println!(
                "getting dependent modifiers for {:?}",
                tag_registry.get_tag(&attribute_id.group, attribute_id.tag)
            );
            for dependent_modifier in attribute_write.dependent_modifiers.iter() {
                print!("dependent modifier loop");
                dependent_modifiers.push(*dependent_modifier);
            }
        }

        self.update_dependent_attribute(attribute_id.clone(), tag_registry, commands);

        println!(
            "{:?} {:?}",
            dependent_attributes,
            dependent_modifiers.clone()
        );

        // Process dependent modifiers and collect affected attributes
        // let mut modifier_affected_attributes = Vec::new();

        // Process dependents without holding mutable borrow
        // for dependent in dependent_attributes {
        //     self.tree_walk_calculate(
        //         dependent,
        //         processed,
        //         processed_modifiers,
        //         tag_registry,
        //         commands,
        //     );
        // }
        for attribute in dependent_attributes {
            commands.trigger_targets(AttributeShouldRecalculate { attribute_id: attribute.clone() }, entity);
        }


        // println!("trigger_update_update_dependent");
        for modifier_entity in dependent_modifiers {
            commands.trigger_targets(ModifierUpdatedEvent { new_value: None }, modifier_entity);
        }
    }

    pub fn recalculate_attribute_and_dependents(
        &mut self,
        attribute_id: AttributeId,
        registry: &Res<TagRegistry>,
        commands: &mut Commands,
        entity: Entity,
    ) {
        // Simple set to track which stats we've processed
        let mut processed: HashSet<AttributeId> = HashSet::new();
        let mut processed_modifiers: EntityHashSet = EntityHashSet::default();
        // Start from the given attribute and walk outward to dependents
        self.recalculate_attribute(&attribute_id.clone(), registry, commands, entity);
        // self.tree_walk_calculate(
        //     attribute_id,
        //     &mut processed,
        //     &mut processed_modifiers,
        //     &registry,
        //     commands,
        // );
    }

    // Tree walk to recalculate attributes and their dependents

    // fn tree_walk_calculate(
    //     &mut self,
    //     attr_id: AttributeId,
    //     processed: &mut HashSet<AttributeId>,
    //     processed_modifiers: &mut EntityHashSet,
    //     tag_registry: &Res<TagRegistry>,
    //     commands: &mut Commands,
    // ) {
    //     println!(
    //         "Walking: {:?} {:?}",
    //         attr_id,
    //         tag_registry.get_tag(&attr_id.group, attr_id.tag)
    //     );
    // 
    //     // Skip if already processed
    //     if processed.contains(&attr_id) {
    //         return;
    //     }
    // 
    //     // Mark as processed to avoid cycles
    //     processed.insert(attr_id.clone());
    // 
    //     // First collect dependents to avoid borrow issues during recursion
    //     let mut dependent_attributes = Vec::new();
    //     let mut dependent_modifiers = Vec::new();
    // 
    //     if let Some(attribute_instance) = self.get_attribute_instance_mut(attr_id.clone()) {
    //         if let Some(dependent_attribute_list) = attribute_instance.dependent_attributes.as_mut()
    //         {
    //             for (dependent_attribute_group, dependent_attribute_tags) in
    //                 dependent_attribute_list
    //             {
    //                 for dependent_attribute in dependent_attribute_tags.iter() {
    //                     dependent_attributes.push(AttributeId::new(
    //                         dependent_attribute_group.clone(),
    //                         *dependent_attribute,
    //                     ));
    //                 }
    //             }
    //         }
    // 
    //         println!(
    //             "getting dependent modifiers for {:?}",
    //             tag_registry.get_tag(&attr_id.group, attr_id.tag)
    //         );
    //         for dependent_modifier in attribute_instance.dependent_modifiers.iter() {
    //             print!("dependent modifier loop");
    //             if !processed_modifiers.contains(dependent_modifier) {
    //                 dependent_modifiers.push(*dependent_modifier);
    //             }
    //         }
    //     }
    // 
    //     self.update_dependent_attribute(attr_id.clone(), tag_registry, commands);
    // 
    //     println!(
    //         "{:?} {:?}",
    //         dependent_attributes,
    //         dependent_modifiers.clone()
    //     );
    // 
    //     // Process dependent modifiers and collect affected attributes
    //     // let mut modifier_affected_attributes = Vec::new();
    // 
    //     // Process dependents without holding mutable borrow
    //     for dependent in dependent_attributes {
    //         self.tree_walk_calculate(
    //             dependent,
    //             processed,
    //             processed_modifiers,
    //             tag_registry,
    //             commands,
    //         );
    //     }
    // 
    //     println!("Walk ended: {:?}", attr_id);
    // 
    //     // println!("trigger_update_update_dependent");
    //     for modifier_entity in dependent_modifiers {
    //         processed_modifiers.insert(modifier_entity);
    //         commands.trigger_targets(ModifierUpdatedEvent { new_value: None }, modifier_entity);
    //     }
    // }

    // Get a stat value by group and tag
    pub fn get_f32(&self, attribute_id: AttributeId) -> Result<f32, StatError> {
        // Try to get the attribute group
        match self.attributes.get(&attribute_id.group) {
            Some(attr_group) => {
                // Try to get the specific attribute
                match attr_group.get(&attribute_id.tag) {
                    Some(attribute) => {
                        let attribute_read = attribute.read().unwrap();
                        Ok(attribute_read.get_total_value_f32())
                    },
                    None => Err(StatError::NotFound(format!(
                        "{}.{}",
                        &attribute_id.group, &attribute_id.tag
                    ))),
                }
            }
            None => Err(StatError::NotFound(attribute_id.group.clone())),
        }
    }

    pub fn get_stat_value(&self, attribute_id: AttributeId) -> Option<StatValue> {
        if let Some(attribute_group) = self.attributes.get(&attribute_id.group) {
            if let Some(attribute) = attribute_group.get(&attribute_id.tag) {
                let attribute_read = attribute.read().unwrap();
                return Some(attribute_read.value.clone());
            }
            return None;
        }
        None
    }

    // fn get_stat_value_mut(&mut self, attribute_id: AttributeId) -> Option<&mut StatValue> {
    //     if let Some(attribute_group) = self.attributes.get_mut(&attribute_id.group) {
    //         if let Some(attribute) = attribute_group.get_mut(&attribute_id.tag) {
    //             return Some(&mut attribute.value);
    //         }
    //         return None;
    //     }
    //     None
    // }
    
    fn modify_stat_value<F>(&mut self, attribute_id: AttributeId, modify_fn: F)
    where 
        F: FnOnce(&mut StatValue) 
    {
        if let Some(attribute_group) = self.attributes.get_mut(&attribute_id.group) {
            if let Some(attribute) = attribute_group.get_mut(&attribute_id.tag) {
                let mut attribute_write = attribute.write().unwrap();
                modify_fn(&mut attribute_write.value);
            }
        }
    }
    

    pub fn get_attribute_instance(&self, attribute_id: AttributeId) -> Option<Arc<RwLock<AttributeInstance>>> {
        if let Some(attribute_group) = self.attributes.get(&attribute_id.group) {
            if let Some(attribute) = attribute_group.get(&attribute_id.tag) {
                return Some(Arc::clone(attribute));
            }
            return None;
        }
        None
    }
    
    
    pub fn get_qualified_tags(
        &mut self,
        attribute_id: AttributeId,
    ) -> Vec<AttributeId> {
        let mut result = Vec::new();
        if let Some(attribute_group) = self.attributes.get_mut(&attribute_id.group) {
            for (tag, attribute) in attribute_group.iter_mut() {
                if tag & attribute_id.tag > 0 {
                    result.push(AttributeId { tag: *tag, group: attribute_id.group.clone() });
                }
            }
        }
        result
    }

    pub fn get_attribute_instance_mut(
        &mut self,
        attribute_id: AttributeId,
    ) -> Option<Arc<RwLock<AttributeInstance>>>  {
        if let Some(attribute_group) = self.attributes.get_mut(&attribute_id.group) {
            if let Some(attribute) = attribute_group.get_mut(&attribute_id.tag) {
                return Some(Arc::clone(attribute));
            }
            return None;
        }
        None
    }

    pub fn get_stat_relevant_context(
        &self,
        attribute_group_tag: &[(String, String)],
        tag_registry: &TagRegistry,
    ) -> HashMap<String, f32> {
        let mut stat_relevant_context = HashMap::new();
        for (group, tag) in attribute_group_tag {
            let attribute_id = AttributeId {
                group: group.clone(),
                tag: tag_registry.get_id(group, tag).unwrap(),
            };
            stat_relevant_context.insert(
                format!("{}.{}", group.clone(), tag.clone()),
                self.get_f32(attribute_id).unwrap(),
            );
            // for (group, tags) in self.get_attribute_instance(attribute_id.clone()).unwrap().dependencies.clone().unwrap().iter() {
            //     for stat in tags {
            //         stat_relevant_context.insert(attribute_id.to_string(tag_registry), self.get_f32(AttributeId {group: group.clone(), tag: *stat}).unwrap());
            //     }
            // }
        }
        stat_relevant_context
    }

    // Update a dependent attribute's value
    pub fn update_dependent_attribute(
        &mut self,
        attribute_id: AttributeId,
        tag_registry: &TagRegistry,
        commands: &mut Commands,
    ) {
        println!("updating dependent: {:?}", attribute_id);
        // Two-step process to avoid borrowing issues:
        // 1. Clone the StatValue from the attribute (if it exists)

        let mut dependency_strings: Vec<(String, String)> = Vec::new();

        if let Some(attribute) = self.get_attribute_instance(attribute_id.clone()) {
            let attribute_read = attribute.read().unwrap();
            if let Some(dependencies) = attribute_read.value.extract_dependencies() {
                for (dependency_group, dependency_tag) in dependencies {
                    dependency_strings.push((dependency_group, dependency_tag));
                }
            }
        }

        let stat_snapshot = self
            .get_stat_relevant_context(&dependency_strings, tag_registry)
            .clone();

        // 2. If we have a stat value, update it with the current stat collection
        if let Some(attribute) = self.get_attribute_instance(attribute_id.clone()) {
            let mut attribute_write = attribute.write().unwrap();
            println!("current value: {:?}", attribute_write.value.get_value_f32());
            attribute_write.value.update_value_with_context(&stat_snapshot);
            println!("current value: {:?}", attribute_write.value.get_value_f32());

            // Trigger updates for dependent modifiers
            let dependent_modifiers = attribute_write.dependent_modifiers.clone();
            for modifier in &dependent_modifiers {
                commands.trigger_targets(ModifierUpdatedEvent { new_value: None }, *modifier);
            }
        }

        println!("end update dependent: {:?}", attribute_id);
    }

    pub fn get_hanging_attributes(&self) -> &HashMap<AttributeId, HashSet<AttributeId>> {
        &self.pending_attributes
    }

    pub fn add_or_replace_modifier(
        &mut self,
        modifier: &ModifierInstance,
        modifier_entity: Entity,
        tag_registry: &Res<TagRegistry>,
        commands: &mut Commands,
        entity: Entity
    ) {
        let mut modifier_deps = Vec::new();

        if let Some(target_group) = self.attributes.get_mut(&modifier.target_stat.group) {
            for (key, value) in target_group {
                if key & modifier.target_stat.tag > 0 {
                    let mut attribute_write = value.write().unwrap();
                    attribute_write.add_or_replace_modifier(modifier, modifier_entity);
                    self.attribute_modifiers
                        .entry(modifier_entity)
                        .or_insert_with(HashSet::new)
                        .insert(modifier.target_stat.clone());
                    for dep in &modifier.dependencies {
                        modifier_deps.push(dep.clone());
                    }
                }
            }
        }

        for modifier_dep in &modifier_deps {
            if let Some(attribute_instance) = self.get_attribute_instance_mut(modifier_dep.clone())
            {
                let mut attribute_write = attribute_instance.write().unwrap();
                
                attribute_write
                    .dependent_modifiers
                    .insert(modifier_entity);
            }
        }

        for dependency in &modifier.dependencies {
            self.recalculate_attribute_and_dependents(dependency.clone(), tag_registry, commands, entity);
        }

        self.recalculate_attribute_and_dependents(
            modifier.target_stat.clone(),
            tag_registry,
            commands,
            entity
        )
    }

    pub fn remove_modifier(
        &mut self,
        modifier_entity: Entity,
        tag_registry: &Res<TagRegistry>,
        commands: &mut Commands,
        entity: Entity
    ) {
        println!("remove_modifier: {:?}", modifier_entity);
        let mut attributes_to_recalculate = Vec::new();
        if let Some(attribute_ids) = self.attribute_modifiers.get_mut(&modifier_entity) {
            for attribute_id in attribute_ids.iter() {
                if let Some(attribute_tags) = self.attributes.get_mut(&attribute_id.group) {
                    if let Some(attribute) = attribute_tags.get_mut(&attribute_id.tag) {
                        let mut attribute_write = attribute.write().unwrap();
                        
                        attribute_write.remove_modifier(modifier_entity);
                    }
                }
                attributes_to_recalculate.push(attribute_id.clone());
            }
        }
        self.attribute_modifiers.remove(&modifier_entity);
        for attribute_id in attributes_to_recalculate {
            self.recalculate_attribute_and_dependents(attribute_id, tag_registry, commands, entity);
        }
    }

    // Updated on_modifier_change to handle potential missing stats
    pub fn update_modifier(
        &mut self,
        modifier_entity: Entity,
        registry: &Res<TagRegistry>,
        commands: &mut Commands,
        entity: Entity
    ) {
        let mut attributes_to_recalculate = Vec::new();

        if let Some(attribute_ids) = self.attribute_modifiers.get(&modifier_entity) {
            for attribute_id in attribute_ids {
                attributes_to_recalculate.push(attribute_id.clone());
            }
        }
        for attribute_id in attributes_to_recalculate {
            self.recalculate_attribute_and_dependents(attribute_id.clone(), &registry, commands, entity);
        }
    }
    
}


#[cfg(test)]
mod stat_tests {}
