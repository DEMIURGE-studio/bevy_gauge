use quote::quote;
use syn::token::{Brace, Paren, Semi};
use syn::{parse_macro_input, Attribute, Ident, Token, Visibility, LitStr, Type};
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
    _struct_token: Option<Token![struct]>,
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
}

/// One field in the user's DSL, e.g.
/// 
/// ```plain
///   foo: f32 <- "Stats.foo",
///   bar: Option<f32> -> "Stats.bar",
///   baz: String,  // non-stat field
/// ```
enum StatField {
    WithDirection {
        vis: Visibility,
        name: Ident,
        _colon_token: Token![:],
        field_type: Type,
        direction: Direction,
        path: LitStr,
    },
    AutoPath {
        vis: Visibility,
        name: Ident,
        _colon_token: Token![:],
        field_type: Type,
        direction: Direction,
        _dollar_token: Token![$],
        explicit_suffix: Option<String>, // Optional ".part.tag@target" after $
    },
    Nested {
        vis: Visibility,
        name: Ident,
        _colon_token: Token![:],
        type_name: Ident,
        _brace_token: Brace,
        nested_fields: Punctuated<StatField, Token![,]>,
    },
    NonStat {
        vis: Visibility,
        name: Ident,
        _colon_token: Token![:],
        field_type: Type,
    },
}

/// Represents one field after parsing
enum ParsedField {
    ReadFrom { 
        vis: Visibility,
        name: Ident, 
        field_type: Type,
        path: String 
    },
    WriteTo { 
        vis: Visibility,
        name: Ident, 
        field_type: Type,
        path: String 
    },
    Nested { 
        vis: Visibility,
        name: Ident,
        type_name: Ident,
        fields: Vec<ParsedField>
    },
    AutoReadFrom { 
        vis: Visibility,
        name: Ident, 
        field_type: Type,
        resolved_path: String 
    },
    AutoWriteTo { 
        vis: Visibility,
        name: Ident, 
        field_type: Type,
        resolved_path: String 
    },
    NonStat {
        vis: Visibility,
        name: Ident,
        field_type: Type,
    },
}

// ---------------------------------------------------------------------
// 2) Parsing
// ---------------------------------------------------------------------
impl Parse for StatStructInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Collect user-supplied outer attributes
        let attrs = input.call(Attribute::parse_outer)?;

        // Parsing order matters to disambiguate intent:
        // - If `struct` appears first, treat visibility as private (inherited)
        // - Else if a visibility appears, use it; `struct` remains optional
        // - Else, default to `pub` and assume struct is omitted

        let (vis, struct_token): (Visibility, Option<Token![struct]>) = if input.peek(Token![struct]) {
            // Explicit `struct` with no vis => private struct
            let st: Token![struct] = input.parse()?;
            (Visibility::Inherited, Some(st))
        } else if input.peek(Token![pub])
            || input.peek(Token![crate])
            || input.peek(Token![self])
            || input.peek(Token![super])
        {
            let v: Visibility = input.parse()?;
            let st = if input.peek(Token![struct]) { Some(input.parse()?) } else { None };
            (v, st)
        } else {
            // No vis and no `struct`: default to public, and assume omitted `struct`
            (syn::parse_quote!(pub), None)
        };

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
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;
        let colon_token: Token![:] = input.parse()?;

        // Use a custom type parser that stops at direction operators
        let field_type = parse_type_until_operator(input)?;

        // Check for direction operators after parsing the type
        if input.peek(Token![<]) && input.peek2(Token![-]) {
            let _lt_token: Token![<] = input.parse()?;
            let _minus_token: Token![-] = input.parse()?;
            
            // It's read-from: <-
            if input.peek(Token![$]) {
                let dollar_token: Token![$] = input.parse()?;
                return Ok(StatField::AutoPath {
                    vis,
                    name,
                    _colon_token: colon_token,
                    field_type,
                    direction: Direction::ReadFrom,
                    _dollar_token: dollar_token,
                    explicit_suffix: None,
                });
            } else if input.peek(LitStr) {
                let path: LitStr = input.parse()?;
                return Ok(StatField::WithDirection {
                    vis,
                    name,
                    _colon_token: colon_token,
                    field_type,
                    direction: Direction::ReadFrom,
                    path,
                });
            } else {
                return Err(input.error("expected string literal or $ after `<-`"));
            }
        } else if input.peek(Token![-]) && input.peek2(Token![>]) {
            let _minus_token: Token![-] = input.parse()?;
            let _gt_token: Token![>] = input.parse()?;
            
            // It's write-to: ->
            if input.peek(Token![$]) {
                let dollar_token: Token![$] = input.parse()?;
                return Ok(StatField::AutoPath {
                    vis,
                    name,
                    _colon_token: colon_token,
                    field_type,
                    direction: Direction::WriteTo,
                    _dollar_token: dollar_token,
                    explicit_suffix: None,
                });
            } else if input.peek(LitStr) {
                let path: LitStr = input.parse()?;
                return Ok(StatField::WithDirection {
                    vis,
                    name,
                    _colon_token: colon_token,
                    field_type,
                    direction: Direction::WriteTo,
                    path,
                });
            } else {
                return Err(input.error("expected string literal or $ after `->`"));
            }
        } else {
            // Check if we're parsing a nested type
            if let Type::Path(type_path) = &field_type {
                if type_path.path.segments.len() == 1 {
                    let type_name = &type_path.path.segments.first().unwrap().ident;
                    if input.peek(Brace) {
                        let content;
                        let brace_token = syn::braced!(content in input);
                        let nested_fields = content.parse_terminated(StatField::parse, Token![,])?;
                        return Ok(StatField::Nested {
                            vis,
                            name,
                            _colon_token: colon_token,
                            type_name: type_name.clone(),
                            _brace_token: brace_token,
                            nested_fields,
                        });
                    }
                }
            }
            
            // Regular non-stat field
            return Ok(StatField::NonStat {
                vis,
                name,
                _colon_token: colon_token,
                field_type,
            });
        }
    }
}

// Helper function to parse types by collecting tokens until direction operators or end of field
fn parse_type_until_operator(input: ParseStream) -> syn::Result<Type> {
    use proc_macro2::{TokenStream, TokenTree};
    use quote::ToTokens;
    
    let mut tokens = TokenStream::new();
    
    // Collect tokens until we hit a direction operator or field separator
    while !input.is_empty() {
        // Stop at direction operators
        if (input.peek(Token![<]) && input.peek2(Token![-])) ||
           (input.peek(Token![-]) && input.peek2(Token![>])) {
            break;
        }
        // Stop at comma (field separator) or closing brace
        if input.peek(Token![,]) || input.peek(Brace) {
            break;
        }
        
        // Special handling for generics - collect everything between < and >
        if input.peek(Token![<]) {
            let lt: Token![<] = input.parse()?;
            lt.to_tokens(&mut tokens);
            
            let mut bracket_depth = 1;
            while bracket_depth > 0 && !input.is_empty() {
                if input.peek(Token![<]) {
                    bracket_depth += 1;
                    let lt: Token![<] = input.parse()?;
                    lt.to_tokens(&mut tokens);
                } else if input.peek(Token![>]) {
                    bracket_depth -= 1;
                    let gt: Token![>] = input.parse()?;
                    gt.to_tokens(&mut tokens);
                } else {
                    let token: TokenTree = input.parse()?;
                    token.to_tokens(&mut tokens);
                }
            }
        } else {
            let token: TokenTree = input.parse()?;
            token.to_tokens(&mut tokens);
        }
    }
    
    // Parse the collected tokens as a type
    syn::parse2(tokens)
}

// ---------------------------------------------------------------------
// 3) Expanding
// ---------------------------------------------------------------------
impl StatStructInput {
    pub fn expand(&self) -> syn::Result<proc_macro2::TokenStream> {
        // parse fields into a Vec<ParsedField>
        let struct_name = self.ident.to_string();
        let parsed_fields = parse_fields_list_with_context(&self.fields, &struct_name, &[])?;

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
        #[derive(::bevy::prelude::Component, ::std::default::Default)]
    };

    // Generate field definitions
    let field_defs = fields.iter().map(|f| {
        match f {
            ParsedField::ReadFrom { vis, name, field_type, .. } => {
                quote! { #vis #name: #field_type }
            },
            ParsedField::WriteTo { vis, name, field_type, .. } => {
                quote! { #vis #name: #field_type }
            },
            ParsedField::Nested { vis, name, type_name, .. } => {
                quote! { #vis #name: #type_name }
            },
            ParsedField::AutoReadFrom { vis, name, field_type, .. } => {
                quote! { #vis #name: #field_type }
            },
            ParsedField::AutoWriteTo { vis, name, field_type, .. } => {
                quote! { #vis #name: #field_type }
            },
            ParsedField::NonStat { vis, name, field_type, .. } => {
                quote! { #vis #name: #field_type }
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
    let wb_body = collect_writeback_lines(fields, quote!(self));
    let should_wb_body = collect_should_writeback_lines(fields, quote!(self));

    quote! {
        impl StatDerived for #struct_name_with_variant {
            fn should_update(&self, stats: &bevy_gauge::prelude::Stats) -> bool {
                #should_update_body
            }
            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::Stats) {
                #update_body
            }
        }

        impl WriteBack for #struct_name_with_variant {
            fn write_back(&self, target_entity: Entity, stats_mutator: &mut bevy_gauge::prelude::StatsMutator) {
                #wb_body
            }
            
            fn should_write_back(&self, target_entity: Entity, stats_mutator: &bevy_gauge::prelude::StatsMutator) -> bool {
                #should_wb_body
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

fn resolve_auto_path(
    struct_name: &str,
    nested_path: &[String], 
    field_name: &str,
    explicit_suffix: Option<&str>
) -> String {
    let mut path_parts = vec![struct_name.to_string()];
    path_parts.extend(nested_path.iter().cloned());
    path_parts.push(field_name.to_string());
    
    let auto_base = format!("$[{}]", path_parts.join("."));
    
    if let Some(suffix) = explicit_suffix {
        format!("{}{}", auto_base, suffix)
    } else {
        auto_base
    }
}

fn parse_fields_list_with_context(
    fields: &Punctuated<StatField, Token![,]>,
    struct_name: &str,
    nested_path: &[String]
) -> syn::Result<Vec<ParsedField>> {
    let mut results = Vec::new();
    for f in fields {
        let pf = match f {
            StatField::WithDirection { vis, name, field_type, path, direction, .. } => {
                let path_str = path.value();
                match direction {
                    Direction::ReadFrom => ParsedField::ReadFrom { 
                        vis: vis.clone(),
                        name: name.clone(), 
                        field_type: field_type.clone(),
                        path: path_str 
                    },
                    Direction::WriteTo => ParsedField::WriteTo { 
                        vis: vis.clone(),
                        name: name.clone(), 
                        field_type: field_type.clone(),
                        path: path_str 
                    },
                }
            },
            StatField::AutoPath { vis, name, field_type, direction, explicit_suffix, .. } => {
                let resolved = resolve_auto_path(
                    struct_name,
                    nested_path,
                    &name.to_string(),
                    explicit_suffix.as_deref()
                );
                
                match direction {
                    Direction::ReadFrom => ParsedField::AutoReadFrom {
                        vis: vis.clone(),
                        name: name.clone(),
                        field_type: field_type.clone(),
                        resolved_path: resolved,
                    },
                    Direction::WriteTo => ParsedField::AutoWriteTo {
                        vis: vis.clone(),
                        name: name.clone(),
                        field_type: field_type.clone(),
                        resolved_path: resolved,
                    },
                }
            },
            StatField::Nested { vis, name, type_name, nested_fields, .. } => {
                let mut new_nested_path = nested_path.to_vec();
                new_nested_path.push(name.to_string());
                
                let sub = parse_fields_list_with_context(
                    nested_fields, 
                    struct_name,
                    &new_nested_path
                )?;
                
                ParsedField::Nested {
                    vis: vis.clone(),
                    name: name.clone(),
                    type_name: type_name.clone(),
                    fields: sub,
                }
            },
            StatField::NonStat { vis, name, field_type, .. } => {
                ParsedField::NonStat {
                    vis: vis.clone(),
                    name: name.clone(),
                    field_type: field_type.clone(),
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
            ParsedField::ReadFrom { name, field_type, path, .. } => {
                let update_code = generate_stat_read_code(name, field_type, path);
                lines.push(quote! {
                    #self_expr.#update_code
                });
            },
            ParsedField::WriteTo { .. } => {
                // WriteTo fields aren't updated from stats
            },
            ParsedField::Nested { name, fields, .. } => {
                let nested_code = collect_update_lines(fields, quote!(#self_expr.#name));
                lines.push(nested_code);
            },
            ParsedField::AutoReadFrom { name, field_type, resolved_path, .. } => {
                let update_code = generate_stat_read_code(name, field_type, resolved_path);
                lines.push(quote! {
                    #self_expr.#update_code
                });
            },
            ParsedField::AutoWriteTo { .. } => {
                // AutoWriteTo fields aren't updated from stats
            },
            ParsedField::NonStat { .. } => {
                // Non-stat fields aren't updated from stats
            },
        }
    }

    quote! { #(#lines)* }
}

// Helper function to generate the appropriate stat reading code based on type
fn generate_stat_read_code(name: &Ident, field_type: &Type, path: &str) -> proc_macro2::TokenStream {
    if is_option_f32(field_type) {
        quote! {
            #name = {
                let val = stats.get(#path);
                if val == 0.0 { None } else { Some(val) }
            };
        }
    } else {
        quote! {
            #name = stats.get(#path);
        }
    }
}

// Helper function to check if a type is Option<f32>
fn is_option_f32(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if args.args.len() == 1 {
                        if let syn::GenericArgument::Type(Type::Path(inner_path)) = &args.args[0] {
                            if let Some(inner_segment) = inner_path.path.segments.last() {
                                return inner_segment.ident == "f32";
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

fn collect_should_update_lines(
    fields: &[ParsedField],
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::ReadFrom { name, field_type, path, .. } => {
                let comparison_code = generate_stat_comparison_code(name, field_type, path, &self_expr);
                lines.push(comparison_code);
            },
            ParsedField::WriteTo { .. } => { /* skip */ },
            ParsedField::Nested { name, fields, .. } => {
                let nested_code = collect_should_update_lines(fields, quote!(#self_expr.#name));
                lines.push(nested_code);
            },
            ParsedField::AutoReadFrom { name, field_type, resolved_path, .. } => {
                let comparison_code = generate_stat_comparison_code(name, field_type, resolved_path, &self_expr);
                lines.push(comparison_code);
            },
            ParsedField::AutoWriteTo { .. } => { /* skip */ },
            ParsedField::NonStat { .. } => {
                // Non-stat fields aren't updated from stats
            },
        }
    }

    // If no lines, return false (nothing to update)
    if lines.is_empty() {
        return quote! { false };
    }

    // Combine with OR - properly join conditions
    if lines.len() == 1 {
        quote! { #(#lines)* }
    } else {
        // Properly construct the OR chain with parentheses around each expression
        quote! { 
            #( (#lines) )||*
        }
    }
}

// Helper function to generate comparison code for should_update
fn generate_stat_comparison_code(name: &Ident, field_type: &Type, path: &str, self_expr: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    if is_option_f32(field_type) {
        quote! {
            {
                let val = stats.get(#path);
                let current_val = if val == 0.0 { None } else { Some(val) };
                #self_expr.#name != current_val
            }
        }
    } else {
        quote! {
            #self_expr.#name != stats.get(#path)
        }
    }
}



fn collect_writeback_lines(
    fields: &[ParsedField],
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::WriteTo { name, field_type, path, .. } => {
                let writeback_code = generate_stat_writeback_code(name, field_type, path, &self_expr);
                lines.push(writeback_code);
            },
            ParsedField::ReadFrom { .. } => { /* skip */ },
            ParsedField::Nested { name, fields, .. } => {
                let nested_code = collect_writeback_lines(fields, quote!(#self_expr.#name));
                lines.push(nested_code);
            },
            ParsedField::AutoWriteTo { name, field_type, resolved_path, .. } => {
                let writeback_code = generate_stat_writeback_code(name, field_type, resolved_path, &self_expr);
                lines.push(writeback_code);
            },
            ParsedField::AutoReadFrom { .. } => { /* skip */ },
            ParsedField::NonStat { .. } => {
                // Non-stat fields aren't written back
            },
        }
    }

    quote! { #(#lines)* }
}

// Helper function to generate writeback code based on type
fn generate_stat_writeback_code(name: &Ident, field_type: &Type, path: &str, self_expr: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    if is_option_f32(field_type) {
        quote! {
            let _ = stats_mutator.set(target_entity, #path, #self_expr.#name.unwrap_or(0.0));
        }
    } else {
        quote! {
            let _ = stats_mutator.set(target_entity, #path, #self_expr.#name);
        }
    }
}

fn collect_should_writeback_lines(
    fields: &[ParsedField],
    self_expr: proc_macro2::TokenStream
) -> proc_macro2::TokenStream {
    let mut lines = Vec::new();

    for pf in fields {
        match pf {
            ParsedField::WriteTo { name, field_type, path, .. } => {
                let comparison_code = generate_writeback_comparison_code(name, field_type, path, &self_expr);
                lines.push(comparison_code);
            },
            ParsedField::ReadFrom { .. } => { /* skip */ },
            ParsedField::Nested { name, fields, .. } => {
                let nested_code = collect_should_writeback_lines(fields, quote!(#self_expr.#name));
                lines.push(nested_code);
            },
            ParsedField::AutoWriteTo { name, field_type, resolved_path, .. } => {
                let comparison_code = generate_writeback_comparison_code(name, field_type, resolved_path, &self_expr);
                lines.push(comparison_code);
            },
            ParsedField::AutoReadFrom { .. } => { /* skip */ },
            ParsedField::NonStat { .. } => {
                // Non-stat fields aren't written back
            },
        }
    }

    // If no writeback fields, return false (never write back)
    if lines.is_empty() {
        return quote! { false };
    }

    // Combine with OR - if any writeback field has changed, we should write back
    if lines.len() == 1 {
        quote! { #(#lines)* }
    } else {
        // Properly construct the OR chain with parentheses around each expression
        quote! { 
            #( (#lines) )||*
        }
    }
}

// Helper function to generate writeback comparison code
fn generate_writeback_comparison_code(name: &Ident, field_type: &Type, path: &str, self_expr: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    if is_option_f32(field_type) {
        quote! {
            #self_expr.#name.unwrap_or(0.0) != stats_mutator.get(target_entity, #path)
        }
    } else {
        quote! {
            #self_expr.#name != stats_mutator.get(target_entity, #path)
        }
    }
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
    let should_wb_body = collect_should_writeback_lines(fields, quote!(self));

    quote! {
        impl #impl_generics StatDerived for #struct_ident #ty_generics #where_clause {
            fn should_update(&self, stats: &bevy_gauge::prelude::Stats) -> bool {
                #should_update_body
            }
            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::Stats) {
                #update_body
            }
        }

        impl #impl_generics WriteBack for #struct_ident #ty_generics #where_clause {
            fn write_back(&self, target_entity: Entity, stats_mutator: &mut bevy_gauge::prelude::StatsMutator) {
                #writeback_body
            }
            
            fn should_write_back(&self, target_entity: Entity, stats_mutator: &bevy_gauge::prelude::StatsMutator) -> bool {
                #should_wb_body
            }
        }
    }
}