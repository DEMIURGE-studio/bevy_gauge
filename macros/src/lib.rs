mod attribute_component_impl;

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
/// [`AttributeDerived`]: bevy_attributes::derived::AttributeDerived
/// [`WriteBack`]: bevy_attributes::derived::WriteBack
/// [`AttributesPlugin`]: bevy_attributes::plugin::AttributesPlugin
#[proc_macro]
pub fn attribute_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    attribute_component_impl::attribute_component(input)
}
