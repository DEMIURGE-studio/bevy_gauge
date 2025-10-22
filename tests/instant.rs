use bevy::prelude::*;
use bevy_gauge::prelude::*;

#[test]
fn evaluate_expression_with_roles_works() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(bevy_gauge::plugin);

    Konfig::reset_for_test();
    Konfig::register_stat_type("Power", "Flat");

    let attacker = app.world_mut().spawn(Stats::new()).id();
    let defender = app.world_mut().spawn(Stats::new()).id();

    StatsMutator::with_world(app.world_mut(), |mut stats| {
        stats.set(attacker, "Power", 10.0);
        stats.set(defender, "Power", 7.0);
    });

    let expr = Expression::new("Power@attacker * 2 + Power@defender").unwrap();

    let result = {
        let q_stats = app.world_mut().query::<&Stats>();
        // SAFETY: Bevy queries need system context normally; for tests we can build a temporary world query
        // Use manual get via world scope
        // We'll emulate what the resolver expects
        // Create a nested function to borrow q_stats properly
        fn eval(expr: &Expression, world: &mut World, attacker: Entity, defender: Entity) -> f32 {
            let mut binding = world.query::<&Stats>();
            let q_stats: Query<&Stats> = binding.query(world);
            bevy_gauge::instant::evaluate_expression_with_roles(
                expr,
                &[("attacker", attacker), ("defender", defender)],
                &q_stats,
                "attacker",
            )
        }
        eval(&expr, app.world_mut(), attacker, defender)
    };

    assert_eq!(result, 10.0 * 2.0 + 7.0);
}

