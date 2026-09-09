use std::collections::{HashMap, HashSet};

use bevy::ecs::query::QueryFilter;
use bevy::ecs::system::{SystemParam, SystemState};
use bevy::prelude::*;

use crate::attributes::Attributes;
use crate::expr::{Dependency, Expr};
use crate::graph::{register_expr_deps, unregister_expr_deps, DepNode, DependencyGraph};
use crate::modifier::Modifier;
use crate::node::ReduceFn;
use crate::attribute_id::{global_rodeo, AttributeId};
use crate::tags::{TagMask, TagResolver};

/// System parameter for mutating entity attributes.
///
/// All writes to the attribute system go through `AttributesMut`. This ensures
/// that dependency edges are maintained and changes propagate correctly
/// through the global `DependencyGraph`.
///
/// Reading attributes does NOT require this - use `&Attributes` directly.
///
/// # Query filters
///
/// `F` narrows which entities this parameter can touch, which is useful for
/// avoiding access conflicts with another `Attributes` query in the same
/// system. Propagation does not stop at the filter: when a change needs to
/// reach an entity the filter hides, that entity's re-evaluation is queued as
/// a command and runs with an unfiltered `AttributesMut` at the next command
/// flush. Within the filtered system itself, values on hidden entities are
/// left as they were (never zeroed).
#[derive(SystemParam)]
pub struct AttributesMut<'w, 's, F: QueryFilter + 'static = ()> {
    query: Query<'w, 's, &'static mut Attributes, F>,
    /// Unfiltered membership check, so "hidden by `F`" can be told apart from
    /// "has no `Attributes`" without conflicting with `query`.
    with_attributes: Query<'w, 's, (), With<Attributes>>,
    graph: ResMut<'w, DependencyGraph>,
    tag_resolver: Res<'w, TagResolver>,
    commands: Commands<'w, 's>,
}

impl<'w, 's, F: QueryFilter> AttributesMut<'w, 's, F> {
    /// Get a reference to the tag resolver.
    pub fn tag_resolver(&self) -> &TagResolver {
        &self.tag_resolver
    }

    fn intern(&self, s: &str) -> AttributeId {
        AttributeId(global_rodeo().get_or_intern(s))
    }

    fn try_intern(&self, s: &str) -> Option<AttributeId> {
        global_rodeo().get(s).map(AttributeId)
    }

    fn resolve_id(&self, id: AttributeId) -> &str {
        global_rodeo().resolve(&id.0)
    }

    pub fn value(&self, entity: Entity, attribute: &str) -> f32 {
        self.query.get(entity).ok().map(|a| a.value(attribute)).unwrap_or(0.0)
    }

    /// Get read-only access to an entity's [`Attributes`].
    ///
    /// Useful when you need to inspect attribute values through `AttributesMut`
    /// without a separate `&Attributes` query (which would conflict).
    pub fn get_attributes(&self, entity: Entity) -> Option<&Attributes> {
        self.query.get(entity).ok()
    }

    // -----------------------------------------------------------------------
    // Core modifier operations
    // -----------------------------------------------------------------------

    /// Add a modifier to a attribute on an entity (untagged - applies to every tag query).
    ///
    /// The attribute node is created with `ReduceFn::Sum` if it doesn't exist.
    /// If the modifier is an `Expr`, dependency edges are registered in the
    /// global graph. The attribute is then re-evaluated and changes propagate.
    pub fn add_modifier(
        &mut self,
        entity: Entity,
        attribute: &str,
        modifier: impl Into<Modifier>,
    ) {
        self.add_modifier_tagged(entity, attribute, modifier, TagMask::NONE);
    }

    /// Add a tagged modifier to a attribute on an entity.
    ///
    /// The modifier will only participate in tag queries whose bits are a
    /// superset of `tag`. A `TagMask::NONE` tag makes the modifier global
    /// (equivalent to [`add_modifier`](Self::add_modifier)).
    pub fn add_modifier_tagged(
        &mut self,
        entity: Entity,
        attribute: &str,
        modifier: impl Into<Modifier>,
        tag: TagMask,
    ) {
        let modifier = modifier.into();
        let attribute_id = self.intern(attribute);

        // Add the modifier to the node first. If the entity has no
        // `Attributes` component, bail out before touching the graph so we
        // don't leave orphaned edges behind.
        if let Ok(mut attrs) = self.query.get_mut(entity) {
            let node = attrs.ensure_node(attribute_id, ReduceFn::Sum);
            node.add_tagged_modifier(modifier.clone(), tag);
        } else {
            return;
        }

        // Register dependencies if this is an expression modifier
        if let Modifier::Expr(expr) = &modifier {
            // Ensure any tag-query dependencies are materialized before
            // registering edges (so the synthetic nodes exist in the graph).
            for dep in expr.dependencies() {
                if let Dependency::TagQuery { attribute, mask, .. } = dep {
                    self.ensure_tag_query(entity, *attribute, *mask);
                }
            }
            register_expr_deps(&mut self.graph, entity, attribute_id, expr.dependencies());
        }

        // Cache source values for any cross-entity refs, then evaluate
        self.cache_source_values(entity, attribute_id);
        self.evaluate_and_propagate(entity, attribute_id);
    }

    /// Add a modifier to a attribute that uses a specific reduce function.
    pub fn add_modifier_with_reduce(
        &mut self,
        entity: Entity,
        attribute: &str,
        modifier: impl Into<Modifier>,
        reduce: ReduceFn,
    ) {
        self.add_modifier_tagged_with_reduce(entity, attribute, modifier, TagMask::NONE, reduce);
    }

    /// Add a tagged modifier with a specific reduce function.
    pub fn add_modifier_tagged_with_reduce(
        &mut self,
        entity: Entity,
        attribute: &str,
        modifier: impl Into<Modifier>,
        tag: TagMask,
        reduce: ReduceFn,
    ) {
        let modifier = modifier.into();
        let attribute_id = self.intern(attribute);

        // Node first; bail before registering graph edges if the entity has
        // no `Attributes` component (see `add_modifier_tagged`).
        if let Ok(mut attrs) = self.query.get_mut(entity) {
            let node = attrs.ensure_node(attribute_id, reduce);
            node.add_tagged_modifier(modifier.clone(), tag);
        } else {
            return;
        }

        if let Modifier::Expr(expr) = &modifier {
            for dep in expr.dependencies() {
                if let Dependency::TagQuery { attribute, mask, .. } = dep {
                    self.ensure_tag_query(entity, *attribute, *mask);
                }
            }
            register_expr_deps(&mut self.graph, entity, attribute_id, expr.dependencies());
        }

        self.cache_source_values(entity, attribute_id);
        self.evaluate_and_propagate(entity, attribute_id);
    }

    /// Add a modifier that is an expression string. The expression is compiled
    /// and dependencies are extracted automatically.
    ///
    /// Supports `{TAG|TAG}` syntax in expressions if tags are registered
    /// in the [`TagResolver`].
    pub fn add_expr_modifier(
        &mut self,
        entity: Entity,
        attribute: &str,
        expr_source: &str,
    ) -> Result<(), crate::expr::CompileError> {
        let expr =
            Expr::compile(expr_source, Some(&self.tag_resolver))?;
        self.add_modifier(entity, attribute, Modifier::Expr(expr));
        Ok(())
    }

    /// Add a tagged expression modifier. The expression is compiled and
    /// dependencies are extracted automatically.
    ///
    /// Supports `{TAG|TAG}` syntax in expressions if tags are registered
    /// in the [`TagResolver`].
    pub fn add_expr_modifier_tagged(
        &mut self,
        entity: Entity,
        attribute: &str,
        expr_source: &str,
        tag: TagMask,
    ) -> Result<(), crate::expr::CompileError> {
        let expr =
            Expr::compile(expr_source, Some(&self.tag_resolver))?;
        self.add_modifier_tagged(entity, attribute, Modifier::Expr(expr), tag);
        Ok(())
    }

    /// Remove a modifier from a attribute on an entity (matches by value, ignores tags).
    pub fn remove_modifier(
        &mut self,
        entity: Entity,
        attribute: &str,
        modifier: &Modifier,
    ) {
        let attribute_id = self.intern(attribute);

        let removed = if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs
                .nodes
                .get_mut(&attribute_id)
                .map(|node| node.remove_modifier(modifier))
                .unwrap_or(false)
        } else {
            false
        };

        // Only unregister edges if a modifier was actually removed - edge
        // refcounts must stay in sync with the modifiers that contributed them.
        if removed {
            if let Modifier::Expr(expr) = modifier {
                unregister_expr_deps(&mut self.graph, entity, attribute_id, expr.dependencies());
            }
        }

        self.evaluate_and_propagate(entity, attribute_id);
    }

    /// Remove a tagged modifier (matches by both value and tag).
    pub fn remove_modifier_tagged(
        &mut self,
        entity: Entity,
        attribute: &str,
        modifier: &Modifier,
        tag: TagMask,
    ) {
        let attribute_id = self.intern(attribute);

        let removed = if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs
                .nodes
                .get_mut(&attribute_id)
                .map(|node| node.remove_tagged_modifier(modifier, tag))
                .unwrap_or(false)
        } else {
            false
        };

        if removed {
            if let Modifier::Expr(expr) = modifier {
                unregister_expr_deps(&mut self.graph, entity, attribute_id, expr.dependencies());
            }
        }

        self.evaluate_and_propagate(entity, attribute_id);
    }

    /// Set a attribute's value directly by adding a flat modifier (untagged).
    pub fn set(&mut self, entity: Entity, attribute: &str, value: f32) {
        self.add_modifier(entity, attribute, Modifier::Flat(value));
    }

    /// Set a tagged attribute value directly by adding a flat tagged modifier.
    pub fn set_tagged(&mut self, entity: Entity, attribute: &str, value: f32, tag: TagMask) {
        self.add_modifier_tagged(entity, attribute, Modifier::Flat(value), tag);
    }

    /// Replace all untagged flat modifiers on a attribute with a single value.
    ///
    /// Expression modifiers and tagged modifiers are preserved. This is useful
    /// for attributes whose "base" value changes over time (e.g., current health,
    /// resource pools, simulation state that accumulates deltas each tick).
    ///
    /// If the attribute node does not exist, it is created with `ReduceFn::Sum`.
    pub fn set_base(&mut self, entity: Entity, attribute: &str, value: f32) {
        let attribute_id = self.intern(attribute);

        if let Ok(mut attrs) = self.query.get_mut(entity) {
            let node = attrs.ensure_node(attribute_id, ReduceFn::Sum);
            node.set_flat_base(TagMask::NONE, value);
        }

        self.evaluate_and_propagate(entity, attribute_id);
    }

    /// Replace all flat modifiers with a specific tag on an attribute.
    ///
    /// Like [`set_base`](Self::set_base), but targets modifiers with an exact
    /// tag match instead of untagged modifiers. Expression modifiers and
    /// modifiers with different tags are preserved.
    pub fn set_base_tagged(
        &mut self,
        entity: Entity,
        attribute: &str,
        value: f32,
        tag: TagMask,
    ) {
        if tag.is_empty() {
            return self.set_base(entity, attribute, value);
        }

        let attribute_id = self.intern(attribute);

        if let Ok(mut attrs) = self.query.get_mut(entity) {
            let node = attrs.ensure_node(attribute_id, ReduceFn::Sum);
            node.set_flat_base(tag, value);
        }

        self.evaluate_and_propagate(entity, attribute_id);
    }

    // -----------------------------------------------------------------------
    // Gauge-style convenience constructors
    // -----------------------------------------------------------------------

    /// Create a **flat attribute** - a single value with no complex modification
    /// rules.
    ///
    /// This is the simplest attribute type: a Sum-reducing node with one flat
    /// modifier. Equivalent to gauge's `Flat` attribute type.
    ///
    /// ```ignore
    /// attributes.flat_attribute(entity, "Health", 100.0);
    /// // Later:
    /// attributes.add_modifier(entity, "Health", 20.0); // now 120
    /// ```
    pub fn flat_attribute(&mut self, entity: Entity, name: &str, value: f32) {
        self.add_modifier(entity, name, value);
    }

    /// Create a **complex attribute** composed of named parts combined via an
    /// expression.
    ///
    /// Each part becomes its own attribute node (`"{name}.{part}"`) that can
    /// receive modifiers independently. A total expression on `"{name}"`
    /// combines the parts. Short part names in the expression are automatically
    /// qualified with the parent name (e.g., `base` → `Damage.base`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// attributes.complex_attribute(
    ///     entity,
    ///     "Damage",
    ///     &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum), ("more", ReduceFn::Product)],
    ///     "base * (1 + increased) * more",
    /// )?;
    /// ```
    pub fn complex_attribute(
        &mut self,
        entity: Entity,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expression: &str,
    ) -> Result<(), crate::expr::CompileError> {
        let part_names: Vec<&str> = parts.iter().map(|(n, _)| *n).collect();

        for (part_name, reduce) in parts {
            let attribute_name = format!("{}.{}", name, part_name);
            let attribute_id = self.intern(&attribute_name);
            if let Ok(mut attrs) = self.query.get_mut(entity) {
                attrs.ensure_node(attribute_id, reduce.clone());
                attrs.evaluate_and_cache(attribute_id);
            }
        }

        let qualified = qualify_expression(name, &part_names, expression, None);
        self.add_expr_modifier(entity, name, &qualified)
    }

    /// Create a **tagged attribute** - a complex attribute with tag-filtered
    /// evaluation that materializes lazily.
    ///
    /// Each part becomes its own attribute node. When
    /// [`evaluate_tagged`](Self::evaluate_tagged) is called for a new tag
    /// combo, the template auto-generates a tagged expression modifier with
    /// `{TAG|TAG}` syntax. No need to enumerate combos up front.
    ///
    /// # Example
    ///
    /// ```ignore
    /// attributes.tagged_attribute(
    ///     entity,
    ///     "Damage",
    ///     &[("added", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
    ///     "added * (1 + increased)",
    /// )?;
    ///
    /// attributes.add_modifier_tagged(entity, "Damage.added", 25.0, PHYSICAL | MELEE);
    /// ```
    pub fn tagged_attribute(
        &mut self,
        entity: Entity,
        name: &str,
        parts: &[(&str, ReduceFn)],
        expression: &str,
    ) -> Result<(), crate::expr::CompileError> {
        // Validate the template now, before touching the entity. Combos are
        // materialized lazily, and a template that fails to compile then
        // would only show up as an error log and a permanently-zero value.
        let part_names: Vec<&str> = parts.iter().map(|(n, _)| *n).collect();
        let untagged = qualify_expression(name, &part_names, expression, None);
        Expr::compile(&untagged, Some(&self.tag_resolver))?;

        for (part_name, reduce) in parts {
            let attribute_name = format!("{}.{}", name, part_name);
            let attribute_id = self.intern(&attribute_name);
            if let Ok(mut attrs) = self.query.get_mut(entity) {
                attrs.ensure_node(attribute_id, reduce.clone());
                attrs.evaluate_and_cache(attribute_id);
            }
        }

        let parent_id = self.intern(name);
        if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs.ensure_node(parent_id, ReduceFn::Sum);
        }

        let template = crate::attributes::AttributeTemplate {
            expression: expression.to_string(),
            parts: parts.iter().map(|(n, _)| n.to_string()).collect(),
            name: name.to_string(),
            materialized: std::collections::HashSet::new(),
        };
        if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs.templates.insert(parent_id, template);
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Cross-entity sources (aliases)
    // -----------------------------------------------------------------------

    /// Register or re-point a cross-entity source alias.
    ///
    /// After this call, expressions on `entity` can reference attributes from
    /// `source_entity` via the `AttributeName@alias` syntax.
    ///
    /// If the alias was already pointing to a different entity, edges are
    /// automatically rewired and affected attributes are re-evaluated.
    pub fn register_source(
        &mut self,
        entity: Entity,
        alias: &str,
        source_entity: Entity,
    ) {
        let alias_id = self.intern(alias);

        // Rewire edges and get affected attributes
        let affected = self.graph.set_alias(entity, alias_id, source_entity);

        // Cache source values for affected attributes and re-evaluate
        for attribute_id in &affected {
            self.cache_source_values(entity, *attribute_id);
        }
        for attribute_id in affected {
            self.evaluate_and_propagate(entity, attribute_id);
        }
    }

    /// Unregister a source alias and clean up all associated edges.
    ///
    /// Attributes that referenced this alias will re-evaluate to 0.0 for those
    /// source values (the cache entries are cleared).
    pub fn unregister_source(&mut self, entity: Entity, alias: &str) {
        let alias_id = self.intern(alias);

        // Clear cached source values for attributes that used this alias
        self.clear_source_cache(entity, alias_id);

        // Remove alias and get affected attributes
        let affected = self.graph.remove_alias(entity, alias_id);

        for attribute_id in affected {
            self.evaluate_and_propagate(entity, attribute_id);
        }
    }

    /// Look up which entity an alias on a given entity currently points to.
    pub fn resolve_source(&self, entity: Entity, alias: &str) -> Option<Entity> {
        let alias_id = self.intern(alias);
        self.graph.resolve_alias(entity, alias_id)
    }

    // -----------------------------------------------------------------------
    // Evaluation
    // -----------------------------------------------------------------------

    /// Force re-evaluation of a attribute and return its value.
    pub fn evaluate(&mut self, entity: Entity, attribute: &str) -> f32 {
        let attribute_id = self.intern(attribute);

        if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs.evaluate_and_cache(attribute_id)
        } else {
            0.0
        }
    }

    /// Re-evaluate a known attribute by name using a read-only interner lookup.
    ///
    /// Uses [`Interner::get`](crate::attribute_id::Interner::get) instead of
    /// [`Interner::get_or_intern`](crate::attribute_id::Interner::get_or_intern), which
    /// avoids the write-lock path on the interner. Returns `None` if the
    /// attribute name hasn't been interned yet.
    pub fn try_evaluate(&mut self, entity: Entity, attribute: &str) -> Option<f32> {
        let attribute_id = self.try_intern(attribute)?;
        Some(self.evaluate_id(entity, attribute_id))
    }

    /// Re-evaluate a attribute by its pre-resolved [`AttributeId`], bypassing
    /// string lookup entirely.
    pub fn evaluate_id(&mut self, entity: Entity, attribute_id: AttributeId) -> f32 {
        if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs.evaluate_and_cache(attribute_id)
        } else {
            0.0
        }
    }

    /// Evaluate a attribute with a tag filter and return the result.
    ///
    /// This ensures a materialized tag-query node exists for the given
    /// `(attribute, mask)` pair, wires it into the dependency graph, evaluates it,
    /// and returns the cached result. Subsequent changes to the parent attribute
    /// will automatically propagate to this query node.
    ///
    /// **Lazy tagged attributes:** if the attribute was created via
    /// [`tagged_attribute`](Self::tagged_attribute) and this is the first time
    /// the given tag combo is evaluated, a tagged expression modifier is
    /// auto-generated from the stored template. No need to enumerate combos
    /// up front.
    pub fn evaluate_tagged(
        &mut self,
        entity: Entity,
        attribute: &str,
        query: TagMask,
    ) -> f32 {
        if query.is_empty() {
            return self.evaluate(entity, attribute);
        }

        let attribute_id = self.intern(attribute);

        // `ensure_tag_query` handles lazy template materialization for
        // tagged attributes created via `tagged_attribute`.
        let synthetic_id = self.ensure_tag_query(entity, attribute_id, query);

        if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs.evaluate_and_cache(synthetic_id)
        } else {
            0.0
        }
    }

    // -----------------------------------------------------------------------
    // Internal: lazy template materialization
    // -----------------------------------------------------------------------

    /// If `attribute_id` has a tagged-attribute template and `mask` hasn't been
    /// materialized yet, generate the combo's expression and attach it to the
    /// combo's **own synthetic node**.
    ///
    /// Each materialized combo is an independent node evaluating its own
    /// expression (e.g. `Damage.added{FIRE|SWORD} * ...`). Combos are NOT
    /// stored as tagged modifiers on the parent: under subset matching, a
    /// query for `FIRE|SWORD` would also match a previously-materialized
    /// `FIRE` combo modifier and double-count it.
    fn maybe_materialize_template(
        &mut self,
        entity: Entity,
        attribute_id: AttributeId,
        mask: TagMask,
    ) {
        // Check if there's a template and whether this combo is new
        let template_info: Option<(String, Vec<String>, String)> = self
            .query
            .get(entity)
            .ok()
            .and_then(|attrs| {
                let tmpl = attrs.templates.get(&attribute_id)?;
                if tmpl.materialized.contains(&mask) {
                    return None; // already done
                }
                Some((
                    tmpl.expression.clone(),
                    tmpl.parts.clone(),
                    tmpl.name.clone(),
                ))
            });

        let Some((expression, parts, name)) = template_info else {
            return;
        };

        // Build the tag suffix (e.g., "{FIRE|MELEE}")
        let Some(tag_suffix) = self.tag_resolver.tag_suffix(mask) else {
            bevy::log::error!(
                "bevy_gauge: cannot materialize tagged attribute {name:?} for tag mask \
                 {:#b}: one or more bits have no registered single-bit tag name. \
                 The query will read 0.0.",
                mask.0,
            );
            return;
        };

        // Compute the combo's synthetic id (same naming as ensure_tag_query).
        let synthetic_name = format!("\0tag:{}:{}", name, mask.0);
        let synthetic_id = self.intern(&synthetic_name);

        if let Ok(mut attrs) = self.query.get_mut(entity) {
            // If a filtered-view tag query was already registered for this
            // combo (e.g. an expression used `Damage{FIRE}` before the
            // template existed), convert it into a normal node: drop the
            // filtered-view mapping and its parent -> synthetic edge.
            let was_filtered_view = attrs.tag_queries.remove(&synthetic_id).is_some();

            attrs.register_template_query(attribute_id, mask, synthetic_id);

            // Mark materialized BEFORE compiling the expression, so re-entry
            // through ensure_tag_query during compilation is a no-op.
            if let Some(tmpl) = attrs.templates.get_mut(&attribute_id) {
                tmpl.materialized.insert(mask);
            }

            if was_filtered_view {
                self.graph.remove_edge(
                    DepNode::new(entity, attribute_id),
                    DepNode::new(entity, synthetic_id),
                );
            }
        } else {
            return;
        }

        // Qualify the expression with the tag suffix and attach it to the
        // synthetic node itself (compiles, registers part deps, evaluates).
        let part_strs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        let qualified = qualify_expression(&name, &part_strs, &expression, Some(&tag_suffix));
        if let Err(err) = self.add_expr_modifier(entity, &synthetic_name, &qualified) {
            bevy::log::error!(
                "bevy_gauge: failed to materialize tagged attribute {name:?} for {tag_suffix}: \
                 {err} (generated expression: {qualified:?}). The query will read 0.0.",
            );
        }
    }

    // -----------------------------------------------------------------------
    // Internal: tag query materialization
    // -----------------------------------------------------------------------

    /// Ensure a materialized tag-query node exists for (parent_attribute, mask).
    /// Returns the synthetic AttributeId. Idempotent - no-ops if already registered.
    ///
    /// For attributes created via [`tagged_attribute`](Self::tagged_attribute),
    /// this materializes the combo from the template as its own node. For
    /// plain attributes, it registers a filtered view of the parent's
    /// modifiers.
    pub(crate) fn ensure_tag_query(
        &mut self,
        entity: Entity,
        parent_attribute_id: AttributeId,
        mask: TagMask,
    ) -> AttributeId {
        // Template attributes materialize the combo as its own node (no-op
        // for non-template attributes or already-materialized combos).
        self.maybe_materialize_template(entity, parent_attribute_id, mask);

        // Check if already registered
        if let Ok(attrs) = self.query.get(entity) {
            if let Some(existing) = attrs.tag_query_synthetic_id(parent_attribute_id, mask) {
                return existing;
            }
        }

        // Create synthetic AttributeId
        let parent_name = self.resolve_id(parent_attribute_id);
        let synthetic_name = format!("\0tag:{parent_name}:{}", mask.0);
        let synthetic_id = self.intern(&synthetic_name);

        // Register in Attributes
        if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs.register_tag_query(parent_attribute_id, mask, synthetic_id);
        }

        // Register dependency: parent → synthetic
        let parent_node = DepNode::new(entity, parent_attribute_id);
        let synthetic_node = DepNode::new(entity, synthetic_id);
        self.graph.add_edge(parent_node, synthetic_node);

        // Evaluate immediately so expressions that depend on this synthetic
        // node see the correct value rather than the default 0.
        if let Ok(mut attrs) = self.query.get_mut(entity) {
            attrs.evaluate_and_cache(synthetic_id);
        }

        synthetic_id
    }

    /// Cache source attribute values in `entity`'s context for the
    /// `LoadSource` / `LoadSourceTagged` ops in an ad-hoc expression.
    ///
    /// Use this when evaluating an expression that isn't registered as a
    /// persistent modifier on any attribute node (e.g., one-shot instants,
    /// preview evaluations).  The caller must have already registered the
    /// required source aliases via [`register_source`](Self::register_source).
    pub fn cache_expr_source_values(&mut self, entity: Entity, expr: &Expr) {
        for (alias_id, attribute_id, cache_key, tag_mask) in expr.source_cache_keys() {
            let source_entity = self.graph.resolve_alias(entity, alias_id);
            let value = match source_entity {
                Some(se) => match self.read_source(se, attribute_id, tag_mask) {
                    Some(v) => v,
                    None => {
                        bevy::log::warn_once!(
                            "bevy_gauge: {:?}@{:?} reads an entity hidden by this \
                             AttributesMut's query filter; its cached value is left \
                             unchanged. (This warning is only logged once.)",
                            self.resolve_id(attribute_id),
                            self.resolve_id(alias_id),
                        );
                        continue;
                    }
                },
                None => 0.0,
            };

            if let Ok(mut attrs) = self.query.get_mut(entity) {
                attrs.context.set(cache_key, value);
            }
        }
    }

    /// Read a source attribute value (optionally tag-filtered) for caching.
    ///
    /// Returns `None` when `source` has `Attributes` but is hidden by the
    /// query filter `F`, so the caller can keep its existing cached value
    /// instead of overwriting it with 0.0. A source with no `Attributes` at
    /// all reads as `Some(0.0)`, consistent with `unregister_source`.
    fn read_source(
        &mut self,
        source: Entity,
        attribute_id: AttributeId,
        tag_mask: Option<TagMask>,
    ) -> Option<f32> {
        if self.is_filtered_out(source) {
            return None;
        }
        Some(match tag_mask {
            Some(mask) => self.read_source_tagged(source, attribute_id, mask),
            None => self
                .query
                .get(source)
                .map(|attrs| attrs.get(attribute_id))
                .unwrap_or(0.0),
        })
    }

    /// True when `entity` has `Attributes` but the query filter `F` hides it.
    /// Always false for the default unfiltered `AttributesMut`.
    fn is_filtered_out(&self, entity: Entity) -> bool {
        self.with_attributes.contains(entity) && self.query.get(entity).is_err()
    }

    /// Queue a re-evaluation of `node` for the next command flush, where it
    /// runs with an unfiltered `AttributesMut`. Used when propagation needs to
    /// reach an entity the query filter `F` hides.
    fn defer_propagation(&mut self, node: DepNode) {
        self.commands.queue(move |world: &mut World| {
            let mut state = SystemState::<AttributesMut>::new(world);
            if let Ok(mut attributes) = state.get_mut(world) {
                attributes.evaluate_and_propagate(node.entity, node.attribute);
            }
            state.apply(world);
        });
    }

    /// Read a tag-filtered value from a source entity, materializing the tag
    /// query on the source if it doesn't exist yet.
    ///
    /// This is what makes cross-entity tagged refs (`Damage{FIRE}@weapon`)
    /// work without the source having evaluated that tag combo itself: the
    /// first read registers the query on the source, evaluates it, and wires
    /// it into the graph so later changes propagate.
    fn read_source_tagged(
        &mut self,
        source: Entity,
        attribute_id: AttributeId,
        mask: TagMask,
    ) -> f32 {
        if self.query.get(source).is_err() {
            return 0.0;
        }
        let synthetic = self.ensure_tag_query(source, attribute_id, mask);
        self.query
            .get(source)
            .map(|attrs| attrs.get(synthetic))
            .unwrap_or(0.0)
    }

    // -----------------------------------------------------------------------
    // Internal: source value caching
    // -----------------------------------------------------------------------

    /// Cache source attribute values in the local context for all expression
    /// modifiers on a attribute that reference cross-entity aliases.
    fn cache_source_values(&mut self, entity: Entity, attribute_id: AttributeId) {
        let cache_entries: Vec<(AttributeId, AttributeId, AttributeId, Option<TagMask>)> = {
            let Ok(attrs) = self.query.get(entity) else { return };
            let Some(node) = attrs.nodes.get(&attribute_id) else { return };
            node.expressions()
                .flat_map(|expr| expr.source_cache_keys())
                .collect()
        };

        if cache_entries.is_empty() {
            return;
        }

        let mut deferred = false;
        for (alias, source_attribute, cache_key, tag_mask) in cache_entries {
            let source_entity = self.graph.resolve_alias(entity, alias);
            let value = match source_entity {
                Some(se) => match self.read_source(se, source_attribute, tag_mask) {
                    Some(v) => v,
                    None => {
                        // Source hidden by the filter: keep the cached value
                        // and refresh this node once the filter is gone.
                        if !deferred {
                            self.defer_propagation(DepNode::new(entity, attribute_id));
                            deferred = true;
                        }
                        continue;
                    }
                },
                None => 0.0,
            };

            if let Ok(mut attrs) = self.query.get_mut(entity) {
                attrs.context.set(cache_key, value);
            }
        }
    }

    /// Clear cached source values for all attributes that use a given alias.
    fn clear_source_cache(&mut self, entity: Entity, alias_id: AttributeId) {
        // Collect all (attribute_id, cache_keys) for modifiers that reference this alias
        let clear_keys: Vec<AttributeId> = {
            let Ok(attrs) = self.query.get(entity) else { return };
            attrs.nodes.values()
                .flat_map(|node| {
                    node.expressions().flat_map(|expr| {
                        expr.source_cache_keys()
                            .filter(|(a, _, _, _)| *a == alias_id)
                            .map(|(_, _, ck, _)| ck)
                    })
                })
                .collect()
        };

        if let Ok(mut attrs) = self.query.get_mut(entity) {
            for key in clear_keys {
                attrs.context.set(key, 0.0);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Internal: evaluation and propagation
    // -----------------------------------------------------------------------

    /// Re-evaluate `root` and propagate changes through the dependency graph
    /// in **topological order**, so every node is evaluated exactly once and
    /// only after all of its (in-subgraph) sources have settled. This is what
    /// makes diamond dependencies (A -> B, A -> C, B+C -> D) recompute
    /// correctly: D evaluates once, after both B and C.
    ///
    /// This can be re-entered: re-evaluating a node may trigger lazy template
    /// materialization on a source entity, which adds a modifier and starts a
    /// nested pass. That is sound because the nested pass is complete over
    /// the graph at that moment and this pass dirties dependents by comparing
    /// old and new cached values, so anything the nested pass already settled
    /// is seen as unchanged here. The `template_registered_after_cross_entity_
    /// reference_converges` regression test pins this.
    fn evaluate_and_propagate(&mut self, entity: Entity, attribute_id: AttributeId) {
        let root = DepNode::new(entity, attribute_id);

        // 1. Collect the downstream subgraph (closed under `dependents`) and
        //    a local copy of its adjacency, so we don't hold graph borrows
        //    while evaluating.
        let mut subgraph: HashSet<DepNode> = HashSet::new();
        subgraph.insert(root);
        let mut edges: HashMap<DepNode, Vec<DepNode>> = HashMap::new();
        let mut discovery = vec![root];
        while let Some(node) = discovery.pop() {
            let deps: Vec<DepNode> = self.graph.dependents(node).collect();
            for &dep in &deps {
                if subgraph.insert(dep) {
                    discovery.push(dep);
                }
            }
            edges.insert(node, deps);
        }

        // Fast path: no dependents.
        if subgraph.len() == 1 {
            self.re_evaluate(root);
            return;
        }

        // 2. In-degree of each node, counting only edges within the subgraph.
        let mut in_degree: HashMap<DepNode, usize> =
            subgraph.iter().map(|n| (*n, 0)).collect();
        for deps in edges.values() {
            for dep in deps {
                *in_degree.get_mut(dep).expect("dep is in subgraph") += 1;
            }
        }

        // 3. Kahn's algorithm. A node is re-evaluated only if it's the root or
        //    at least one of its sources changed value.
        let mut ready: Vec<DepNode> = in_degree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut dirty: HashSet<DepNode> = HashSet::new();
        dirty.insert(root);
        let mut processed = 0usize;

        while let Some(node) = ready.pop() {
            processed += 1;

            if dirty.contains(&node) && self.re_evaluate(node) {
                for &dep in &edges[&node] {
                    dirty.insert(dep);
                }
            }

            for &dep in &edges[&node] {
                let d = in_degree.get_mut(&dep).expect("dep is in subgraph");
                *d -= 1;
                if *d == 0 {
                    ready.push(dep);
                }
            }
        }

        // 4. Cycle backstop: nodes on a dependency cycle never reach
        //    in-degree 0. Evaluate each of them once so they at least pick
        //    up this mutation, and tell the user - cyclic attribute graphs
        //    produce order-dependent values and are considered an authoring
        //    error.
        if processed < subgraph.len() {
            bevy::log::warn_once!(
                "bevy_gauge: dependency cycle detected while propagating \
                 {:?}; attributes on a cycle re-evaluate in arbitrary order \
                 and their values are unreliable. Break the cycle in your \
                 attribute definitions. (This warning is only logged once.)",
                self.resolve_id(root.attribute),
            );
            let leftover: Vec<DepNode> = in_degree
                .iter()
                .filter(|(_, d)| **d > 0)
                .map(|(n, _)| *n)
                .collect();
            for node in leftover {
                self.re_evaluate(node);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Internal: entity removal
    // -----------------------------------------------------------------------

    /// Clean up after an entity with `Attributes` is despawned.
    ///
    /// Removes all graph data involving the entity, zeroes cached source
    /// values for aliases that pointed at it (consistent with
    /// [`unregister_source`](Self::unregister_source)), and re-evaluates
    /// attributes on other entities that depended on it.
    pub(crate) fn handle_entity_removal(&mut self, entity: Entity) {
        let cleanup = self.graph.remove_entity(entity);

        for (owner, alias_id) in cleanup.dangling_aliases {
            if owner != entity {
                self.clear_source_cache(owner, alias_id);
            }
        }

        for dep in cleanup.dependents {
            if dep.entity != entity {
                self.evaluate_and_propagate(dep.entity, dep.attribute);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Internal: instant support
    // -----------------------------------------------------------------------

    /// Sum of the flat modifiers on an attribute that `set_base` /
    /// `set_base_tagged` would replace: untagged flats when `tag` is empty,
    /// exact-tag flats otherwise. Returns 0.0 if the attribute doesn't exist.
    pub(crate) fn flat_base(&self, entity: Entity, attribute: &str, tag: TagMask) -> f32 {
        let Some(attribute_id) = self.try_intern(attribute) else {
            return 0.0;
        };
        self.query
            .get(entity)
            .ok()
            .and_then(|attrs| attrs.nodes.get(&attribute_id))
            .map(|node| node.flat_base(tag))
            .unwrap_or(0.0)
    }

    /// Refresh a node's cross-entity source caches, re-evaluate it, and
    /// report whether its cached value changed.
    ///
    /// If the query filter `F` hides the node's entity, the node can't be
    /// evaluated here; its re-evaluation is deferred to the next command
    /// flush and this returns `false` (its cached value is unchanged, so its
    /// dependents are not dirtied by this pass - the deferred pass reaches
    /// them).
    fn re_evaluate(&mut self, node: DepNode) -> bool {
        self.cache_source_values(node.entity, node.attribute);

        if let Ok(mut attrs) = self.query.get_mut(node.entity) {
            let old = attrs.context.get(node.attribute);
            let new = attrs.evaluate_and_cache(node.attribute);
            !crate::expr::approx_eq(old, new)
        } else {
            if self.is_filtered_out(node.entity) {
                self.defer_propagation(node);
            }
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// Qualify short part names in an expression string with a parent prefix.
///
/// Given `prefix = "Damage"`, `parts = ["base", "increased"]`, and
/// `expr = "base * (1 + increased)"`, produces:
///
/// ```text
/// "Damage.base * (1 + Damage.increased)"
/// ```
///
/// If `tag_suffix` is `Some("{FIRE|MELEE}")`, each qualified part also gets
/// the suffix appended:
///
/// ```text
/// "Damage.base{FIRE|MELEE} * (1 + Damage.increased{FIRE|MELEE})"
/// ```
///
/// Identifiers not in `parts` (e.g., function names, other attribute refs) are
/// left unchanged.
fn qualify_expression(
    prefix: &str,
    parts: &[&str],
    expr: &str,
    tag_suffix: Option<&str>,
) -> String {
    let mut result = String::with_capacity(expr.len() * 2);
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            // Read a full identifier
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();

            if parts.contains(&ident.as_str()) {
                // Qualify: "base" → "Damage.base" (+ optional tag suffix)
                result.push_str(prefix);
                result.push('.');
                result.push_str(&ident);
                if let Some(suffix) = tag_suffix {
                    result.push_str(suffix);
                }
            } else {
                // Not a part - pass through unchanged (function names, other attributes)
                result.push_str(&ident);
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod qualify_tests {
    use super::*;

    #[test]
    fn basic_qualification() {
        let result = qualify_expression(
            "Damage",
            &["base", "increased", "more"],
            "base * (1 + increased) * more",
            None,
        );
        assert_eq!(
            result,
            "Damage.base * (1 + Damage.increased) * Damage.more"
        );
    }

    #[test]
    fn with_tag_suffix() {
        let result = qualify_expression(
            "Damage",
            &["added", "increased"],
            "added * (1 + increased)",
            Some("{FIRE|MELEE}"),
        );
        assert_eq!(
            result,
            "Damage.added{FIRE|MELEE} * (1 + Damage.increased{FIRE|MELEE})"
        );
    }

    #[test]
    fn non_part_identifiers_unchanged() {
        let result = qualify_expression(
            "Damage",
            &["base"],
            "max(base, Strength) + 1.0",
            None,
        );
        assert_eq!(result, "max(Damage.base, Strength) + 1.0");
    }

    #[test]
    fn no_false_partial_match() {
        // "base_extra" should NOT be treated as "base" + "extra"
        let result = qualify_expression(
            "Attribute",
            &["base"],
            "base_extra + base",
            None,
        );
        assert_eq!(result, "base_extra + Attribute.base");
    }

    #[test]
    fn empty_expression() {
        let result = qualify_expression("X", &["a"], "", None);
        assert_eq!(result, "");
    }
}
