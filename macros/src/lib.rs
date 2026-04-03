mod attribute_component_impl;
mod define_tags_impl;
mod resolvable_impl;

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

/// Derive macro that generates [`AttributeDerived`] and/or [`WriteBack`]
/// implementations for a Bevy component, binding its fields to attributes.
///
/// Fields annotated with `#[read]` are read from attributes ([`AttributeDerived`]).
/// Fields annotated with `#[write]` are written back to attributes ([`WriteBack`]).
/// Fields without an annotation are plain struct fields.
///
/// The macro also emits `inventory::submit!` calls so that the component
/// is automatically registered with [`AttributesPlugin`] - no manual
/// `app.register_attribute_derived::<T>()` needed.
///
/// # Syntax
///
/// ```ignore
/// #[derive(Component, Default, AttributeComponent, Debug)]
/// pub struct Life {
///     #[read("Life")]
///     pub max: f32,              // read from "Life" attribute
///     #[write]
///     pub current: f32,          // write back to "Life.current" (auto-path)
///     pub label: String,         // plain field, not attribute-bound
/// }
/// ```
///
/// ## Path resolution
///
/// - `#[read("path")]` / `#[write("path")]` - explicit attribute path string
/// - `#[read]` / `#[write]` (no argument) - auto-path: `"StructName.field_name"`
///
/// [`AttributeDerived`]: bevy_gauge::derived::AttributeDerived
/// [`WriteBack`]: bevy_gauge::derived::WriteBack
/// [`AttributesPlugin`]: bevy_gauge::plugin::AttributesPlugin
/// Derive macro that generates [`AttributeResolvable`] for structs and enums.
///
/// Fields are resolved from attributes using dot-separated paths based on
/// field names. Use `#[skip]` to exclude fields from resolution.
///
/// # Rules
///
/// - **Named struct fields**: each appends `.field_name` to the prefix
/// - **Single resolvable field**: transparent — prefix resolves directly to the value
/// - **Enums**: variant name is transparent, fields within each variant follow struct rules
/// - **Newtypes**: transparent — prefix resolves to the inner value
/// - **Unit variants/structs**: no-op
/// - **Terminal types** (f32, integers, bool): read directly via `attrs.value()`
/// - **Other types**: delegate to their `AttributeResolvable` impl
///
/// # Example
///
/// ```ignore
/// #[derive(AttributeResolvable)]
/// struct DirectionOffset {
///     #[skip]
///     direction: Dir3,
///     magnitude: f32,  // single resolvable field → transparent
/// }
///
/// #[derive(AttributeResolvable)]
/// enum Gatherer {
///     Circle { radius: f32, count: u32 },
///     Sphere { radius: f32, count: u32 },
/// }
/// ```
///
/// [`AttributeResolvable`]: bevy_gauge::resolvable::AttributeResolvable
#[proc_macro_derive(AttributeResolvable, attributes(skip))]
pub fn derive_attribute_resolvable(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match resolvable_impl::derive(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(AttributeComponent, attributes(read, write, init_to, init_from))]
pub fn derive_attribute_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match attribute_component_impl::derive(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
