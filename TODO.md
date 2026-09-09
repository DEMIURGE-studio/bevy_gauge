# TODO

Follow-ups from the September 2026 review. Ordered by value for effort within
each section. Nothing here blocks using the crate as-is.

## Performance

- [ ] **Replace SipHash maps with FxHash.** `AttributeContext`, `Attributes::nodes`,
      `tag_queries`, `tag_query_ids`, and the `DependencyGraph` edge maps all key
      on `AttributeId` or `DepNode` (u32-sized) but use the default hasher. Every
      `Op::Load` in expression evaluation and every edge lookup in propagation pays
      for it. Mechanical change, largest win available.
      (`src/context.rs`, `src/attributes.rs`, `src/graph.rs`)
- [ ] **Stop allocating per field per frame in `AttributeResolvable`.** The derive
      builds each field's path with `format!` and then does a string-to-id lookup
      on every resolve. Precompute interned `AttributeId`s on first use
      (e.g. a `OnceLock` per field, or resolve the prefix once and pass ids down).
      (`macros/src/resolvable_impl.rs`)
- [ ] **Cache compiled instant expressions.** `evaluate_instant` parses and
      compiles every `ExprSource` entry on each application. Store the compiled
      `Expr` on the entry (lazily, keyed on the resolver) so a hit does not parse.
      (`src/instant.rs`)
- [ ] **Fix the propagation benches.** `bench_stats_update_propagation` and
      `bench_propagation_mutation` call `add_modifier` inside `b.iter`, which
      appends a modifier per iteration, so they measure a growing workload.
      Use `set_base`. Do this before the items above so the numbers mean something.
      (`benches/stats_bench.rs`)
- [ ] **Do not mark `Attributes` changed on read.** `AttributesMut::evaluate`,
      `evaluate_id`, and `try_evaluate` take `&mut` via `query.get_mut`, so
      read-shaped calls trip `Changed<Attributes>` and the derived-component sync
      runs nearly every frame in both `PreUpdate` and `PostUpdate`. Return the
      cached value through the read path, or only take the mutable borrow when
      the value actually changes.
      (`src/attributes_mut.rs`, `src/derived.rs`)
- [ ] **Avoid rebuilding `SystemState` per `commands.entity(..).attrs(..)` call.**
      Query state is constructed on every flush of every closure. Cache it in a
      resource or `Local` if spawn-time attribute setup shows up in profiles.
      (`src/commands.rs`)

## Tests

- [ ] **Cover the derived-component bridge.** Nothing exercises `AttributeDerived`,
      `WriteBack`, `InitFrom`, `InitTo`, or `add_gauge_sync_to_schedule`. An
      integration test with a `Health` component using the `AttributeComponent`
      derive (read, write, init_from) through `PreUpdate`/`PostUpdate` would cover
      the path most users hit first.
- [ ] **Cover `commands.entity(..).attrs(..)`.** Only `examples/custom_extensions.rs`
      uses it.
- [ ] **Cover `ModifierSet::remove` and `try_remove`**, including partial failure.
- [ ] **Cover init ordering.** `InitTo`'s `On<Add, T>` observer and
      `apply_initial_attributes`'s `On<Add, AttributeInitializer>` can both write
      the same attribute for one spawn bundle; last writer wins. Pin whichever
      precedence is intended, and cover the case where `T` is added to an entity
      that has no `Attributes` yet (`apply_init_from` consumes `Added<T>` and never
      retries).
- [ ] **Doctests.** 30 of 31 are `ignore`. Convert the ones that only need an
      `App` with `AttributesPlugin` into real doctests.

## Cleanup

- [ ] Remove or use dead items: `TagMask::satisfies`, `TagMask::union`,
      `TagResolver::resolve_set`, `DependencyGraph::remove_dependent`,
      `TaggedModifier::global`, `AttributeNode::sum` / `product`,
      `AttributeContext::{remove, contains, len, is_empty}`,
      `Attributes::has_attribute`, `CompileError::UnresolvableTagMask`.
- [ ] Deduplicate `add_modifier_tagged` / `add_modifier_tagged_with_reduce` and the
      `apply` / `try_apply`, `remove` / `try_remove` pairs in `ModifierSet`.
- [ ] `AttributeWriter` has one implementor (`BoundAttributesMut`) that is pure
      delegation across 25 methods. Either give it a second implementor or make
      `BoundAttributesMut` inherent and drop the trait.
- [ ] `register_init_from!` does not exist even though the `_register_attribute!`
      arm does; `register_init_to` is not in the prelude. Make the four
      registration macros symmetric.
- [ ] Instant error handling: a bad tag in an attribute name (`"Damage{TYPO}"`) is
      discarded by `unwrap_or_else` and writes to a literal attribute named
      `Damage{TYPO}`; compile failures yield `0.0`. Log or return these.
      (`src/instant.rs`)
- [ ] `AttributeRequirements::met` warns every frame when uncompiled instead of
      panicking as the doc says, and the warning names a `check` method that does
      not exist. Requirements also compile with `Expr::compile(source, None)`, so
      `{TAG}` syntax is unusable in them.
      (`src/requirements.rs`)
- [ ] `commands.rs` logs "could not get attributes for {entity}" when the failure
      is system-param acquisition, not a missing component.
- [ ] Clippy: 7 lib warnings and 6 macro warnings, all trivial
      (`cargo clippy --fix`).
- [ ] Macro diagnostics: `classify_type` only matches bare idents, so
      `std::primitive::f32` or a type alias silently becomes `Composite`; composite
      `#[write]` / `#[init_to]` emit `compile_error!` inside the generated fn body
      rather than a spanned `syn::Error`.
      (`macros/src/attribute_component_impl.rs`)

## Avian

- [ ] `src/avian.rs` covers `Mass` only, with `AttributeDerived` and `InitTo` but
      no `WriteBack`, so avian-computed mass from `ColliderDensity` is overwritten
      each sync. No 2D, no tests, no example. Either grow it (`LinearDamping`,
      `Friction`, `Restitution`, `GravityScale`, 2D) or mark the feature
      experimental in the readme.
