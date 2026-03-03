use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Attribute, Ident, LitStr, Token, Type, Visibility, braced};

// ---------------------------------------------------------------------------
// Parsing structures
// ---------------------------------------------------------------------------

/// The full macro input.
struct AttributeComponentInput {
    attrs: Vec<Attribute>,
    vis: Visibility,
    _struct_token: Token![struct],
    name: Ident,
    _brace_token: syn::token::Brace,
    fields: Punctuated<AttributeField, Token![,]>,
}

/// Direction arrow parsed from the DSL.
#[derive(Clone, Copy)]
enum Direction {
    ReadFrom,  // <-
    WriteTo,   // ->
}

/// A single field in the attribute_component DSL.
enum AttributeField {
    /// `name: Type <- "path"` or `name: Type -> "path"`
    Bound {
        vis: Visibility,
        name: Ident,
        _colon: Token![:],
        ty: Type,
        direction: Direction,
        path: AttributePath,
    },
    /// `name: Type` — no direction arrow, plain field.
    Plain {
        vis: Visibility,
        name: Ident,
        _colon: Token![:],
        ty: Type,
    },
}

/// The attribute path — either an explicit string or `$` (auto-generated).
enum AttributePath {
    Explicit(LitStr),
    Auto, // $
}

// ---------------------------------------------------------------------------
// Parse implementations
// ---------------------------------------------------------------------------

impl Parse for AttributeComponentInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let struct_token: Token![struct] = input.parse()?;
        let name: Ident = input.parse()?;
        let content;
        let brace_token = braced!(content in input);
        let fields = content.parse_terminated(AttributeField::parse, Token![,])?;

        Ok(Self {
            attrs,
            vis,
            _struct_token: struct_token,
            name,
            _brace_token: brace_token,
            fields,
        })
    }
}

impl Parse for AttributeField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;
        let colon: Token![:] = input.parse()?;
        let ty: Type = input.parse()?;

        // Try to parse a direction arrow
        if input.peek(Token![<]) && input.peek2(Token![-]) {
            // <- (read from attributes)
            let _lt: Token![<] = input.parse()?;
            let _minus: Token![-] = input.parse()?;
            let path = parse_attribute_path(input)?;
            Ok(AttributeField::Bound {
                vis,
                name,
                _colon: colon,
                ty,
                direction: Direction::ReadFrom,
                path,
            })
        } else if input.peek(Token![->]) {
            // -> (write to attributes)
            let _arrow: Token![->] = input.parse()?;
            let path = parse_attribute_path(input)?;
            Ok(AttributeField::Bound {
                vis,
                name,
                _colon: colon,
                ty,
                direction: Direction::WriteTo,
                path,
            })
        } else {
            Ok(AttributeField::Plain {
                vis,
                name,
                _colon: colon,
                ty,
            })
        }
    }
}

fn parse_attribute_path(input: ParseStream) -> syn::Result<AttributePath> {
    if input.peek(Token![$]) {
        let _: Token![$] = input.parse()?;
        Ok(AttributePath::Auto)
    } else {
        let lit: LitStr = input.parse()?;
        Ok(AttributePath::Explicit(lit))
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

struct BoundField {
    name: Ident,
    #[allow(dead_code)]
    ty: Type,
    direction: Direction,
    path: String,
}

pub fn attribute_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as AttributeComponentInput);
    match expand(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(input: AttributeComponentInput) -> syn::Result<TokenStream> {
    let struct_name = &input.name;
    let struct_vis = &input.vis;
    let struct_attrs = &input.attrs;
    let struct_name_str = struct_name.to_string();

    // Separate bound and plain fields, collect struct field definitions
    let mut field_defs = Vec::new();
    let mut bound_fields: Vec<BoundField> = Vec::new();

    for field in &input.fields {
        match field {
            AttributeField::Bound { vis, name, ty, direction, path, .. } => {
                field_defs.push(quote! { #vis #name: #ty });

                let resolved_path = match path {
                    AttributePath::Explicit(lit) => lit.value(),
                    AttributePath::Auto => format!("{}.{}", struct_name_str, name),
                };

                bound_fields.push(BoundField {
                    name: name.clone(),
                    ty: ty.clone(),
                    direction: *direction,
                    path: resolved_path,
                });
            }
            AttributeField::Plain { vis, name, ty, .. } => {
                field_defs.push(quote! { #vis #name: #ty });
            }
        }
    }

    let read_fields: Vec<&BoundField> = bound_fields
        .iter()
        .filter(|f| matches!(f.direction, Direction::ReadFrom))
        .collect();

    let write_fields: Vec<&BoundField> = bound_fields
        .iter()
        .filter(|f| matches!(f.direction, Direction::WriteTo))
        .collect();

    let has_reads = !read_fields.is_empty();
    let has_writes = !write_fields.is_empty();

    // --- Struct definition ---
    let struct_def = quote! {
        #(#struct_attrs)*
        #[derive(::bevy::prelude::Component, Default)]
        #struct_vis struct #struct_name {
            #(#field_defs),*
        }
    };

    // --- AttributeDerived impl ---
    let attribute_derived_impl = if has_reads {
        let should_update_checks: Vec<TokenStream> = read_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            quote! {
                {
                    let _val = attrs.value(#path);
                    if (self.#name - _val).abs() > f32::EPSILON {
                        return true;
                    }
                }
            }
        }).collect();

        let update_assignments: Vec<TokenStream> = read_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            quote! {
                self.#name = attrs.value(#path);
            }
        }).collect();

        quote! {
            impl ::bevy_gauge::derived::AttributeDerived for #struct_name {
                fn should_update(
                    &self,
                    attrs: &::bevy_gauge::attributes::Attributes,
                ) -> bool {
                    #(#should_update_checks)*
                    false
                }

                fn update_from_attributes(
                    &mut self,
                    attrs: &::bevy_gauge::attributes::Attributes,
                ) {
                    #(#update_assignments)*
                }
            }
        }
    } else {
        TokenStream::new()
    };

    // --- WriteBack impl ---
    let write_back_impl = if has_writes {
        let should_writeback_checks: Vec<TokenStream> = write_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            quote! {
                {
                    let _val = attrs.value(#path);
                    if (self.#name - _val).abs() > f32::EPSILON {
                        return true;
                    }
                }
            }
        }).collect();

        let writeback_assignments: Vec<TokenStream> = write_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            quote! {
                attributes.set_base(entity, #path, self.#name);
            }
        }).collect();

        quote! {
            impl ::bevy_gauge::derived::WriteBack for #struct_name {
                fn should_write_back(
                    &self,
                    attrs: &::bevy_gauge::attributes::Attributes,
                ) -> bool {
                    #(#should_writeback_checks)*
                    false
                }

                fn write_back(
                    &self,
                    entity: ::bevy::prelude::Entity,
                    attributes: &mut ::bevy_gauge::attributes_mut::AttributesMut,
                ) {
                    #(#writeback_assignments)*
                }
            }
        }
    } else {
        TokenStream::new()
    };

    // --- Inventory registration ---
    let inventory_submits = {
        let mut registrations = Vec::new();

        if has_reads {
            registrations.push(quote! {
                ::inventory::submit! {
                    ::bevy_gauge::derived::AttributeRegistration {
                        register_fn: |app| {
                            use ::bevy_gauge::derived::AttributesAppExt;
                            app.register_attribute_derived::<#struct_name>();
                        }
                    }
                }
            });
        }

        if has_writes {
            registrations.push(quote! {
                ::inventory::submit! {
                    ::bevy_gauge::derived::AttributeRegistration {
                        register_fn: |app| {
                            use ::bevy_gauge::derived::AttributesAppExt;
                            app.register_write_back::<#struct_name>();
                        }
                    }
                }
            });
        }

        quote! { #(#registrations)* }
    };

    Ok(quote! {
        #struct_def
        #attribute_derived_impl
        #write_back_impl
        #inventory_submits
    })
}
