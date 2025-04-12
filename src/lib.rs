#![feature(sync_unsafe_cell)]
#![feature(associated_type_defaults)]

// TODO ContextDrivenStats type that wraps stats, but contains a context (Hashmap of strings to entities). Can only call evaluate on it if you pass in a StatContextRefs

// TODO Stats.definitions should match String -> T where T implements StatLike. Convert the current StatType into DefaultStatType.

// TODO Systemetize asset-like definitions.
//     - get_total_expr_from_name
//     - get_initial_value_for_modifier
//     - match strings to sets of tags, i.e., "damage" -> Damage

// TODO wrapper for u32 that lets us conveniently do queries (HasTag, HasAny, HasAll). Possibly change ComplexModifiable to take type T where T implements TagLike

// TODO Implement fasteval instead of evalexpr

// TODO Build some examples 
//     - Path of Exile
//     - World of Warcraft
//     - Dungeons and Dragons
//     - Halo

// TODO Reintegrate with other stats code
//     - StatDerived
//     - Writeback

// TODO Some way to avoid parse::<u32>()

// TODO integrate string interning (string-interner vs lasso)

pub mod asset_like;
pub mod expressions;
pub mod modifier_set;
pub mod prelude;
pub mod stat_accessor;
pub mod stat_addressing;
pub mod stat_effect;
pub mod stat_error;
pub mod stat_like;
pub mod stat_requirements;
pub mod stat_types;
pub mod stats_component;
pub mod tags;
pub mod tests;