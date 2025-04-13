use quote::{format_ident, quote};
use syn::token::{Brace, Paren, Semi};
use syn::{braced, parse_macro_input, Attribute, Ident, Token, Visibility};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

#[proc_macro_derive(Named)]
pub fn derive_named(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl Named for #name {
            const NAME: &'static str = stringify!(#name);
        }
    };

    proc_macro::TokenStream::from(expanded).into()
}

#[proc_macro_derive(SimpleStatDerived)]
pub fn derive_simple_stat_derived(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl bevy_gauge::prelude::StatDerived for #name {
            fn from_stats(stats: &bevy_gauge::prelude::StatContextRefs) -> Self {
                let value = stats.get(Self::NAME).unwrap_or(0.0);
                return Self(value);
            }
            
            fn should_update(&self, stats: &StatContextRefs) -> bool {
                true
            }
        
            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::StatContextRefs) {
                let value = stats.get(Self::NAME).unwrap_or(0.0);
                self.0 = value;
            }

            fn is_valid(stats: &bevy_gauge::prelude::StatContextRefs) -> bool {
                stats.get(Self::NAME).is_ok()
            }
        }
    };

    proc_macro::TokenStream::from(expanded).into()
}

#[proc_macro]
pub fn stat_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as StatStructInput);
    let expanded = match ast.expand() {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error(),
    };
    expanded.into()
}

/// The entire macro input, including outer attributes, generics, optional variants, etc.
struct StatStructInput {
    /// e.g. `#[derive(Debug)]` or others the user might have typed above the struct.
    attrs: Vec<Attribute>,

    vis: Visibility,
    _struct_token: Token![struct],
    ident: Ident,
    generics: syn::Generics,
    _brace_token: Brace,
    fields: Punctuated<StatField, Token![,]>,
    _semi_token: Option<Semi>,

    /// e.g. `(OnBlock, OnMeditate)`
    variants: Option<Punctuated<Ident, Token![,]>>,
}

/// One field in the user’s DSL, e.g.
/// 
/// ```plain
///   foo: ..,
///   bar: WriteBack,
///   nested: SomeType {
///       x: ..,
///       y: ..,
///   }
/// ```
enum StatField {
    Derived {
        name: Ident,
        _colon_token: Token![:],
        _dots_token: Token![..],
    },
    WriteBack {
        name: Ident,
        _colon_token: Token![:],
        _writeback_ident: Ident,
    },
    DerivedWriteBack {
        name: Ident,
        _colon_token: Token![:],
        _dots_token: Token![..],
        _writeback_ident: Ident,
    },
    Nested {
        name: Ident,
        _colon_token: Token![:],
        type_name: Ident,
        _brace_token: Brace,
        nested_fields: Punctuated<StatField, Token![,]>,
    },
}

/// Represents one field of the top-level struct or a nested struct:
///   `field_name : ..` => Derived
///   `field_name : WriteBack` => WriteBack
///   `field_name : SomeType { ... }` => Nested
#[derive(Debug)]
enum ParsedField {
    Derived { name: Ident },
    WriteBack { name: Ident },
    DerivedWriteBack { name: Ident },  // new
    Nested { 
        name: Ident,
        type_name: Ident,
        fields: Vec<ParsedField>
    },
}

// ---------------------------------------------------------------------
// 2) Parsing
// ---------------------------------------------------------------------
impl Parse for StatStructInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Collect user-supplied outer attributes
        let attrs = input.call(Attribute::parse_outer)?;

        let vis: Visibility = input.parse()?;
        let struct_token: Token![struct] = input.parse()?;
        let ident: Ident = input.parse()?;
        let generics: syn::Generics = input.parse()?;

        // parse the brace for the fields
        let content;
        let brace_token = syn::braced!(content in input);
        let fields = content.parse_terminated(StatField::parse, Token![,])?;

        let semi_token = if input.peek(Token![;]) {
            Some(input.parse()?)
        } else {
            None
        };

        // parse optional `(VariantA, VariantB, ...)`
        let variants = if input.peek(Paren) {
            let content2;
            syn::parenthesized!(content2 in input);
            Some(content2.parse_terminated(Ident::parse, Token![,])?)
        } else {
            None
        };

        Ok(StatStructInput {
            attrs,
            vis,
            _struct_token: struct_token,
            ident,
            generics,
            _brace_token: brace_token,
            fields,
            _semi_token: semi_token,
            variants,
        })
    }
}

impl Parse for StatField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let colon_token: Token![:] = input.parse()?;

        // Check if next token is `..`
        if input.peek(Token![..]) {
            let dots_token: Token![..] = input.parse()?;

            // Now see if the user wrote `..WriteBack` or just `..`
            if input.peek(Ident) {
                let maybe_wb: Ident = input.parse()?;
                if maybe_wb == "WriteBack" {
                    // => DerivedWriteBack
                    return Ok(StatField::DerivedWriteBack {
                        name,
                        _colon_token: colon_token,
                        _dots_token: dots_token,
                        _writeback_ident: maybe_wb,
                    });
                } else {
                    return Err(input.error("expected `WriteBack` after `..`"));
                }
            }

            // If there's no `Ident` after `..`, then it's the normal Derived
            return Ok(StatField::Derived {
                name,
                _colon_token: colon_token,
                _dots_token: dots_token,
            });
        }
        else if input.peek(Ident) {
            // Possibly "WriteBack" or "TypeName { ... }" (Nested)
            let ident2: Ident = input.parse()?;
            if ident2 == "WriteBack" {
                return Ok(StatField::WriteBack {
                    name,
                    _colon_token: colon_token,
                    _writeback_ident: ident2,
                });
            } else {
                // nested
                let content;
                let brace_token = syn::braced!(content in input);
                let nested_fields = content.parse_terminated(StatField::parse, Token![,])?;
                return Ok(StatField::Nested {
                    name,
                    _colon_token: colon_token,
                    type_name: ident2,
                    _brace_token: brace_token,
                    nested_fields,
                });
            }
        }

        Err(input.error("expected `..`, `..WriteBack`, `WriteBack`, or `TypeName { ... }`"))
    }
}

// ---------------------------------------------------------------------
// 3) Expanding
// ---------------------------------------------------------------------
impl StatStructInput {
    pub fn expand(&self) -> syn::Result<proc_macro2::TokenStream> {
        // parse fields into a Vec<ParsedField>
        let parsed_fields = parse_fields_list(&self.fields)?;

        // Step A) Build exactly ONE generic struct definition—e.g. `Generic<T>`.
        //         (Add or merge user attributes, plus always add some of your own.)
        let struct_code = expand_single_struct_def(
            &self.attrs,
            &self.vis,
            &self.ident,
            &self.generics,
            &parsed_fields,
        )?;

        // Step B) For each variant, create specialized trait impls
        let variants_code = if let Some(variant_idents) = &self.variants {
            let mut v_impls = Vec::new();
            for v in variant_idents {
                let impls = expand_trait_impls_for_variant(
                    &self.ident,
                    v,
                    &parsed_fields,
                );
                v_impls.push(impls);
            }
            quote! { #(#v_impls)* }
        } else {
            // If no variants, just generate an impl for "impl StatDerived for Simple" (or whatever the struct name is).
            expand_trait_impls_for_no_variant(&self.ident, &self.generics, &parsed_fields)
        };

        Ok(quote! {
            #struct_code
            #variants_code
        })
    }
}

/// Build the single struct definition with user + forced attributes,
/// plus a `_pd: PhantomData<T>` field if generics are present.
fn expand_single_struct_def(
    user_attrs: &[Attribute],
    vis: &Visibility,
    ident: &Ident,
    generics: &syn::Generics,
    fields: &[ParsedField],
) -> syn::Result<proc_macro2::TokenStream> {
    // We'll forcibly add `#[derive(Component, Default, Debug)]`.
    // If you want to merge with user-specified derives, you can parse them carefully.
    // For now, let's just push them on top:
    // (If you want to allow user to override them, you'd do something more fancy.)
    let forced_attrs = quote! {
        #[derive(::bevy::prelude::Component, ::std::default::Default, ::std::fmt::Debug)]
    };

    // For the struct fields:
    //  - Derived/WriteBack => `pub name: f32`
    //  - Nested => `pub name: TheNestedType`
    //  plus the `_pd: ::std::marker::PhantomData<T>` if there's at least one generic param
    let field_defs = fields.iter().map(|f| {
        match f {
            ParsedField::Derived { name } => {
                quote! { pub #name: f32 }
            }
            ParsedField::WriteBack { name } => {
                quote! { pub #name: f32 }
            }
            ParsedField::Nested { name, type_name, .. } => {
                quote! { pub #name: #type_name }
            }
            ParsedField::DerivedWriteBack { name } => {
                quote! { pub #name: f32}
            },
        }
    });

    let has_generics = !generics.params.is_empty();
    let phantom_field = if has_generics {
        // `_pd: PhantomData<T>` or if multiple generics, you might do PhantomData<(A,B,...)>.
        // For simplicity let's assume one T, or treat them all as a tuple.
        let generic_params_as_tuple = build_phantom_tuple(generics);
        quote! {
            pub _pd: ::std::marker::PhantomData<#generic_params_as_tuple>
        }
    } else {
        quote! {}
    };

    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote! {
        // user-supplied attributes, if any
        #(#user_attrs)*

        // forced attributes
        #forced_attrs

        #vis struct #ident #impl_generics #where_clause {
            #(#field_defs),*,
            #phantom_field
        }
    })
}

/// Expand specialized impls for e.g. `Generic<OnBlock>`:
///
/// - `impl StatDerived for Generic<OnBlock> { ... }`
/// - `impl WriteBack for Generic<OnBlock> { ... }`
fn expand_trait_impls_for_variant(
    struct_ident: &Ident,
    variant_ident: &Ident,
    fields: &[ParsedField],
) -> proc_macro2::TokenStream {
    // We'll produce code that says `impl StatDerived for #struct_ident<#variant_ident>`.
    // That means we effectively replace T with the variant type in the path strings.
    //
    // If you have multiple generics, you’ll need to do something more involved:
    //   e.g. `impl<A, B> StatDerived for Generic<A, B, OnBlock>` or something similar.
    // For now, let's assume there's only one generic param T.

    // Rebuild generics for the *impl* line. We'll use all the same generics except T.
    // But if your struct has exactly 1 type param T, and you're overriding T with
    // `variant_ident`, you might do it like:
    //    impl StatDerived for #struct_ident<#variant_ident> { ... }
    //
    // If you have other generics besides T, you’d want to keep them. Example:
    //    impl<U> StatDerived for #struct_ident<U, OnBlock> { ... }
    // For simplicity, we’ll pretend the user only has `<T>`.

    let struct_name_with_variant = quote! { #struct_ident<#variant_ident> };

    // The path prefix for e.g. `"Generic<OnBlock>"`.
    // If you want to handle multiple type params, you'll need more robust string building.
    let path_prefix_str = format!("{}<{}>", struct_ident, variant_ident);

    let should_update_body = collect_should_update_lines_with_prefix(fields, &path_prefix_str, quote!(self));

    // Build the body of `update_from_stats` (only for derived fields).
    let update_body = collect_update_lines_with_prefix(fields, &path_prefix_str, quote!(self));

    let is_valid_body = collect_is_valid_lines_with_prefix(fields, &path_prefix_str, quote!(self));

    // Build the body of `write_back` (only for writeback fields).
    let wb_body = collect_writeback_lines_with_prefix(fields, &path_prefix_str, quote!(self));

    quote! {
        impl StatDerived for #struct_name_with_variant {
            fn from_stats(stats: &StatContextRefs) -> Self {
                let mut s = Self::default();
                s.update_from_stats(stats);
                s
            }
            fn should_update(&self, stats: &StatContextRefs) -> bool {
                #should_update_body
            }
            fn update_from_stats(&mut self, stats: &StatContextRefs) {
                #update_body
            }
            fn is_valid(stats: &StatContextRefs) -> bool {
                #is_valid_body
            }
        }

        impl WriteBack for #struct_name_with_variant {
            fn write_back(&self, stats: &mut StatDefinitions) {
                #wb_body
            }
        }
    }
}

// Helper to build `_pd: PhantomData<(A, B, ...)>` from generics if there are multiple.
fn build_phantom_tuple(generics: &syn::Generics) -> proc_macro2::TokenStream {
    // Collect each type param as a token, e.g. T, U, V
    let params = generics.params.iter().map(|gp| {
        match gp {
            syn::GenericParam::Type(t) => {
                let ident = &t.ident;
                quote!( #ident )
            },
            syn::GenericParam::Lifetime(l) => {
                // rarely want phantom for lifetimes, but let's just skip it
                let lt = &l.lifetime;
                quote!(&#lt ())
            },
            syn::GenericParam::Const(c) => {
                let ident = &c.ident;
                quote!( #ident )
            }
        }
    });
    quote! { (#(#params),*) }
}

// ---------------------------------------------------------------------
// 4) Generating code for `update_from_stats` / `write_back`
//    with a prefix like "Generic<OnBlock>" in the paths
// ---------------------------------------------------------------------

fn parse_fields_list(fields: &Punctuated<StatField, Token![,]>) -> syn::Result<Vec<ParsedField>> {
    let mut results = Vec::new();
    for f in fields {
        let pf = match f {
            StatField::Derived { name, .. } => ParsedField::Derived { name: name.clone() },
            StatField::WriteBack { name, .. } => ParsedField::WriteBack { name: name.clone() },
            StatField::DerivedWriteBack { name, .. } => ParsedField::DerivedWriteBack {
                name: name.clone(),
            },
            StatField::Nested { name, type_name, nested_fields, .. } => {
                let sub = parse_fields_list(nested_fields)?;
                ParsedField::Nested {
                    name: name.clone(),
                    type_name: type_name.clone(),
                    fields: sub,
                }
            }
        };
        results.push(pf);
    }
    Ok(results)
}

/// Recursively build statements for `update_from_stats` that do:
/// `self.name = stats.get("Prefix.name").unwrap_or(0.0)` for derived fields.
/// For nested fields, recursively build lines with `Prefix.nested.some_subfield`.
fn collect_update_lines_with_prefix(
    fields: &[ParsedField],
    prefix: &str,
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::Derived { name } => {
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    #self_expr.#name = stats.get(#path_str).unwrap_or(0.0);
                });
            }
            ParsedField::DerivedWriteBack { name } => {
                // same as Derived
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    #self_expr.#name = stats.get(#path_str).unwrap_or(0.0);
                });
            }
            ParsedField::WriteBack { .. } => {
                // skip
            }
            ParsedField::Nested { name, fields, .. } => {
                let new_prefix = format!("{}.{}", prefix, name);
                let new_self = quote!( #self_expr.#name );
                let nested_code = collect_update_lines_with_prefix(fields, &new_prefix, new_self);
                lines.push(nested_code);
            }
        }
    }

    quote! { #(#lines)* }
}

fn collect_should_update_lines_with_prefix(
    fields: &[ParsedField],
    prefix: &str,
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::Derived { name } => {
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    #self_expr.#name != stats.get(#path_str).unwrap_or(0.0)
                });
            }
            ParsedField::DerivedWriteBack { name } => {
                // same as Derived
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    #self_expr.#name != stats.get(#path_str).unwrap_or(0.0)
                });
            }
            ParsedField::WriteBack { .. } => { /* skip in check? or you can do something if you want */ }
            ParsedField::Nested { name, fields, .. } => {
                let new_prefix = format!("{}.{}", prefix, name);
                let nested_code = collect_should_update_lines_with_prefix(fields, &new_prefix, 
                    quote!(#self_expr.#name));
                lines.push(nested_code);
            }
        }
    }

    // Combine them with OR:
    quote! { #(#lines)||* }
}

/// Recursively build statements for `write_back` that do:
/// `stats.set("Prefix.name", self.name)` for writeback fields.
fn collect_writeback_lines_with_prefix(
    fields: &[ParsedField],
    prefix: &str,
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::WriteBack { name } => {
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    let _ = stats.set(#path_str, #self_expr.#name);
                });
            }
            ParsedField::DerivedWriteBack { name } => {
                // same as WriteBack
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    let _ = stats.set(#path_str, #self_expr.#name);
                });
            }
            ParsedField::Derived { .. } => {
                // skip
            }
            ParsedField::Nested { name, fields, .. } => {
                let new_prefix = format!("{}.{}", prefix, name);
                let nested_code = collect_writeback_lines_with_prefix(fields, &new_prefix, 
                    quote!(#self_expr.#name));
                lines.push(nested_code);
            }
        }
    }

    quote! { #(#lines)* }
}

fn collect_is_valid_lines_with_prefix(
    fields: &[ParsedField],
    prefix: &str,
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();
    for pf in fields {
        match pf {
            ParsedField::Derived { name } => {
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    stats.get(#path_str).is_ok()
                });
            }
            ParsedField::DerivedWriteBack { name } => {
                // same as Derived or WriteBack
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    stats.get(#path_str).is_ok()
                });
            }
            ParsedField::WriteBack { name } => {
                let path_str = format!("{}.{}", prefix, name);
                lines.push(quote! {
                    stats.get(#path_str).is_ok()
                });
            }
            ParsedField::Nested { name, fields, .. } => {
                let new_prefix = format!("{}.{}", prefix, name);
                let nested_code = collect_is_valid_lines_with_prefix(fields, &new_prefix,
                    quote!(#self_expr.#name));
                lines.push(nested_code);
            }
        }
    }
    // Combine them with OR or AND, depending on your desired semantics
    quote! { #(#lines)||* }
}

fn expand_trait_impls_for_no_variant(
    struct_ident: &Ident,
    generics: &syn::Generics,
    fields: &[ParsedField],
) -> proc_macro2::TokenStream {
    // e.g. `impl StatDerived for Simple`
    // the path is just "Simple" instead of "Simple<SomeVariant>".
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // So the path string becomes just `"Simple"` or whatever the name is.
    let path_prefix_str = struct_ident.to_string();

    let should_update_body = collect_should_update_lines_with_prefix(fields, &path_prefix_str, quote!(self));
    let update_body = collect_update_lines_with_prefix(fields, &path_prefix_str, quote!(self));
    let writeback_body = collect_writeback_lines_with_prefix(fields, &path_prefix_str, quote!(self));
    let is_valid_body = collect_is_valid_lines_with_prefix(fields, &path_prefix_str, quote!(self));

    quote! {
        impl #impl_generics StatDerived for #struct_ident #ty_generics #where_clause {
            fn from_stats(stats: &StatContextRefs) -> Self {
                let mut s = Self::default();
                s.update_from_stats(stats);
                s
            }
            fn should_update(&self, stats: &StatContextRefs) -> bool {
                #should_update_body
            }
            fn update_from_stats(&mut self, stats: &StatContextRefs) {
                #update_body
            }
            fn is_valid(stats: &StatContextRefs) -> bool {
                #is_valid_body
            }
        }

        impl #impl_generics WriteBack for #struct_ident #ty_generics #where_clause {
            fn write_back(&self, stats: &mut StatDefinitions) {
                #writeback_body
            }
        }
    }
}

use syn::token::Comma;

/// A tag node with a name and optional children.
struct Tag {
    name: Ident,
    children: Vec<Tag>,
}

impl Parse for Tag {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        // Parse the tag name (an identifier).
        let name: Ident = input.parse()?;
        let children = if input.peek(syn::token::Brace) {
            // Capture the content inside the braces.
            let content;
            let _brace_token = braced!(content in input);
            // Parse a comma-separated list of child tags.
            let child_list: Punctuated<Tag, Comma> =
                content.parse_terminated(Tag::parse, Comma)?;
            child_list.into_iter().collect()
        } else {
            Vec::new()
        };
        Ok(Tag { name, children })
    }
}

/// Root level container for multiple categories
struct TagRoot {
    categories: Punctuated<Tag, Comma>,
}

impl Parse for TagRoot {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let categories = input.parse_terminated(Tag::parse, Comma)?;
        Ok(TagRoot { categories })
    }
}

// A structure to hold information about each tag
struct TagInfo {
    name: String,
    const_name: String,
    bit_expr: proc_macro2::TokenStream,
    parent_category: Option<String>,
}

/// The procedural macro definition.
#[proc_macro]
pub fn define_tags(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input as multiple top-level categories
    let root = parse_macro_input!(input as TagRoot);
    
    // We use a counter to assign unique bits to each leaf.
    let mut counter = 0u32;
    let mut constants = Vec::new();
    let mut match_arms = Vec::new();
    let mut tag_info_list: Vec<TagInfo> = Vec::new();
    
    // Process each top-level category
    for category in &root.categories {
        let category_name = category.name.to_string();
        let category_const = format_ident!("{}", category_name.to_uppercase());
        
        // Process this category and its children
        let category_expr = gen_constants(
            category, 
            &mut counter, 
            &mut constants, 
            &mut match_arms, 
            &mut tag_info_list, 
            None
        );
        
        // Store the category itself
        tag_info_list.push(TagInfo {
            name: category_name,
            const_name: category_const.to_string(),
            bit_expr: category_expr.clone(),
            parent_category: None,
        });
    }
    
    // Create category mapping arms
    let mut category_arms = Vec::new();
    for info in &tag_info_list {
        if let Some(parent) = &info.parent_category {
            let tag_const = format_ident!("{}", info.const_name);
            let parent_const = format_ident!("{}", parent);
            category_arms.push(quote! {
                #tag_const => #parent_const
            });
        }
    }
    
    // Build the output tokens as regular functions, not in a module
    let expanded = quote! {
        // Generated constant definitions.
        #(#constants)*
        
        // Match a tag string to its constant.
        pub fn match_tag(tag: &str) -> u32 {
            match tag {
                #(#match_arms),*,
                _ => 0,
            }
        }
        
        // Match a tag constant to its category constant.
        pub fn tag_category(tag: u32) -> u32 {
            match tag {
                #(#category_arms),*,
                // For any unrecognized tag or categories themselves, return the tag
                tag => tag,
            }
        }
    };
    
    expanded.into()
}

/// Recursively generates constant definitions and tracking tag hierarchy info.
fn gen_constants(
    tag: &Tag,
    counter: &mut u32,
    constants: &mut Vec<proc_macro2::TokenStream>,
    match_arms: &mut Vec<proc_macro2::TokenStream>,
    tag_info_list: &mut Vec<TagInfo>,
    parent_category: Option<String>,
) -> proc_macro2::TokenStream {
    // Convert the tag name (e.g. "fire") to uppercase (e.g. FIRE) for the constant.
    let name_str = tag.name.to_string();
    let const_ident = format_ident!("{}", name_str.to_uppercase());
    let const_name = const_ident.to_string();
    
    if tag.children.is_empty() {
        // This is a leaf: assign a unique bit.
        let bit = *counter;
        *counter += 1;
        let expr = quote! { 1 << #bit };
        
        constants.push(quote! {
            pub const #const_ident: u32 = #expr;
        });
        
        match_arms.push(quote! {
            #name_str => #const_ident
        });
        
        // Store tag info
        tag_info_list.push(TagInfo {
            name: name_str,
            const_name,
            bit_expr: expr.clone(),
            parent_category,
        });
        
        expr
    } else {
        // Internal node: compute the OR of all children.
        let mut child_exprs = Vec::new();
        
        // If there's no parent provided, this is a top-level category
        // Otherwise, use the parent that was passed in
        let next_parent = if parent_category.is_none() {
            Some(const_name.clone())
        } else {
            parent_category.clone()
        };
        
        for child in &tag.children {
            let child_expr = gen_constants(
                child, 
                counter, 
                constants, 
                match_arms, 
                tag_info_list, 
                next_parent.clone()
            );
            child_exprs.push(child_expr);
        }
        
        let expr = quote! { #(#child_exprs)|* };
        
        constants.push(quote! {
            pub const #const_ident: u32 = #expr;
        });
        
        match_arms.push(quote! {
            #name_str => #const_ident
        });
        
        // Store info for the category itself
        tag_info_list.push(TagInfo {
            name: name_str,
            const_name,
            bit_expr: expr.clone(),
            parent_category,
        });
        
        expr
    }
}