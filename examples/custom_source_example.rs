use bevy::prelude::*;
use bevy_gauge::prelude::*;

// --- Setup --- //

fn main() {
    app_config();
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_gauge::plugin)
        .add_systems(Startup, (spawn_entities, ApplyDeferred).chain())
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

fn app_config() {
    // Parent's Strength (Modifiable)
    // No total_expression needed, defaults to its base value after modifiers.
    Konfig::register_stat_type("Strength", "Modifiable");

    // Child's Bonus
    Konfig::register_stat_type("ChildBonus", "Modifiable");
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
    )).insert(ChildOf(parent));

    println!("Entities spawned. Parent has 50 Strength.");
}

// --- Systems --- //

fn register_parent_source_for_child(
    mut stats_mutator: StatsMutator,
    child_query: Query<(Entity, &ChildOf), Changed<ChildOf>>,
) {
    for (child_entity, parent) in child_query.iter() {
        stats_mutator.register_source(
            child_entity,
            "MyParent", 
            parent.parent(),
        );
    }
}

fn read_stats_system(
    child_query: Query<&Stats, With<ChildOf>>,
) {
    for child in child_query.iter() {
        let child_bonus = child.get("ChildBonus");
        println!("ChildBonus: {}", child_bonus);
    }
}