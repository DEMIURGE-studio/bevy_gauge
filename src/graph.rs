use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::expr::Dependency;
use crate::attribute_id::{global_rodeo, AttributeId};
use crate::tags::TagMask;

/// A node in the dependency graph: an (Entity, AttributeId) pair.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DepNode {
    pub entity: Entity,
    pub attribute: AttributeId,
}

impl DepNode {
    pub fn new(entity: Entity, attribute: AttributeId) -> Self {
        Self { entity, attribute }
    }
}

/// The interned name of the synthetic node for a tag query on `attribute_name`.
///
/// This is the single definition of the format. The expression parser,
/// `AttributesMut`, and [`tag_query_synthetic_id`] all build the name here so
/// they can never disagree (a mismatch would make tagged queries read 0.0
/// with no error). The leading NUL keeps it out of the user name space.
pub(crate) fn tag_query_synthetic_name(attribute_name: &str, mask: TagMask) -> String {
    format!("\0tag:{}:{}", attribute_name, mask.0)
}

/// True if `name` is a synthetic tag-query node name (see
/// [`tag_query_synthetic_name`]).
pub(crate) fn is_tag_query_synthetic_name(name: &str) -> bool {
    name.starts_with("\0tag:")
}

/// Compute the synthetic [`AttributeId`] for a tag query on `attribute`.
pub(crate) fn tag_query_synthetic_id(attribute: AttributeId, mask: TagMask) -> AttributeId {
    let rodeo = global_rodeo();
    let name = rodeo.resolve(&attribute.0);
    let synthetic_name = tag_query_synthetic_name(name, mask);
    AttributeId(rodeo.get_or_intern(&synthetic_name))
}

/// Tracks which attributes on an entity use a particular alias in their expressions.
/// When an alias is re-pointed, we use this to know which attributes need rewiring.
///
/// Usage is refcounted per (dependent attribute, source attribute) pair: two
/// modifiers on the same attribute may both reference the same source attribute,
/// and removing one must not sever the other's rewiring.
#[derive(Clone, Debug, Default)]
struct AliasUsage {
    /// Key: dependent attribute on the entity. Value: source attributes it
    /// depends on via this alias, with a count per source attribute.
    attribute_deps: HashMap<AttributeId, HashMap<AttributeId, u32>>,
}

/// Data returned by [`DependencyGraph::remove_entity`] so the caller can
/// re-evaluate attributes that referenced the removed entity.
#[derive(Debug, Default)]
pub struct EntityCleanup {
    /// Nodes on *other* entities that had a dependency edge from the removed
    /// entity. These should be re-evaluated (their source values are gone).
    pub dependents: Vec<DepNode>,
    /// `(owner, alias)` pairs on other entities whose alias pointed at the
    /// removed entity. The alias registration is removed; usage records are
    /// kept so re-pointing the alias later still rewires correctly.
    pub dangling_aliases: Vec<(Entity, AttributeId)>,
}

/// Global dependency graph tracking all attribute-to-attribute edges and cross-entity aliases.
///
/// This is a Bevy Resource. It tracks:
/// - **Dependency edges**: both local (within one entity) and cross-entity.
///   Edges are refcounted: each modifier that contributes a dependency bumps
///   the count, so removing one of two modifiers that share an edge keeps the
///   edge alive for the survivor.
/// - **Aliases**: which entity an alias on a given entity points to.
/// - **Alias usage**: which attributes on an entity reference which aliases
///   (so we can rewire edges when an alias changes).
///
/// When a attribute changes, dependents are found via this graph and re-evaluated.
/// When an alias is re-pointed, edges are automatically rewired.
#[derive(Resource, Default, Debug)]
pub struct DependencyGraph {
    /// Forward edges: when `source` changes, re-evaluate all keys of the inner
    /// map. The value is the edge's refcount.
    forward: HashMap<DepNode, HashMap<DepNode, u32>>,
    /// Reverse edges: for efficient cleanup and in-degree computation.
    reverse: HashMap<DepNode, HashMap<DepNode, u32>>,
    /// Alias registry: (entity, alias_id) -> source_entity.
    aliases: HashMap<(Entity, AttributeId), Entity>,
    /// Alias usage: (entity, alias_id) -> which local attributes depend on which
    /// source attributes via this alias.
    alias_usage: HashMap<(Entity, AttributeId), AliasUsage>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Edge operations
    // -----------------------------------------------------------------------

    /// Register a dependency edge: `dependent` depends on `source`.
    /// Adding the same edge again increments its refcount.
    pub fn add_edge(&mut self, source: DepNode, dependent: DepNode) {
        self.add_edge_count(source, dependent, 1);
    }

    fn add_edge_count(&mut self, source: DepNode, dependent: DepNode, count: u32) {
        if count == 0 {
            return;
        }
        *self.forward.entry(source).or_default().entry(dependent).or_insert(0) += count;
        *self.reverse.entry(dependent).or_default().entry(source).or_insert(0) += count;
    }

    /// Remove one contribution to a dependency edge. The edge disappears only
    /// when its refcount reaches zero.
    pub fn remove_edge(&mut self, source: DepNode, dependent: DepNode) {
        self.remove_edge_count(source, dependent, 1);
    }

    fn remove_edge_count(&mut self, source: DepNode, dependent: DepNode, count: u32) {
        Self::decrement(&mut self.forward, source, dependent, count);
        Self::decrement(&mut self.reverse, dependent, source, count);
    }

    fn decrement(
        map: &mut HashMap<DepNode, HashMap<DepNode, u32>>,
        key: DepNode,
        target: DepNode,
        count: u32,
    ) {
        if let Some(inner) = map.get_mut(&key) {
            if let Some(c) = inner.get_mut(&target) {
                *c = c.saturating_sub(count);
                if *c == 0 {
                    inner.remove(&target);
                }
            }
            if inner.is_empty() {
                map.remove(&key);
            }
        }
    }

    /// Iterate over all dependents of a source node.
    pub fn dependents(&self, source: DepNode) -> impl Iterator<Item = DepNode> + '_ {
        self.forward
            .get(&source)
            .into_iter()
            .flat_map(|m| m.keys().copied())
    }

    /// Iterate over all sources that a dependent node depends on.
    pub fn sources_of(&self, dependent: DepNode) -> impl Iterator<Item = DepNode> + '_ {
        self.reverse
            .get(&dependent)
            .into_iter()
            .flat_map(|m| m.keys().copied())
    }

    /// Remove all edges where a specific (entity, attribute) is a dependent,
    /// regardless of refcount.
    pub fn remove_dependent(&mut self, dependent: DepNode) {
        if let Some(sources) = self.reverse.remove(&dependent) {
            for (src, _) in sources {
                if let Some(fwd) = self.forward.get_mut(&src) {
                    fwd.remove(&dependent);
                    if fwd.is_empty() {
                        self.forward.remove(&src);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Alias operations
    // -----------------------------------------------------------------------

    /// Look up which entity an alias on a given entity points to.
    pub fn resolve_alias(&self, entity: Entity, alias: AttributeId) -> Option<Entity> {
        self.aliases.get(&(entity, alias)).copied()
    }

    /// Register or re-point a cross-entity source alias.
    ///
    /// Returns the list of local attributes on `entity` that need re-evaluation
    /// because their source entity changed.
    ///
    /// If an old source existed, edge refcounts are transferred from the old
    /// source's nodes to the new source's nodes.
    pub fn set_alias(
        &mut self,
        entity: Entity,
        alias: AttributeId,
        new_source: Entity,
    ) -> Vec<AttributeId> {
        let key = (entity, alias);
        let old_source = self.aliases.insert(key, new_source);

        // If source didn't change, nothing to rewire
        if old_source == Some(new_source) {
            return Vec::new();
        }

        // Get the usage info: which attributes on `entity` depend on which source
        // attributes via this alias
        let usage = match self.alias_usage.get(&key) {
            Some(u) => u.clone(),
            None => return Vec::new(),
        };

        let mut affected_attributes = Vec::new();

        for (local_attribute, source_attributes) in &usage.attribute_deps {
            let dependent = DepNode::new(entity, *local_attribute);

            for (source_attribute, count) in source_attributes {
                // Transfer the full refcount from the old source to the new one
                if let Some(old_src) = old_source {
                    let old_node = DepNode::new(old_src, *source_attribute);
                    self.remove_edge_count(old_node, dependent, *count);
                }

                let new_node = DepNode::new(new_source, *source_attribute);
                self.add_edge_count(new_node, dependent, *count);
            }

            if !affected_attributes.contains(local_attribute) {
                affected_attributes.push(*local_attribute);
            }
        }

        affected_attributes
    }

    /// Remove an alias and all its associated edges.
    ///
    /// Returns the list of local attributes that need re-evaluation.
    pub fn remove_alias(&mut self, entity: Entity, alias: AttributeId) -> Vec<AttributeId> {
        let key = (entity, alias);
        let old_source = self.aliases.remove(&key);

        let usage = match self.alias_usage.remove(&key) {
            Some(u) => u,
            None => return Vec::new(),
        };

        let mut affected_attributes = Vec::new();

        if let Some(old_src) = old_source {
            for (local_attribute, source_attributes) in &usage.attribute_deps {
                let dependent = DepNode::new(entity, *local_attribute);
                for (source_attribute, count) in source_attributes {
                    let old_node = DepNode::new(old_src, *source_attribute);
                    self.remove_edge_count(old_node, dependent, *count);
                }
                if !affected_attributes.contains(local_attribute) {
                    affected_attributes.push(*local_attribute);
                }
            }
        }

        affected_attributes
    }

    /// Record that a attribute on an entity uses a particular alias to reference
    /// a specific source attribute. Called when an expression modifier is added.
    /// Refcounted: call once per modifier occurrence.
    pub fn record_alias_usage(
        &mut self,
        entity: Entity,
        alias: AttributeId,
        local_attribute: AttributeId,
        source_attribute: AttributeId,
    ) {
        let usage = self
            .alias_usage
            .entry((entity, alias))
            .or_default();
        *usage
            .attribute_deps
            .entry(local_attribute)
            .or_default()
            .entry(source_attribute)
            .or_insert(0) += 1;
    }

    /// Remove one usage record. Called when an expression modifier is removed.
    pub fn remove_alias_usage(
        &mut self,
        entity: Entity,
        alias: AttributeId,
        local_attribute: AttributeId,
        source_attribute: AttributeId,
    ) {
        let key = (entity, alias);
        if let Some(usage) = self.alias_usage.get_mut(&key) {
            if let Some(deps) = usage.attribute_deps.get_mut(&local_attribute) {
                if let Some(c) = deps.get_mut(&source_attribute) {
                    *c = c.saturating_sub(1);
                    if *c == 0 {
                        deps.remove(&source_attribute);
                    }
                }
                if deps.is_empty() {
                    usage.attribute_deps.remove(&local_attribute);
                }
            }
            if usage.attribute_deps.is_empty() {
                self.alias_usage.remove(&key);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Entity cleanup
    // -----------------------------------------------------------------------

    /// Remove ALL data involving an entity: edges, aliases, alias usage.
    /// Called when an entity is despawned.
    ///
    /// Returns the external dependents and dangling aliases so the caller can
    /// re-evaluate attributes that were reading from the removed entity.
    pub fn remove_entity(&mut self, entity: Entity) -> EntityCleanup {
        let mut external: HashSet<DepNode> = HashSet::new();

        // Remove forward edges where this entity is the source
        let forward_keys: Vec<DepNode> = self
            .forward
            .keys()
            .filter(|k| k.entity == entity)
            .copied()
            .collect();

        for source in &forward_keys {
            if let Some(dependents) = self.forward.remove(source) {
                for (dep, _) in dependents {
                    if dep.entity != entity {
                        external.insert(dep);
                    }
                    if let Some(rev) = self.reverse.get_mut(&dep) {
                        rev.remove(source);
                        if rev.is_empty() {
                            self.reverse.remove(&dep);
                        }
                    }
                }
            }
        }

        // Remove reverse edges where this entity is the dependent
        let reverse_keys: Vec<DepNode> = self
            .reverse
            .keys()
            .filter(|k| k.entity == entity)
            .copied()
            .collect();

        for dependent in &reverse_keys {
            if let Some(sources) = self.reverse.remove(dependent) {
                for (src, _) in sources {
                    if let Some(fwd) = self.forward.get_mut(&src) {
                        fwd.remove(dependent);
                        if fwd.is_empty() {
                            self.forward.remove(&src);
                        }
                    }
                }
            }
        }

        // Remove aliases owned by this entity
        let alias_keys: Vec<(Entity, AttributeId)> = self
            .aliases
            .keys()
            .filter(|(e, _)| *e == entity)
            .copied()
            .collect();
        for key in alias_keys {
            self.aliases.remove(&key);
        }

        // Remove aliases on OTHER entities that point at this entity. Usage
        // records are kept: the owning entity's modifiers still exist, and
        // re-pointing the alias later must still rewire their edges.
        let dangling_aliases: Vec<(Entity, AttributeId)> = self
            .aliases
            .iter()
            .filter(|(_, target)| **target == entity)
            .map(|(key, _)| *key)
            .collect();
        for key in &dangling_aliases {
            self.aliases.remove(key);
        }

        // Remove alias usage owned by this entity
        let usage_keys: Vec<(Entity, AttributeId)> = self
            .alias_usage
            .keys()
            .filter(|(e, _)| *e == entity)
            .copied()
            .collect();
        for key in usage_keys {
            self.alias_usage.remove(&key);
        }

        EntityCleanup {
            dependents: external.into_iter().collect(),
            dangling_aliases,
        }
    }

    /// Check if the graph has any edges.
    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    /// Check if the graph has any aliases.
    pub fn has_aliases(&self) -> bool {
        !self.aliases.is_empty()
    }
}

/// Helper: register dependency edges for an expression's dependencies.
/// This is used by `AttributesMut` when adding expression modifiers.
pub fn register_expr_deps(
    graph: &mut DependencyGraph,
    entity: Entity,
    attribute_id: AttributeId,
    deps: &[Dependency],
) {
    let dependent = DepNode::new(entity, attribute_id);

    for dep in deps {
        match dep {
            Dependency::Local(source_attribute) => {
                let source = DepNode::new(entity, *source_attribute);
                graph.add_edge(source, dependent);
            }
            Dependency::Source { alias, attribute } => {
                graph.record_alias_usage(entity, *alias, attribute_id, *attribute);

                if let Some(source_entity) = graph.resolve_alias(entity, *alias) {
                    let source = DepNode::new(source_entity, *attribute);
                    graph.add_edge(source, dependent);
                }
            }
            Dependency::SourceTagQuery { alias, attribute, mask } => {
                // Depend on the source's synthetic tag-query node, not the
                // parent attribute: the dependent reads the synthetic's cached
                // value, so the edge must come from the node that re-evaluates
                // last (otherwise parent -> {synthetic, dependent} forms a
                // diamond and the dependent can read a stale synthetic).
                let synthetic = tag_query_synthetic_id(*attribute, *mask);
                graph.record_alias_usage(entity, *alias, attribute_id, synthetic);

                if let Some(source_entity) = graph.resolve_alias(entity, *alias) {
                    let source = DepNode::new(source_entity, synthetic);
                    graph.add_edge(source, dependent);
                }
            }
            Dependency::TagQuery { synthetic, .. } => {
                let source = DepNode::new(entity, *synthetic);
                graph.add_edge(source, dependent);
            }
        }
    }
}

/// Helper: unregister dependency edges for an expression's dependencies.
pub fn unregister_expr_deps(
    graph: &mut DependencyGraph,
    entity: Entity,
    attribute_id: AttributeId,
    deps: &[Dependency],
) {
    let dependent = DepNode::new(entity, attribute_id);

    for dep in deps {
        match dep {
            Dependency::Local(source_attribute) => {
                let source = DepNode::new(entity, *source_attribute);
                graph.remove_edge(source, dependent);
            }
            Dependency::Source { alias, attribute } => {
                graph.remove_alias_usage(entity, *alias, attribute_id, *attribute);

                if let Some(source_entity) = graph.resolve_alias(entity, *alias) {
                    let source = DepNode::new(source_entity, *attribute);
                    graph.remove_edge(source, dependent);
                }
            }
            Dependency::SourceTagQuery { alias, attribute, mask } => {
                let synthetic = tag_query_synthetic_id(*attribute, *mask);
                graph.remove_alias_usage(entity, *alias, attribute_id, synthetic);

                if let Some(source_entity) = graph.resolve_alias(entity, *alias) {
                    let source = DepNode::new(source_entity, synthetic);
                    graph.remove_edge(source, dependent);
                }
            }
            Dependency::TagQuery { synthetic, .. } => {
                let source = DepNode::new(entity, *synthetic);
                graph.remove_edge(source, dependent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_id::Interner;

    fn make_entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).expect("test entity")
    }

    fn deps_of(graph: &DependencyGraph, source: DepNode) -> Vec<DepNode> {
        graph.dependents(source).collect()
    }

    fn srcs_of(graph: &DependencyGraph, dependent: DepNode) -> Vec<DepNode> {
        graph.sources_of(dependent).collect()
    }

    #[test]
    fn add_and_query_edge() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e = make_entity(1);
        let strength = interner.get_or_intern("Strength");
        let health = interner.get_or_intern("Health");

        let source = DepNode::new(e, strength);
        let dependent = DepNode::new(e, health);

        graph.add_edge(source, dependent);
        assert_eq!(deps_of(&graph, source), vec![dependent]);
        assert_eq!(srcs_of(&graph, dependent), vec![source]);
    }

    #[test]
    fn remove_edge() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e = make_entity(1);
        let a = interner.get_or_intern("A");
        let b = interner.get_or_intern("B");

        let source = DepNode::new(e, a);
        let dependent = DepNode::new(e, b);

        graph.add_edge(source, dependent);
        graph.remove_edge(source, dependent);
        assert!(deps_of(&graph, source).is_empty());
        assert!(srcs_of(&graph, dependent).is_empty());
        assert!(graph.is_empty());
    }

    #[test]
    fn edges_are_refcounted() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e = make_entity(1);
        let a = interner.get_or_intern("A");
        let b = interner.get_or_intern("B");

        let source = DepNode::new(e, a);
        let dependent = DepNode::new(e, b);

        // Two modifiers contribute the same edge
        graph.add_edge(source, dependent);
        graph.add_edge(source, dependent);
        assert_eq!(deps_of(&graph, source).len(), 1);

        // Removing one contribution keeps the edge alive
        graph.remove_edge(source, dependent);
        assert_eq!(deps_of(&graph, source), vec![dependent]);

        // Removing the second contribution deletes it
        graph.remove_edge(source, dependent);
        assert!(deps_of(&graph, source).is_empty());
        assert!(graph.is_empty());
    }

    #[test]
    fn remove_entity_cleans_all_edges() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e1 = make_entity(1);
        let e2 = make_entity(2);
        let a = interner.get_or_intern("A");
        let b = interner.get_or_intern("B");

        graph.add_edge(DepNode::new(e1, a), DepNode::new(e2, b));
        graph.add_edge(DepNode::new(e2, a), DepNode::new(e2, b));

        graph.remove_entity(e2);
        assert!(deps_of(&graph, DepNode::new(e1, a)).is_empty());
        assert!(graph.is_empty());
    }

    #[test]
    fn remove_entity_returns_external_dependents() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let source_entity = make_entity(1);
        let dependent_entity = make_entity(2);
        let strength = interner.get_or_intern("Strength");
        let attack = interner.get_or_intern("AttackPower");

        // dependent_entity's AttackPower reads source_entity's Strength
        graph.add_edge(
            DepNode::new(source_entity, strength),
            DepNode::new(dependent_entity, attack),
        );

        let cleanup = graph.remove_entity(source_entity);
        assert_eq!(
            cleanup.dependents,
            vec![DepNode::new(dependent_entity, attack)]
        );
        assert!(graph.is_empty());
    }

    #[test]
    fn alias_set_and_resolve() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player = make_entity(2);
        let wielder = interner.get_or_intern("Wielder");

        graph.set_alias(sword, wielder, player);
        assert_eq!(graph.resolve_alias(sword, wielder), Some(player));
    }

    #[test]
    fn alias_rewire_on_change() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player_a = make_entity(2);
        let player_b = make_entity(3);
        let wielder = interner.get_or_intern("Wielder");
        let strength = interner.get_or_intern("Strength");
        let attack = interner.get_or_intern("AttackPower");

        // Sword's AttackPower depends on Wielder's Strength
        graph.record_alias_usage(sword, wielder, attack, strength);

        // Point alias to player_a and add edge
        graph.set_alias(sword, wielder, player_a);
        // set_alias wired: (player_a, Strength) -> (sword, AttackPower)
        assert_eq!(
            deps_of(&graph, DepNode::new(player_a, strength)),
            vec![DepNode::new(sword, attack)]
        );

        // Re-point to player_b - should rewire
        let affected = graph.set_alias(sword, wielder, player_b);
        assert!(affected.contains(&attack));
        // Old edge gone
        assert!(deps_of(&graph, DepNode::new(player_a, strength)).is_empty());
        // New edge present
        assert_eq!(
            deps_of(&graph, DepNode::new(player_b, strength)),
            vec![DepNode::new(sword, attack)]
        );
    }

    #[test]
    fn alias_rewire_transfers_refcounts() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player_a = make_entity(2);
        let player_b = make_entity(3);
        let wielder = interner.get_or_intern("Wielder");
        let strength = interner.get_or_intern("Strength");
        let attack = interner.get_or_intern("AttackPower");

        // TWO modifiers on AttackPower both read Wielder's Strength
        graph.record_alias_usage(sword, wielder, attack, strength);
        graph.record_alias_usage(sword, wielder, attack, strength);
        graph.set_alias(sword, wielder, player_a);

        // Re-point: the refcount-2 edge must move wholesale
        graph.set_alias(sword, wielder, player_b);
        assert!(deps_of(&graph, DepNode::new(player_a, strength)).is_empty());

        // Removing ONE modifier's contribution keeps the edge for the survivor
        graph.remove_alias_usage(sword, wielder, attack, strength);
        graph.remove_edge(DepNode::new(player_b, strength), DepNode::new(sword, attack));
        assert_eq!(
            deps_of(&graph, DepNode::new(player_b, strength)),
            vec![DepNode::new(sword, attack)]
        );
    }

    #[test]
    fn alias_remove_cleans_edges() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player = make_entity(2);
        let wielder = interner.get_or_intern("Wielder");
        let strength = interner.get_or_intern("Strength");
        let attack = interner.get_or_intern("AttackPower");

        graph.record_alias_usage(sword, wielder, attack, strength);
        graph.set_alias(sword, wielder, player);

        let affected = graph.remove_alias(sword, wielder);
        assert!(affected.contains(&attack));
        assert!(deps_of(&graph, DepNode::new(player, strength)).is_empty());
        assert!(graph.resolve_alias(sword, wielder).is_none());
    }

    #[test]
    fn remove_entity_cleans_aliases() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player = make_entity(2);
        let wielder = interner.get_or_intern("Wielder");

        graph.set_alias(sword, wielder, player);
        graph.remove_entity(sword);
        assert!(graph.resolve_alias(sword, wielder).is_none());
        assert!(!graph.has_aliases());
    }

    #[test]
    fn remove_entity_drops_aliases_pointing_at_it() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player = make_entity(2);
        let wielder = interner.get_or_intern("Wielder");
        let strength = interner.get_or_intern("Strength");
        let attack = interner.get_or_intern("AttackPower");

        graph.record_alias_usage(sword, wielder, attack, strength);
        graph.set_alias(sword, wielder, player);

        // Despawn the TARGET of the alias
        let cleanup = graph.remove_entity(player);
        assert_eq!(cleanup.dangling_aliases, vec![(sword, wielder)]);
        assert!(graph.resolve_alias(sword, wielder).is_none());

        // Usage records survive: re-pointing the alias rewires the edges
        let player_b = make_entity(3);
        let affected = graph.set_alias(sword, wielder, player_b);
        assert!(affected.contains(&attack));
        assert_eq!(
            deps_of(&graph, DepNode::new(player_b, strength)),
            vec![DepNode::new(sword, attack)]
        );
    }
}
