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
        fn eval(expr: &Expression, world: &mut World, attacker: Entity, defender: Entity) -> f32 {
            StatsMutator::with_world(world, |mut sm| {
                sm.set(attacker, "Power", 10.0);
                sm.set(defender, "Power", 7.0);
                
                sm.with_stats_query(|q_stats| {
                    bevy_gauge::instant::evaluate_expression_with_roles(
                        expr,
                        &[("attacker", attacker), ("defender", defender)],
                        &q_stats,
                        "attacker",
                    )
                })
            })
        }
        eval(&expr, app.world_mut(), attacker, defender)
    };

    assert_eq!(result, 10.0 * 2.0 + 7.0);
}

