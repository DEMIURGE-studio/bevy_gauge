use quote::quote;
use syn::token::{Brace, Paren, Semi};
use syn::{parse_macro_input, Attribute, Ident, Token, Visibility, LitStr};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

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

/// Direction for the field
#[derive(Debug, Clone, Copy)]
enum Direction {
    ReadFrom,    // <-
    WriteTo,     // ->
    Both,        // <->
}

/// One field in the user's DSL, e.g.
/// 
/// ```plain
///   foo: <- "Stats.foo",
///   bar: -> "Stats.bar",
///   baz: <-> "Stats.baz"
/// ```
enum StatField {
    WithDirection {
        name: Ident,
        _colon_token: Token![:],
        direction: Direction,
        path: LitStr,
    },
    Nested {
        name: Ident,
        _colon_token: Token![:],
        type_name: Ident,
        _brace_token: Brace,
        nested_fields: Punctuated<StatField, Token![,]>,
    },
}

/// Represents one field after parsing
#[derive(Debug)]
enum ParsedField {
    ReadFrom { 
        name: Ident, 
        path: String 
    },
    WriteTo { 
        name: Ident, 
        path: String 
    },
    Both { 
        name: Ident, 
        path: String 
    },
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

        // Check if we're parsing a nested type first
        if input.peek(Ident) {
            let type_name: Ident = input.parse()?;
            if input.peek(Brace) {
                let content;
                let brace_token = syn::braced!(content in input);
                let nested_fields = content.parse_terminated(StatField::parse, Token![,])?;
                return Ok(StatField::Nested {
                    name,
                    _colon_token: colon_token,
                    type_name,
                    _brace_token: brace_token,
                    nested_fields,
                });
            } else {
                return Err(input.error("expected `{` after type name"));
            }
        }
        
        // Now check for direction operators
        
        // ReadFrom: <-
        if input.peek(Token![<]) {
            let _lt_token: Token![<] = input.parse()?;
            
            // Check if next token is -
            if input.peek(Token![-]) {
                let _minus_token: Token![-] = input.parse()?;
                
                // Now check if it's followed by > for <->
                if input.peek(Token![>]) {
                    let _gt_token: Token![>] = input.parse()?;
                    
                    // It's bidirectional: <->
                    if input.peek(LitStr) {
                        let path: LitStr = input.parse()?;
                        return Ok(StatField::WithDirection {
                            name,
                            _colon_token: colon_token,
                            direction: Direction::Both,
                            path,
                        });
                    } else {
                        return Err(input.error("expected string literal after `<->`"));
                    }
                }
                
                // It's read-from: <-
                if input.peek(LitStr) {
                    let path: LitStr = input.parse()?;
                    return Ok(StatField::WithDirection {
                        name,
                        _colon_token: colon_token,
                        direction: Direction::ReadFrom,
                        path,
                    });
                } else {
                    return Err(input.error("expected string literal after `<-`"));
                }
            } else {
                return Err(input.error("expected `-` after `<`"));
            }
        }
        
        // WriteTo: ->
        else if input.peek(Token![-]) {
            let _minus_token: Token![-] = input.parse()?;
            
            if input.peek(Token![>]) {
                let _gt_token: Token![>] = input.parse()?;
                
                // It's write-to: ->
                if input.peek(LitStr) {
                    let path: LitStr = input.parse()?;
                    return Ok(StatField::WithDirection {
                        name,
                        _colon_token: colon_token,
                        direction: Direction::WriteTo,
                        path,
                    });
                } else {
                    return Err(input.error("expected string literal after `->`"));
                }
            } else {
                return Err(input.error("expected `>` after `-`"));
            }
        }

        Err(input.error("expected one of `<- \"path\"`, `-> \"path\"`, `<-> \"path\"`, or `TypeName { ... }`"))
    }
}

// ---------------------------------------------------------------------
// 3) Expanding
// ---------------------------------------------------------------------
impl StatStructInput {
    pub fn expand(&self) -> syn::Result<proc_macro2::TokenStream> {
        // parse fields into a Vec<ParsedField>
        let parsed_fields = parse_fields_list(&self.fields)?;

        // Step A) Build exactly ONE generic struct definition
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
            // If no variants, just generate an impl for the struct.
            expand_trait_impls_for_no_variant(&self.ident, &self.generics, &parsed_fields)
        };

        Ok(quote! {
            #struct_code
            #variants_code
        })
    }
}

fn expand_single_struct_def(
    user_attrs: &[Attribute],
    vis: &Visibility,
    ident: &Ident,
    generics: &syn::Generics,
    fields: &[ParsedField],
) -> syn::Result<proc_macro2::TokenStream> {
    // Add required derives
    let forced_attrs = quote! {
        #[derive(::bevy::prelude::Component, ::std::default::Default, ::std::fmt::Debug)]
    };

    // Generate field definitions
    let field_defs = fields.iter().map(|f| {
        match f {
            ParsedField::ReadFrom { name, .. } => {
                quote! { pub #name: f32 }
            },
            ParsedField::WriteTo { name, .. } => {
                quote! { pub #name: f32 }
            },
            ParsedField::Both { name, .. } => {
                quote! { pub #name: f32 }
            },
            ParsedField::Nested { name, type_name, .. } => {
                quote! { pub #name: #type_name }
            },
        }
    });

    // Add PhantomData if needed
    let has_generics = !generics.params.is_empty();
    let phantom_field = if has_generics {
        let generic_params_as_tuple = build_phantom_tuple(generics);
        quote! {
            pub _pd: ::std::marker::PhantomData<#generic_params_as_tuple>
        }
    } else {
        quote! {}
    };

    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote! {
        // user-supplied attributes
        #(#user_attrs)*

        // forced attributes
        #forced_attrs

        #vis struct #ident #impl_generics #where_clause {
            #(#field_defs),*,
            #phantom_field
        }
    })
}

fn expand_trait_impls_for_variant(
    struct_ident: &Ident,
    variant_ident: &Ident,
    fields: &[ParsedField],
) -> proc_macro2::TokenStream {
    let struct_name_with_variant = quote! { #struct_ident<#variant_ident> };

    // Build implementation bodies
    let should_update_body = collect_should_update_lines(fields, quote!(self));
    let update_body = collect_update_lines(fields, quote!(self));
    let is_valid_body = collect_is_valid_lines(fields);
    let wb_body = collect_writeback_lines(fields, quote!(self));

    quote! {
        impl StatDerived for #struct_name_with_variant {
            fn from_stats(stats: &bevy_gauge::prelude::Stats) -> Self {
                let mut s = Self::default();
                s.update_from_stats(stats);
                s
            }
            fn should_update(&self, stats: &bevy_gauge::prelude::Stats) -> bool {
                #should_update_body
            }
            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::Stats) {
                #update_body
            }
            fn is_valid(stats: &bevy_gauge::prelude::Stats) -> bool {
                #is_valid_body
            }
        }

        impl WriteBack for #struct_name_with_variant {
            fn write_back(&self, target_entity: Entity, stat_accessor: &mut bevy_gauge::prelude::StatAccessor) {
                #wb_body
            }
        }
    }
}

fn build_phantom_tuple(generics: &syn::Generics) -> proc_macro2::TokenStream {
    // Collect each type param
    let params = generics.params.iter().map(|gp| {
        match gp {
            syn::GenericParam::Type(t) => {
                let ident = &t.ident;
                quote!( #ident )
            },
            syn::GenericParam::Lifetime(l) => {
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
// 4) Parse fields from the user input to our intermediate representation
// ---------------------------------------------------------------------

fn parse_fields_list(fields: &Punctuated<StatField, Token![,]>) -> syn::Result<Vec<ParsedField>> {
    let mut results = Vec::new();
    for f in fields {
        let pf = match f {
            StatField::WithDirection { name, path, direction, .. } => {
                let path_str = path.value();
                match direction {
                    Direction::ReadFrom => ParsedField::ReadFrom { 
                        name: name.clone(), 
                        path: path_str 
                    },
                    Direction::WriteTo => ParsedField::WriteTo { 
                        name: name.clone(), 
                        path: path_str 
                    },
                    Direction::Both => ParsedField::Both { 
                        name: name.clone(), 
                        path: path_str 
                    },
                }
            },
            StatField::Nested { name, type_name, nested_fields, .. } => {
                let sub = parse_fields_list(nested_fields)?;
                ParsedField::Nested {
                    name: name.clone(),
                    type_name: type_name.clone(),
                    fields: sub,
                }
            },
        };
        results.push(pf);
    }
    Ok(results)
}

// ---------------------------------------------------------------------
// 5) Code generation for the implementation methods
// ---------------------------------------------------------------------

fn collect_update_lines(
    fields: &[ParsedField],
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::ReadFrom { name, path } => {
                lines.push(quote! {
                    #self_expr.#name = stats.get(#path).unwrap_or(0.0);
                });
            },
            ParsedField::Both { name, path } => {
                lines.push(quote! {
                    #self_expr.#name = stats.get(#path).unwrap_or(0.0);
                });
            },
            ParsedField::WriteTo { .. } => {
                // WriteTo fields aren't updated from stats
            },
            ParsedField::Nested { name, fields, .. } => {
                let nested_code = collect_update_lines(fields, quote!(#self_expr.#name));
                lines.push(nested_code);
            },
        }
    }

    quote! { #(#lines)* }
}

fn collect_should_update_lines(
    fields: &[ParsedField],
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::ReadFrom { name, path } => {
                lines.push(quote! {
                    #self_expr.#name != stats.get(#path).unwrap_or(0.0)
                });
            },
            ParsedField::Both { name, path } => {
                lines.push(quote! {
                    #self_expr.#name != stats.get(#path).unwrap_or(0.0)
                });
            },
            ParsedField::WriteTo { .. } => { /* skip */ },
            ParsedField::Nested { name, fields, .. } => {
                let nested_code = collect_should_update_lines(fields, quote!(#self_expr.#name));
                lines.push(nested_code);
            },
        }
    }

    // If no lines, return true
    if lines.is_empty() {
        return quote! { true };
    }

    // Combine with OR
    quote! { #(#lines)||* }
}

fn collect_is_valid_lines(fields: &[ParsedField]) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();
    
    for pf in fields {
        match pf {
            ParsedField::ReadFrom { path, .. } => {
                lines.push(quote! {
                    stats.get(#path).is_ok()
                });
            },
            ParsedField::WriteTo { path, .. } => {
                lines.push(quote! {
                    stats.get(#path).is_ok()
                });
            },
            ParsedField::Both { path, .. } => {
                lines.push(quote! {
                    stats.get(#path).is_ok()
                });
            },
            ParsedField::Nested { fields, .. } => {
                let nested_code = collect_is_valid_lines(fields);
                lines.push(nested_code);
            },
        }
    }
    
    // If no lines, return true
    if lines.is_empty() {
        return quote! { true };
    }
    
    // Combine with AND
    quote! { #(#lines)&&* }
}

fn collect_writeback_lines(
    fields: &[ParsedField],
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::WriteTo { name, path } => {
                lines.push(quote! {
                    let _ = stat_accessor.set(target_entity, #path, #self_expr.#name);
                });
            },
            ParsedField::Both { name, path } => {
                lines.push(quote! {
                    let _ = stat_accessor.set(target_entity, #path, #self_expr.#name);
                });
            },
            ParsedField::ReadFrom { .. } => { /* skip */ },
            ParsedField::Nested { name, fields, .. } => {
                let nested_code = collect_writeback_lines(fields, quote!(#self_expr.#name));
                lines.push(nested_code);
            },
        }
    }

    quote! { #(#lines)* }
}

fn expand_trait_impls_for_no_variant(
    struct_ident: &Ident,
    generics: &syn::Generics,
    fields: &[ParsedField],
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let should_update_body = collect_should_update_lines(fields, quote!(self));
    let update_body = collect_update_lines(fields, quote!(self));
    let writeback_body = collect_writeback_lines(fields, quote!(self));
    let is_valid_body = collect_is_valid_lines(fields);

    quote! {
        impl #impl_generics StatDerived for #struct_ident #ty_generics #where_clause {
            fn from_stats(stats: &bevy_gauge::prelude::Stats) -> Self {
                let mut s = Self::default();
                s.update_from_stats(stats);
                s
            }
            fn should_update(&self, stats: &bevy_gauge::prelude::Stats) -> bool {
                #should_update_body
            }
            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::Stats) {
                #update_body
            }
            fn is_valid(stats: &bevy_gauge::prelude::Stats) -> bool {
                #is_valid_body
            }
        }

        impl #impl_generics WriteBack for #struct_ident #ty_generics #where_clause {
            fn write_back(&self, target_entity: Entity, stat_accessor: &mut bevy_gauge::prelude::StatAccessor) {
                #writeback_body
            }
        }
    }
}