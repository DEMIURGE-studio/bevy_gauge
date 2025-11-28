
mod define_tags_impl;
mod named_impl;
mod proc_stats_impl;
mod simple_stat_derived_impl;
mod stat_component_impl;

#[proc_macro_derive(Named)]
pub fn derive_named(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    named_impl::derive_named(input)
}

#[proc_macro_derive(SimpleStatDerived)]
pub fn derive_simple_stat_derived(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    simple_stat_derived_impl::derive_simple_stat_derived(input)
}

#[proc_macro]
pub fn stat_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    stat_component_impl::stat_component(input)
}

#[proc_macro]
pub fn define_tags(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    define_tags_impl::define_tags(input)
}

#[proc_macro]
pub fn mod_set(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_stats_impl::mod_set(input)
}

#[proc_macro]
pub fn stats(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_stats_impl::stats(input)
}

#[proc_macro]
pub fn instant(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_stats_impl::instant(input)
}