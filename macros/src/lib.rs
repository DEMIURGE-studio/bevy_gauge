mod attribute_component_impl;
mod define_tags_impl;

/// Declare a unit struct with [`TagMask`] associated constants for a tag
/// hierarchy, plus a `register(&mut TagResolver)` method that registers every
/// tag name with the resolver.
///
/// # Syntax
///
/// ```ignore
/// define_tags! {
///     DamageTags,
///     damage_type {
///         elemental { fire, cold, lightning },
///         physical,
///         chaos,
///     },
///     weapon_type {
///         melee { sword, axe },
///         ranged { bow, wand },
///     },
/// }
/// ```
///
/// This generates:
///
/// ```ignore
/// pub struct DamageTags;
/// impl DamageTags {
///     pub const FIRE: TagMask = TagMask::bit(0);
///     // ...
///     pub const ELEMENTAL: TagMask = TagMask::new(Self::FIRE.0 | Self::COLD.0 | Self::LIGHTNING.0);
///     // ...
///     pub fn register(resolver: &mut TagResolver) { /* ... */ }
/// }
/// ```
///
/// [`TagMask`]: bevy_gauge::tags::TagMask
/// [`TagResolver`]: bevy_gauge::tags::TagResolver
#[proc_macro]
pub fn define_tags(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    define_tags_impl::define_tags(input)
}

/// Generate a Bevy component whose fields are bound to attributes.
///
/// Fields marked with `<-` are read from attributes ([`AttributeDerived`]).
/// Fields marked with `->` are written back to attributes ([`WriteBack`]).
/// Fields without an arrow are plain struct fields.
///
/// The macro also emits `inventory::submit!` calls so that the component
/// is automatically registered with [`AttributesPlugin`] — no manual
/// `app.register_attribute_derived::<T>()` needed.
///
/// # Syntax
///
/// ```ignore
/// attribute_component! {
///     #[derive(Debug)]
///     pub struct Life {
///         pub max: f32 <- "Life",         // read from "Life" attribute
///         pub current: f32 -> $,          // write back to "Life.current"
///         pub label: String,              // plain field, not attribute-bound
///     }
/// }
/// ```
///
/// ## Path resolution
///
/// - `"AttributePath"` — explicit attribute path string
/// - `$` — auto-path: `"StructName.field_name"` (e.g. `"Life.current"`)
///
/// [`AttributeDerived`]: bevy_gauge::derived::AttributeDerived
/// [`WriteBack`]: bevy_gauge::derived::WriteBack
/// [`AttributesPlugin`]: bevy_gauge::plugin::AttributesPlugin
#[proc_macro]
pub fn attribute_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    attribute_component_impl::attribute_component(input)
}
