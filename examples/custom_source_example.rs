use bevy::prelude::*;
use bevy_gauge::prelude::*;

// --- Setup --- //

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_gauge::plugin)
        .insert_resource(app_config())
        .add_systems(Startup, (spawn_entities, apply_deferred).chain())
        .add_systems(
            Update,
            (
                register_parent_source_for_child,
                read_stats_system,
            )
                .chain(),
        )
        .run();
}

fn app_config() -> Config {
    let mut config = Config::default();

    // Parent's Strength (Modifiable)
    // No total_expression needed, defaults to its base value after modifiers.
    config.register_stat_type("Strength", "Modifiable");

    // Child's Bonus
    config.register_stat_type("ChildBonus", "Modifiable");

    config
}

fn spawn_entities(mut commands: Commands) {
    let parent = commands
        .spawn((
            Stats::new(), // Explicitly add Stats
            stats! { "Strength" => 50.0 }, // Initialize Strength for Modifiable stat
            Name::new("Parent"),
        ))
        .id();

    commands.spawn((
        Stats::new(), // Explicitly add Stats
        // ChildBonus is calculated based on Parent's Strength.
        // No specific initialization needed for ChildBonus parts here unless it had its own independent base.
        stats! { "ChildBonus" => "Parent@Strength * 0.1" },
        Name::new("Child"),
    )).set_parent(parent);

    println!("Entities spawned. Parent has 50 Strength.");
}

// --- Systems --- //

fn register_parent_source_for_child(
    mut stat_accessor: StatAccessor,
    child_query: Query<(Entity, &Parent), Changed<Parent>>,
) {
    for (child_entity, parent) in child_query.iter() {
        stat_accessor.register_source(
            child_entity,
            "MyParent", 
            parent.get(),
        );
    }
}

fn read_stats_system(
    child_query: Query<&Stats, With<Parent>>,
) {
    for child in child_query.iter() {
        let child_bonus = child.get("ChildBonus");
        println!("ChildBonus: {}", child_bonus);
    }
}