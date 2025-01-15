use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, token, Ident, Token};
use syn::parse::{Parse, ParseStream};

#[proc_macro_derive(Named)]
pub fn derive_named(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl Named for #name {
            const NAME: &'static str = stringify!(#name);
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(SimpleStatDerived)]
pub fn derive_simple_stat_derived(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl bevy_guage::prelude::StatDerived for #name {
            fn from_stats(stats: &bevy_guage::prelude::Stats) -> Self {
                let value = stats.get(Self::NAME).unwrap_or(0.0);
                return Self(value);
            }
        
            fn update_from_stats(&mut self, stats: &bevy_guage::prelude::Stats) {
                let value = stats.get(Self::NAME).unwrap_or(0.0);
                self.0 = value;
            }

            fn is_valid(stats: &bevy_guage::prelude::Stats) -> bool {
                stats.get(Self::NAME).is_ok()
            }
        }
    };

    TokenStream::from(expanded)
}

use syn::{
    punctuated::Punctuated, 
    FieldsNamed, ItemStruct,
};

/// Our input to the `complex_stat_derived!` macro looks like:
///
///  #[derive(Component, Default)]
///  struct SelfExplosionEffect<T> {
///      pub area: f32,
///      pub damage: f32,
///      _pd: PhantomData<T>,
///  };
/// 
///  (OnBlock, OnAttack)
///
/// Or (optionally) we can omit the parenthesized list entirely if we have no “cue” types.
struct ComplexStatInput {
    item_struct: ItemStruct,
    _semi_token: Token![;],

    /// Optional list of “cue” idents from a parenthesized block
    optional_generic_idents: Option<Punctuated<Ident, Token![,]>>,
}

/// Implement `Parse` so `syn` can parse our macro input.
impl Parse for ComplexStatInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 1) Parse the struct definition (including attributes, generics, fields, etc.)
        let item_struct: ItemStruct = input.parse()?;

        // 2) Expect a semicolon after the struct
        let _semi_token: Token![;] = input.parse()?;

        // 3) Check if we have an opening parenthesis for the cue list
        let optional_generic_idents = if input.peek(token::Paren) {
            // If we see `(`, parse the parenthesized list of generic types, e.g. `(OnBlock, OnAttack)`
            let content;
            syn::parenthesized!(content in input);
            let generic_idents: Punctuated<Ident, Token![,]> =
                content.parse_terminated(Ident::parse, Token![,])?;
            Some(generic_idents)
        } else {
            // No parenthesized list found
            None
        };

        Ok(Self {
            item_struct,
            _semi_token,
            optional_generic_idents,
        })
    }
}

#[proc_macro]
pub fn complex_stat_derived(input: TokenStream) -> TokenStream {
    let ComplexStatInput {
        item_struct,
        optional_generic_idents,
        ..
    } = parse_macro_input!(input as ComplexStatInput);

    let struct_ident= &item_struct.ident;
    let struct_generics = &item_struct.generics;

    // Must be a named-fields struct
    let fields = match &item_struct.fields {
        syn::Fields::Named(FieldsNamed { named, .. }) => named,
        _ => {
            return syn::Error::new_spanned(
                &item_struct.fields,
                "complex_stat_derived! requires a struct with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Gather field idents, excluding those starting with '_'
    let visible_field_idents: Vec<&Ident> = fields
        .iter()
        .filter_map(|f| {
            let ident = f.ident.as_ref().unwrap();
            if ident.to_string().starts_with('_') {
                None
            } else {
                Some(ident)
            }
        })
        .collect();

    // 1) Re-emit the struct exactly as the user wrote it
    let struct_def = quote! {
        #item_struct
    };

    // We’ll collect expansions into a Vec
    let mut expanded_impls = Vec::new();

    // ----------------------------------------------------------
    // CASE A: We have an explicit list of cue types (non-empty)
    // ----------------------------------------------------------
    if let Some(generic_idents) = optional_generic_idents {
        // If the user provided something like (OnBlock, OnAttack),
        // we generate an impl for each of those cues.
        for cue_ident in generic_idents {
            // Build string expressions only for visible fields
            let field_string_exprs = visible_field_idents.iter().map(|field_name| {
                // e.g. "SelfExplosionEffect<OnBlock>.damage"
                quote! {
                    concat!(
                        stringify!(#struct_ident), "<",
                        stringify!(#cue_ident), ">.",
                        stringify!(#field_name)
                    )
                }
            });

            let set_match_arms = visible_field_idents.iter().map(|field_name| {
                quote! {
                    concat!(
                        stringify!(#struct_ident), "<",
                        stringify!(#cue_ident), ">.",
                        stringify!(#field_name)
                    ) => {
                        self.#field_name = value;
                    }
                }
            });

            // impl Fields
            let fields_impl = quote! {
                impl bevy_guage::prelude::Fields for #struct_ident<#cue_ident> {
                    const FIELDS: &'static [&'static str] = &[
                        #( #field_string_exprs ),*
                    ];

                    fn set(&mut self, field: &str, value: f32) {
                        match field {
                            #( #set_match_arms ),*,
                            _ => (),
                        }
                    }
                }
            };

            // impl StatDerived
            let statderived_impl = quote! {
                impl bevy_guage::prelude::StatDerived for #struct_ident<#cue_ident> {
                    fn from_stats(stats: &bevy_guage::prelude::Stats) -> Self {
                        let mut s = Self::default();
                        s.update_from_stats(stats);
                        s
                    }

                    fn update_from_stats(&mut self, stats: &bevy_guage::prelude::Stats) {
                        for &field in <Self as bevy_guage::prelude::Fields>::FIELDS {
                            let value = stats.get(field).unwrap_or(0.0);
                            bevy_guage::prelude::Fields::set(self, field, value);
                        }
                    }

                    fn is_valid(stats: &bevy_guage::prelude::Stats) -> bool {
                        for &field in <Self as bevy_guage::prelude::Fields>::FIELDS {
                            if stats.get(field).is_ok() {
                                return true;
                            }
                        }
                        false
                    }
                }
            };

            expanded_impls.push(fields_impl);
            expanded_impls.push(statderived_impl);
        }
    }
    // ----------------------------------------------------------
    // CASE B: No parenthesized list => generate *one* set of impl
    // ----------------------------------------------------------
    else {
        // Build string expressions only for visible fields, with *no* "<Cue>"
        let field_string_exprs = visible_field_idents.iter().map(|field_name| {
            // e.g. "SelfExplosionEffect.damage"
            // or just "Life.max"
            quote! {
                concat!(
                    stringify!(#struct_ident), ".",
                    stringify!(#field_name)
                )
            }
        });

        let set_match_arms = visible_field_idents.iter().map(|field_name| {
            // e.g. "SelfExplosionEffect.damage" => self.damage = value
            quote! {
                concat!(
                    stringify!(#struct_ident), ".",
                    stringify!(#field_name)
                ) => {
                    self.#field_name = value;
                }
            }
        });

        // impl Fields
        let fields_impl = quote! {
            impl bevy_guage::prelude::Fields for #struct_ident #struct_generics {
                const FIELDS: &'static [&'static str] = &[
                    #( #field_string_exprs ),*
                ];

                fn set(&mut self, field: &str, value: f32) {
                    match field {
                        #( #set_match_arms ),*,
                        _ => (),
                    }
                }
            }
        };

        // impl StatDerived
        let statderived_impl = quote! {
            impl bevy_guage::prelude::StatDerived for #struct_ident #struct_generics {
                fn from_stats(stats: &bevy_guage::prelude::Stats) -> Self {
                    let mut s = Self::default();
                    s.update_from_stats(stats);
                    s
                }

                fn update_from_stats(&mut self, stats: &bevy_guage::prelude::Stats) {
                    for &field in <Self as bevy_guage::prelude::Fields>::FIELDS {
                        let value = stats.get(field).unwrap_or(0.0);
                        bevy_guage::prelude::Fields::set(self, field, value);
                    }
                }

                fn is_valid(stats: &bevy_guage::prelude::Stats) -> bool {
                    for &field in <Self as bevy_guage::prelude::Fields>::FIELDS {
                        if stats.get(field).is_ok() {
                            return true;
                        }
                    }
                    false
                }
            }
        };

        expanded_impls.push(fields_impl);
        expanded_impls.push(statderived_impl);
    }

    // Combine everything into one final token stream
    let tokens = quote! {
        // 1) Re-emit the user’s struct exactly
        #struct_def

        // 2) The impl(s)
        #( #expanded_impls )*
    };

    tokens.into()
}