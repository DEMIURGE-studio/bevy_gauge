//! Resolve the path to the `bevy_gauge` runtime crate at macro-expansion time.
//!
//! Derives emit code that names gauge's runtime types. A consumer might depend
//! on `bevy_gauge` directly, or reach it re-exported through the `bevy_diesel`
//! umbrella (`bevy_diesel::gauge`). Following Bevy's `BevyManifest` pattern, we
//! prefer the umbrella when it is a dependency and fall back to the leaf crate
//! otherwise, so standalone `bevy_gauge` users are unaffected.
//!
//! `FoundCrate::Itself` means "the crate being compiled is the one queried" -
//! which differs per branch: in the umbrella branch it means we are compiling
//! `bevy_diesel` (so name it, `::bevy_diesel::gauge`), in the leaf branch it
//! means we are compiling `bevy_gauge`. We name the crate rather than emitting
//! `crate`, so the path also resolves in examples/tests (where `crate` is the
//! example, not the lib); each crate carries `extern crate self as …;` to make
//! that name resolve internally.

use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::Ident;

fn named(name: &str) -> TokenStream {
    let ident = Ident::new(name, Span::call_site());
    quote! { ::#ident }
}

/// Path prefix for gauge's runtime crate: `bevy_diesel::gauge` when the umbrella
/// is present, otherwise `bevy_gauge`.
pub(crate) fn gauge_root() -> TokenStream {
    // Umbrella: reached through the bevy_diesel crate.
    match crate_name("bevy_diesel") {
        Ok(FoundCrate::Itself) => return quote! { ::bevy_diesel::gauge },
        Ok(FoundCrate::Name(name)) => {
            let base = named(&name);
            return quote! { #base::gauge };
        }
        Err(_) => {}
    }
    // Leaf: bevy_gauge directly (or compiling bevy_gauge itself).
    match crate_name("bevy_gauge") {
        Ok(FoundCrate::Itself) => quote! { ::bevy_gauge },
        Ok(FoundCrate::Name(name)) => named(&name),
        Err(_) => quote! { ::bevy_gauge },
    }
}
