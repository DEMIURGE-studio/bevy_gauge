
pub fn get_total_expr_from_name(name: &str) -> &'static str {
    match name {
        "Damage" => "Added * Increased * More",
        "Life" => "Added * Increased * More",
        _ => "Added * Increased * More",
    }
}

pub fn get_initial_value_for_modifier(modifier_type: &str) -> f32 {
    match modifier_type {
        "Added" | "Base" | "Flat" => 0.0,
        "Increased" | "More" | "Multiplier" => 1.0,
        "Override" => 1.0,
        _ => 0.0,
    }
}