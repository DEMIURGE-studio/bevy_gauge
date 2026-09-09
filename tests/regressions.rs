//! Regression tests for dependency-graph and evaluation correctness
//! guarantees: diamond recomputation, edge refcounting, tag-query counting,
//! cross-entity tagged refs, despawn cleanup, instant semantics, cycle
//! termination, change detection, and expression depth.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gauge::graph::DependencyGraph;
use bevy_gauge::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(AttributesPlugin);
    app
}

fn spawn_attrs(app: &mut App) -> Entity {
    app.world_mut().spawn(Attributes::new()).id()
}

/// Passive cached read - what sync systems and `&Attributes` readers see.
fn cached_value(app: &App, entity: Entity, name: &str) -> f32 {
    app.world()
        .entity(entity)
        .get::<Attributes>()
        .expect("entity has Attributes")
        .value(name)
}

fn register_tags(app: &mut App, tags: &[(&str, TagMask)]) {
    let mut resolver = app.world_mut().resource_mut::<TagResolver>();
    for (name, mask) in tags {
        resolver.register(name, *mask);
    }
}

// ---------------------------------------------------------------------------
// Diamond staleness
// ---------------------------------------------------------------------------

#[test]
fn diamond_dependency_recomputes_correctly() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(e, "A", 1.0);
            attributes.add_expr_modifier(e, "B", "A * 2").unwrap();
            attributes.add_expr_modifier(e, "C", "A * 3").unwrap();
            attributes.add_expr_modifier(e, "D", "B + C").unwrap();
        })
        .unwrap();
    assert_eq!(cached_value(&app, e, "D"), 5.0);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set_base(e, "A", 2.0);
        })
        .unwrap();

    // The cached value must be 10 (new B=4 + new C=6), not 8 (stale B=2 + new C=6).
    assert_eq!(cached_value(&app, e, "D"), 10.0);
}

#[test]
fn deep_diamond_recomputes_correctly() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    // Two-layer diamond: A -> {B, C} -> {D}, D -> {E, F} -> G
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(e, "A", 1.0);
            attributes.add_expr_modifier(e, "B", "A * 2").unwrap();
            attributes.add_expr_modifier(e, "C", "A * 3").unwrap();
            attributes.add_expr_modifier(e, "D", "B + C").unwrap();
            attributes.add_expr_modifier(e, "E", "D * 2").unwrap();
            attributes.add_expr_modifier(e, "F", "D * 3").unwrap();
            attributes.add_expr_modifier(e, "G", "E + F").unwrap();
        })
        .unwrap();
    assert_eq!(cached_value(&app, e, "G"), 25.0); // D=5, E=10, F=15

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set_base(e, "A", 2.0);
        })
        .unwrap();
    assert_eq!(cached_value(&app, e, "G"), 50.0); // D=10, E=20, F=30
}

// ---------------------------------------------------------------------------
// Edge refcounting
// ---------------------------------------------------------------------------

#[test]
fn shared_edge_survives_modifier_removal() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(e, "Strength", 10.0);

            // Two expression modifiers on Damage, both reading Strength -
            // they share the Strength -> Damage edge.
            let m1 = Modifier::Expr(Expr::compile("Strength * 2", None).unwrap());
            let m2 = Modifier::Expr(Expr::compile("Strength * 3", None).unwrap());
            attributes.add_modifier(e, "Damage", m1.clone());
            attributes.add_modifier(e, "Damage", m2);
            assert_eq!(attributes.evaluate(e, "Damage"), 50.0);

            // Remove one; the survivor must keep updating.
            attributes.remove_modifier(e, "Damage", &m1);
            assert_eq!(attributes.evaluate(e, "Damage"), 30.0);

            attributes.set_base(e, "Strength", 20.0);
        })
        .unwrap();

    // Propagation through the surviving edge: 20 * 3 = 60.
    assert_eq!(cached_value(&app, e, "Damage"), 60.0);
}

// ---------------------------------------------------------------------------
// Template tag-query double counting
// ---------------------------------------------------------------------------

#[test]
fn superset_tag_query_does_not_double_count() {
    let mut app = test_app();
    let fire = TagMask::bit(0);
    let sword = TagMask::bit(1);
    register_tags(&mut app, &[("FIRE", fire), ("SWORD", sword)]);
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes
                .tagged_attribute(
                    e,
                    "Damage",
                    &[("added", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
                    "added * (1 + increased)",
                )
                .unwrap();
            attributes.add_modifier_tagged(e, "Damage.added", 10.0, fire);

            // Materialize FIRE first, then query the superset FIRE|SWORD.
            let fire_dmg = attributes.evaluate_tagged(e, "Damage", fire);
            assert_eq!(fire_dmg, 10.0);

            let fire_sword_dmg = attributes.evaluate_tagged(e, "Damage", fire | sword);
            // Must be 10 (the FIRE-tagged added modifier matches the superset
            // query at the PART level), not 20 (both combo templates counted).
            assert_eq!(fire_sword_dmg, 10.0);

            // The narrower query still works after the superset materialized.
            assert_eq!(attributes.evaluate_tagged(e, "Damage", fire), 10.0);
        })
        .unwrap();

    // Changes to parts propagate to every materialized combo.
    let fire_c = fire;
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier_tagged(e, "Damage.added", 5.0, fire_c);
        })
        .unwrap();
    let attrs = app.world().entity(e).get::<Attributes>().unwrap();
    assert_eq!(attrs.value_tagged("Damage", fire), 15.0);
    assert_eq!(attrs.value_tagged("Damage", fire | sword), 15.0);
}

// ---------------------------------------------------------------------------
// Cross-entity tagged refs
// ---------------------------------------------------------------------------

#[test]
fn cross_entity_tagged_ref_reads_live_value() {
    let mut app = test_app();
    let fire = TagMask::bit(0);
    register_tags(&mut app, &[("FIRE", fire)]);
    let weapon = spawn_attrs(&mut app);
    let player = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier_tagged(weapon, "Damage", 25.0, fire);
            attributes.register_source(player, "weapon", weapon);
            attributes
                .add_expr_modifier(player, "SpellPower", "Damage{FIRE}@weapon * 2")
                .unwrap();
        })
        .unwrap();
    // Nothing on the weapon ever evaluated the FIRE query itself - the read
    // must materialize it: 25 * 2 = 50.
    assert_eq!(cached_value(&app, player, "SpellPower"), 50.0);

    // And changes on the source propagate through the tag query.
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier_tagged(weapon, "Damage", 5.0, fire);
        })
        .unwrap();
    assert_eq!(cached_value(&app, player, "SpellPower"), 60.0);
}

#[test]
fn cross_entity_tagged_ref_works_when_alias_set_after_modifier() {
    let mut app = test_app();
    let fire = TagMask::bit(0);
    register_tags(&mut app, &[("FIRE", fire)]);
    let weapon = spawn_attrs(&mut app);
    let player = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier_tagged(weapon, "Damage", 25.0, fire);
            // Modifier added BEFORE the alias exists: reads 0 for now.
            attributes
                .add_expr_modifier(player, "SpellPower", "Damage{FIRE}@weapon * 2")
                .unwrap();
            assert_eq!(attributes.evaluate(player, "SpellPower"), 0.0);

            // Registering the alias rewires and re-evaluates.
            attributes.register_source(player, "weapon", weapon);
        })
        .unwrap();
    assert_eq!(cached_value(&app, player, "SpellPower"), 50.0);
}

// ---------------------------------------------------------------------------
// Despawn cleanup
// ---------------------------------------------------------------------------

#[test]
fn despawned_source_zeroes_dependents() {
    let mut app = test_app();
    let source = spawn_attrs(&mut app);
    let dependent = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(source, "Strength", 10.0);
            attributes.register_source(dependent, "wielder", source);
            attributes
                .add_expr_modifier(dependent, "AttackPower", "Strength@wielder + 1")
                .unwrap();
        })
        .unwrap();
    assert_eq!(cached_value(&app, dependent, "AttackPower"), 11.0);

    app.world_mut().despawn(source);

    // The dependent must re-evaluate with the source gone (reads 0), not
    // stay frozen at 11.
    assert_eq!(cached_value(&app, dependent, "AttackPower"), 1.0);

    // The dangling alias is gone too.
    let resolved = app
        .world_mut()
        .run_system_once(move |attributes: AttributesMut| {
            attributes.resolve_source(dependent, "wielder")
        })
        .unwrap();
    assert_eq!(resolved, None);
}

// ---------------------------------------------------------------------------
// Instant side effects
// ---------------------------------------------------------------------------

#[test]
fn instant_role_does_not_clobber_existing_alias() {
    let mut app = test_app();
    let weapon_a = spawn_attrs(&mut app);
    let weapon_b = spawn_attrs(&mut app);
    let target = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(weapon_a, "Damage", 7.0);
            attributes.set(weapon_b, "Damage", 100.0);

            // Persistent alias: target's Power reads weapon_a's Damage.
            attributes.register_source(target, "weapon", weapon_a);
            attributes
                .add_expr_modifier(target, "Power", "Damage@weapon")
                .unwrap();
            assert_eq!(attributes.evaluate(target, "Power"), 7.0);

            // An instant whose role name collides with the persistent alias.
            let on_hit = instant! {
                "Scratch" += "Damage@weapon",
            };
            attributes.apply_instant(&on_hit, &[("weapon", weapon_b)], target);

            // The persistent alias survives and still points at weapon_a...
            assert_eq!(attributes.resolve_source(target, "weapon"), Some(weapon_a));
            // ...and the instant read through the EXISTING alias (7, not 100).
            assert_eq!(attributes.evaluate(target, "Scratch"), 7.0);

            // The persistent binding still updates after the instant.
            attributes.set_base(weapon_a, "Damage", 9.0);
        })
        .unwrap();
    assert_eq!(cached_value(&app, target, "Power"), 9.0);
}

#[test]
fn instant_add_counts_expression_modifiers_once() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(e, "Bonus", 5.0);
            attributes.set(e, "Mana", 10.0);
            attributes.add_expr_modifier(e, "Mana", "Bonus").unwrap();
            assert_eq!(attributes.evaluate(e, "Mana"), 15.0);

            let regen = instant! {
                "Mana" += 1.0,
            };
            attributes.apply_instant(&regen, &[], e);

            // 15 + 1 = 16. Add adjusts the flat base, so the Bonus
            // expression counts once; writing the evaluated value back into
            // the base would count it twice (21).
            assert_eq!(attributes.evaluate(e, "Mana"), 16.0);
        })
        .unwrap();
}

// ---------------------------------------------------------------------------
// Graph pollution
// ---------------------------------------------------------------------------

#[test]
fn adding_modifier_to_entity_without_attributes_leaves_graph_clean() {
    let mut app = test_app();
    let bare = app.world_mut().spawn_empty().id();

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            let _ = attributes.add_expr_modifier(bare, "X", "Y + 1");
        })
        .unwrap();

    assert!(app.world().resource::<DependencyGraph>().is_empty());
}

#[test]
fn tagged_instant_add_counts_globals_once() {
    let mut app = test_app();
    let fire = TagMask::bit(0);
    register_tags(&mut app, &[("FIRE", fire)]);
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(e, "Damage", 5.0); // global flat
            attributes.set_tagged(e, "Damage", 10.0, fire); // FIRE flat
            assert_eq!(attributes.evaluate_tagged(e, "Damage", fire), 15.0);

            let hit = instant! {
                "Damage{FIRE}" += 3.0,
            };
            attributes.apply_instant(&hit, &[], e);

            // 15 + 3 = 18. Add adjusts the exact-tag flat base; writing the
            // subset-matching query value (which includes the global flat)
            // back into the exact-tag base would count the global twice (23).
            assert_eq!(attributes.evaluate_tagged(e, "Damage", fire), 18.0);
        })
        .unwrap();
}

// ---------------------------------------------------------------------------
// Cycle backstop
// ---------------------------------------------------------------------------

#[test]
fn cycles_terminate_and_still_pick_up_mutations() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(e, "X", 1.0);
            // A and B depend on each other (a cycle), but both are
            // mathematically determined by X alone, so the assertions are
            // order-independent even though cycle evaluation order isn't.
            attributes.add_expr_modifier(e, "A", "X + 0 * B").unwrap();
            attributes.add_expr_modifier(e, "B", "X + 0 * A").unwrap();

            // Must terminate (Kahn's never reaches cycle nodes; the backstop
            // evaluates them once) and both must see the new X.
            attributes.set_base(e, "X", 5.0);
        })
        .unwrap();
    assert_eq!(cached_value(&app, e, "A"), 5.0);
    assert_eq!(cached_value(&app, e, "B"), 5.0);
}

// ---------------------------------------------------------------------------
// Change detection at large magnitudes
// ---------------------------------------------------------------------------

#[test]
fn large_magnitude_changes_propagate() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(e, "Gold", 1e8);
            attributes.add_expr_modifier(e, "Score", "Gold / 2").unwrap();
        })
        .unwrap();
    assert_eq!(cached_value(&app, e, "Score"), 5e7);

    // ulp at 1e8 is 8; +16 is a real, representable change and must beat the
    // relative tolerance (~12 at this magnitude) and propagate.
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set_base(e, "Gold", 1e8 + 16.0);
        })
        .unwrap();
    assert_eq!(cached_value(&app, e, "Score"), 50_000_008.0);
}

// ---------------------------------------------------------------------------
// Expression stack depth
// ---------------------------------------------------------------------------

/// Right-nested multiplier composition: peak stack depth = 2n + 1.
fn right_nested(n: usize) -> String {
    let mut s = String::from("1.0");
    for _ in 0..n {
        s = format!("2.0 * (1.0 + {s})");
    }
    s
}

#[test]
fn expression_depth_checked_at_compile_time() {
    // Interner must exist before Expr::compile.
    let _app = test_app();

    // Depth 15: fits.
    assert!(Expr::compile(&right_nested(7), None).is_ok());

    // Depth 17: rejected at compile time instead of panicking at eval time.
    match Expr::compile(&right_nested(8), None) {
        Err(CompileError::ExpressionTooDeep { required, max }) => {
            assert_eq!(required, 17);
            assert_eq!(max, 16);
        }
        other => panic!("expected ExpressionTooDeep, got {:?}", other.map(|_| "Ok")),
    }

    // Same math left-folded compiles no matter how long it is.
    let mut flat = String::from("1.0");
    for _ in 0..100 {
        flat.push_str(" * (1.0 + 2.0)");
    }
    assert!(Expr::compile(&flat, None).is_ok());
}

// ---------------------------------------------------------------------------
// set_base keeps earlier contributors removable
// ---------------------------------------------------------------------------

#[test]
fn set_base_survives_later_modifier_removal() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier(e, "Damage", 10.0);
            attributes.add_modifier(e, "Damage", 5.0);
            assert_eq!(attributes.evaluate(e, "Damage"), 15.0);

            // An instant-style rewrite of the flat base.
            attributes.set_base(e, "Damage", 18.0);
            assert_eq!(attributes.evaluate(e, "Damage"), 18.0);

            // Removing the earlier contributors subtracts them; it must not
            // drain the slot and wipe the base.
            attributes.remove_modifier(e, "Damage", &Modifier::Flat(10.0));
            assert_eq!(attributes.evaluate(e, "Damage"), 8.0);
            attributes.remove_modifier(e, "Damage", &Modifier::Flat(5.0));
            assert_eq!(attributes.evaluate(e, "Damage"), 3.0);

            // And the base still stacks with new modifiers afterwards.
            attributes.add_modifier(e, "Damage", 4.0);
            assert_eq!(attributes.evaluate(e, "Damage"), 7.0);
            attributes.remove_modifier(e, "Damage", &Modifier::Flat(4.0));
            assert_eq!(attributes.evaluate(e, "Damage"), 3.0);
        })
        .unwrap();
    assert_eq!(cached_value(&app, e, "Damage"), 3.0);
}

// ---------------------------------------------------------------------------
// Re-entrant propagation via lazy template materialization
// ---------------------------------------------------------------------------

/// A reader references `Damage{FIRE}@weapon` before the weapon has `Damage`
/// as a tagged (template) attribute. The first propagation through the
/// reader after that converts the filtered view into a template node, which
/// adds a modifier *during* the propagation pass and starts a nested pass
/// over a graph the outer pass has already snapshotted. Every value must
/// still converge, both immediately and on later changes.
#[test]
fn template_registered_after_cross_entity_reference_converges() {
    let mut app = test_app();
    let fire = TagMask::bit(0);
    register_tags(&mut app, &[("FIRE", fire)]);
    let weapon = spawn_attrs(&mut app);
    let reader = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier_tagged(weapon, "Damage", 10.0, fire);
            attributes.register_source(reader, "weapon", weapon);
            attributes
                .add_expr_modifier(reader, "X", "Damage{FIRE}@weapon * 2")
                .unwrap();
            assert_eq!(attributes.evaluate(reader, "X"), 20.0);

            // Now Damage becomes a template attribute; the existing filtered
            // view the reader uses is stale until the next propagation.
            attributes
                .tagged_attribute(
                    weapon,
                    "Damage",
                    &[("added", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
                    "added * (1 + increased)",
                )
                .unwrap();
            attributes.add_modifier_tagged(weapon, "Damage.added", 5.0, fire);

            // Propagate from the parent: reaches the reader, whose
            // re-evaluation triggers the template conversion mid-pass.
            attributes.set_base_tagged(weapon, "Damage", 12.0, fire);
        })
        .unwrap();

    // Template semantics: Damage{FIRE} = added{FIRE} * (1 + increased{FIRE}) = 5.
    assert_eq!(cached_value(&app, reader, "X"), 10.0);

    // The converted node is wired to its parts: further changes propagate.
    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.add_modifier_tagged(weapon, "Damage.increased", 1.0, fire);
        })
        .unwrap();
    assert_eq!(cached_value(&app, reader, "X"), 20.0);
}

// ---------------------------------------------------------------------------
// Filtered AttributesMut
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Player;

/// Propagation from a filtered `AttributesMut` must still reach entities the
/// filter hides (deferred to the command flush) and must never zero a cached
/// cross-entity value just because the source is hidden.
#[test]
fn filtered_attributes_mut_propagates_after_flush() {
    let mut app = test_app();
    let player = app.world_mut().spawn((Attributes::new(), Player)).id();
    let minion = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes.set(player, "Aura", 1.0);
            attributes.register_source(minion, "owner", player);
            attributes.add_expr_modifier(minion, "Power", "Aura@owner * 2").unwrap();

            attributes.set(minion, "Threat", 7.0);
            attributes.register_source(player, "pet", minion);
        })
        .unwrap();
    assert_eq!(cached_value(&app, minion, "Power"), 2.0);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut<With<Player>>| {
            // Downstream of the change is hidden by the filter.
            attributes.set_base(player, "Aura", 5.0);

            // A new expression on a visible entity reads a hidden source.
            attributes.add_expr_modifier(player, "PetThreat", "Threat@pet").unwrap();

            // Inside the filtered system the hidden entity is untouched.
            assert_eq!(attributes.value(minion, "Power"), 0.0, "hidden by filter");
        })
        .unwrap();

    // After the flush both sides are current.
    assert_eq!(cached_value(&app, minion, "Power"), 10.0);
    assert_eq!(cached_value(&app, player, "PetThreat"), 7.0);
}

// ---------------------------------------------------------------------------
// Tag names that are ambiguous across namespaces still materialize
// ---------------------------------------------------------------------------

#[test]
fn ambiguous_short_tag_name_materializes_template() {
    let mut app = test_app();
    let element_fire = TagMask::bit(0);
    let weapon_fire = TagMask::bit(4);
    {
        let mut resolver = app.world_mut().resource_mut::<TagResolver>();
        resolver.register_namespaced("Element", "FIRE", element_fire);
        resolver.register_namespaced("Weapon", "FIRE", weapon_fire);
        assert_eq!(resolver.resolve("FIRE"), None, "short name is ambiguous");
    }
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            attributes
                .tagged_attribute(e, "Damage", &[("added", ReduceFn::Sum)], "added")
                .unwrap();
            attributes.add_modifier_tagged(e, "Damage.added", 10.0, element_fire);
            attributes.add_modifier_tagged(e, "Damage.added", 3.0, weapon_fire);

            // Materialization builds `Damage.added{ELEMENT::FIRE}`; the bare
            // `{FIRE}` would fail to compile and read as 0.
            assert_eq!(attributes.evaluate_tagged(e, "Damage", element_fire), 10.0);
            assert_eq!(attributes.evaluate_tagged(e, "Damage", weapon_fire), 3.0);
            assert_eq!(
                attributes.evaluate_tagged(e, "Damage", element_fire | weapon_fire),
                13.0
            );
        })
        .unwrap();
}

#[test]
fn tagged_attribute_rejects_broken_template() {
    let mut app = test_app();
    let e = spawn_attrs(&mut app);

    app.world_mut()
        .run_system_once(move |mut attributes: AttributesMut| {
            let result = attributes.tagged_attribute(
                e,
                "Damage",
                &[("added", ReduceFn::Sum)],
                "added * (1 +",
            );
            assert!(result.is_err(), "broken template must be rejected up front");
            // Nothing was set up for it.
            assert_eq!(attributes.evaluate(e, "Damage.added"), 0.0);
        })
        .unwrap();
}
