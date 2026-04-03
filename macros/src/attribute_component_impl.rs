use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, Ident, LitStr, Type};

#[derive(Clone, Copy)]
enum Direction {
    ReadFrom,
    WriteTo,
}

enum AttributePath {
    Explicit(String),
    Auto,
}

/// How to convert between f32 (gauge's native type) and the field type.
#[derive(Clone, Copy)]
enum FieldKind {
    /// `f32` — no conversion needed
    Float,
    /// Integer types (`u32`, `i32`, `usize`, `u64`, `i64`, etc.) — `.round() as T`
    Integer,
    /// `bool` — `!= 0.0` to read, `as u32 as f32` to write
    Bool,
    /// Non-terminal type — delegate to `AttributeResolvable`
    Composite,
}

/// Classify a syn::Type into a FieldKind.
fn classify_type(ty: &Type) -> FieldKind {
    if let Type::Path(type_path) = ty {
        if let Some(ident) = type_path.path.get_ident() {
            let name = ident.to_string();
            return match name.as_str() {
                "f32" | "f64" => FieldKind::Float,
                "bool" => FieldKind::Bool,
                "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => FieldKind::Integer,
                _ => FieldKind::Composite,
            };
        }
    }
    FieldKind::Composite
}

struct BoundField {
    name: Ident,
    ty: Type,
    direction: Direction,
    path: String,
    tag_expr: Option<syn::Expr>,
    kind: FieldKind,
}

struct InitFromField {
    name: Ident,
    ty: Type,
    path: String,
    kind: FieldKind,
}

struct InitToField {
    name: Ident,
    path: String,
    kind: FieldKind,
}

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;
    let struct_name_str = struct_name.to_string();

    let fields = match &input.data {
        syn::Data::Struct(data) => &data.fields,
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "AttributeComponent can only be derived on structs",
            ))
        }
    };

    let Fields::Named(named) = fields else {
        return Err(syn::Error::new_spanned(
            fields,
            "AttributeComponent requires named fields",
        ));
    };

    let mut bound_fields: Vec<BoundField> = Vec::new();
    let mut init_from_fields: Vec<InitFromField> = Vec::new();
    let mut init_to_fields: Vec<InitToField> = Vec::new();

    for field in &named.named {
        let field_name = field.ident.as_ref().unwrap();
        let mut has_init_to = false;
        let mut read_path: Option<String> = None;

        // First pass: detect #[init_to] and collect #[read] path
        for attr in &field.attrs {
            if attr.path().is_ident("init_to") {
                has_init_to = true;
            }
        }

        for attr in &field.attrs {
            if attr.path().is_ident("read") || attr.path().is_ident("write") {
                let direction = if attr.path().is_ident("read") {
                    Direction::ReadFrom
                } else {
                    Direction::WriteTo
                };
                let (path, tag_expr) = parse_path_and_tag(attr)?;

                let resolved_path = match path {
                    AttributePath::Explicit(lit) => lit,
                    AttributePath::Auto => format!("{}.{}", struct_name_str, field_name),
                };

                if matches!(direction, Direction::ReadFrom) {
                    read_path = Some(resolved_path.clone());
                }

                bound_fields.push(BoundField {
                    name: field_name.clone(),
                    ty: field.ty.clone(),
                    direction,
                    path: resolved_path,
                    tag_expr,
                    kind: classify_type(&field.ty),
                });
            } else if attr.path().is_ident("init_from") {
                let path = parse_init_from(attr)?;
                init_from_fields.push(InitFromField {
                    name: field_name.clone(),
                    ty: field.ty.clone(),
                    path,
                    kind: classify_type(&field.ty),
                });
            }
        }

        if has_init_to {
            let path = read_path.ok_or_else(|| {
                syn::Error::new_spanned(
                    field_name,
                    "#[init_to] requires a #[read(\"path\")] attribute on the same field",
                )
            })?;
            init_to_fields.push(InitToField {
                name: field_name.clone(),
                path,
                kind: classify_type(&field.ty),
            });
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
    let has_init_to = !init_to_fields.is_empty();
    let has_init_from = !init_from_fields.is_empty();

    let attribute_derived_impl = if has_reads {
        let should_update_checks: Vec<TokenStream> = read_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            let val_expr = read_value_expr(path, &f.tag_expr);
            match f.kind {
                FieldKind::Float => quote! {
                    {
                        let _val = #val_expr;
                        if (self.#name - _val).abs() > f32::EPSILON {
                            return true;
                        }
                    }
                },
                FieldKind::Integer => {
                    let ty = &f.ty;
                    quote! {
                        {
                            let _val = (#val_expr).round() as #ty;
                            if self.#name != _val {
                                return true;
                            }
                        }
                    }
                },
                FieldKind::Bool => quote! {
                    {
                        let _val = (#val_expr) != 0.0;
                        if self.#name != _val {
                            return true;
                        }
                    }
                },
                FieldKind::Composite => quote! {
                    if ::bevy_gauge::resolvable::AttributeResolvable::should_resolve(
                        &self.#name, #path, attrs,
                    ) {
                        return true;
                    }
                },
            }
        }).collect();

        let update_assignments: Vec<TokenStream> = read_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            let val_expr = read_value_expr(path, &f.tag_expr);
            match f.kind {
                FieldKind::Float => quote! {
                    self.#name = #val_expr;
                },
                FieldKind::Integer => {
                    let ty = &f.ty;
                    quote! {
                        self.#name = (#val_expr).round() as #ty;
                    }
                },
                FieldKind::Bool => quote! {
                    self.#name = (#val_expr) != 0.0;
                },
                FieldKind::Composite => quote! {
                    ::bevy_gauge::resolvable::AttributeResolvable::resolve(
                        &mut self.#name, #path, attrs,
                    );
                },
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

    let write_back_impl = if has_writes {
        let should_writeback_checks: Vec<TokenStream> = write_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            let val_expr = read_value_expr(path, &f.tag_expr);
            match f.kind {
                FieldKind::Float => quote! {
                    {
                        let _val = #val_expr;
                        if (self.#name - _val).abs() > f32::EPSILON {
                            return true;
                        }
                    }
                },
                FieldKind::Integer => {
                    let ty = &f.ty;
                    quote! {
                        {
                            let _val = (#val_expr).round() as #ty;
                            if self.#name != _val {
                                return true;
                            }
                        }
                    }
                },
                FieldKind::Bool => quote! {
                    {
                        let _val = (#val_expr) != 0.0;
                        if self.#name != _val {
                            return true;
                        }
                    }
                },
                FieldKind::Composite => quote! {
                    if ::bevy_gauge::resolvable::AttributeResolvable::should_resolve(
                        &self.#name, #path, attrs,
                    ) {
                        return true;
                    }
                },
            }
        }).collect();

        let writeback_assignments: Vec<TokenStream> = write_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            match f.kind {
                FieldKind::Float => quote! {
                    attributes.set_base(entity, #path, self.#name);
                },
                FieldKind::Integer => quote! {
                    attributes.set_base(entity, #path, self.#name as f32);
                },
                FieldKind::Bool => quote! {
                    attributes.set_base(entity, #path, if self.#name { 1.0 } else { 0.0 });
                },
                FieldKind::Composite => quote! {
                    // Composite WriteBack is not supported — composites are read-only
                    // via AttributeResolvable. Writing back nested structures to
                    // attributes is not a supported pattern.
                    compile_error!("Cannot use #[write] on a composite (non-terminal) field. Use #[read] instead.");
                },
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

                fn write_back<F: ::bevy::ecs::query::QueryFilter>(
                    &self,
                    entity: ::bevy::prelude::Entity,
                    attributes: &mut ::bevy_gauge::attributes_mut::AttributesMut<'_, '_, F>,
                ) {
                    #(#writeback_assignments)*
                }
            }
        }
    } else {
        TokenStream::new()
    };

    let init_to_impl = if has_init_to {
        let seed_assignments: Vec<TokenStream> = init_to_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            match f.kind {
                FieldKind::Float => quote! {
                    attributes.set_base(entity, #path, self.#name);
                },
                FieldKind::Integer => quote! {
                    attributes.set_base(entity, #path, self.#name as f32);
                },
                FieldKind::Bool => quote! {
                    attributes.set_base(entity, #path, if self.#name { 1.0 } else { 0.0 });
                },
                FieldKind::Composite => quote! {
                    compile_error!("Cannot use #[init_to] on a composite (non-terminal) field.");
                },
            }
        }).collect();

        quote! {
            impl ::bevy_gauge::derived::InitTo for #struct_name {
                fn init_to_attributes<F: ::bevy::ecs::query::QueryFilter>(
                    &self,
                    entity: ::bevy::prelude::Entity,
                    attributes: &mut ::bevy_gauge::attributes_mut::AttributesMut<'_, '_, F>,
                ) {
                    #(#seed_assignments)*
                }
            }
        }
    } else {
        TokenStream::new()
    };

    let init_from_impl = if has_init_from {
        let init_assignments: Vec<TokenStream> = init_from_fields.iter().map(|f| {
            let name = &f.name;
            let path = &f.path;
            match f.kind {
                FieldKind::Float => quote! {
                    self.#name = attrs.value(#path);
                },
                FieldKind::Integer => {
                    let ty = &f.ty;
                    quote! {
                        self.#name = attrs.value(#path).round() as #ty;
                    }
                },
                FieldKind::Bool => quote! {
                    self.#name = attrs.value(#path) != 0.0;
                },
                FieldKind::Composite => quote! {
                    ::bevy_gauge::resolvable::AttributeResolvable::resolve(
                        &mut self.#name, #path, attrs,
                    );
                },
            }
        }).collect();

        quote! {
            impl ::bevy_gauge::derived::InitFrom for #struct_name {
                fn init_from_attributes(
                    &mut self,
                    attrs: &::bevy_gauge::attributes::Attributes,
                ) {
                    #(#init_assignments)*
                }
            }
        }
    } else {
        TokenStream::new()
    };

    let inventory_submits = {
        let mut registrations = Vec::new();

        if has_reads {
            registrations.push(quote! {
                ::inventory::submit! {
                    ::bevy_gauge::derived::AttributeRegistration {
                        kind: ::bevy_gauge::derived::RegistrationKind::Derived,
                        register_fn: |app| {
                            use ::bevy_gauge::derived::AttributesAppExt;
                            app.register_attribute_derived::<#struct_name>();
                        },
                        register_in_schedule_fn: |app, schedule| {
                            use ::bevy::prelude::*;
                            app.add_systems(
                                schedule,
                                ::bevy_gauge::derived::update_attribute_derived::<#struct_name>
                                    .in_set(::bevy_gauge::derived::AttributeDerivedSet),
                            );
                        },
                    }
                }
            });
        }

        if has_writes {
            registrations.push(quote! {
                ::inventory::submit! {
                    ::bevy_gauge::derived::AttributeRegistration {
                        kind: ::bevy_gauge::derived::RegistrationKind::WriteBack,
                        register_fn: |app| {
                            use ::bevy_gauge::derived::AttributesAppExt;
                            app.register_write_back::<#struct_name>();
                        },
                        register_in_schedule_fn: |app, schedule| {
                            use ::bevy::prelude::*;
                            app.add_systems(
                                schedule,
                                ::bevy_gauge::derived::update_write_back::<#struct_name>
                                    .in_set(::bevy_gauge::derived::WriteBackSet),
                            );
                        },
                    }
                }
            });
        }

        if has_init_to {
            registrations.push(quote! {
                ::inventory::submit! {
                    ::bevy_gauge::derived::AttributeRegistration {
                        kind: ::bevy_gauge::derived::RegistrationKind::InitTo,
                        register_fn: |app| {
                            use ::bevy_gauge::derived::AttributesAppExt;
                            app.register_init_to::<#struct_name>();
                        },
                        register_in_schedule_fn: |app, schedule| {
                            use ::bevy::prelude::*;
                            app.add_systems(
                                schedule,
                                ::bevy_gauge::derived::apply_init_to::<#struct_name>
                                    .in_set(::bevy_gauge::derived::WriteBackSet),
                            );
                        },
                    }
                }
            });
        }

        if has_init_from {
            registrations.push(quote! {
                ::inventory::submit! {
                    ::bevy_gauge::derived::AttributeRegistration {
                        kind: ::bevy_gauge::derived::RegistrationKind::InitFrom,
                        register_fn: |app| {
                            use ::bevy_gauge::derived::AttributesAppExt;
                            app.register_init_from::<#struct_name>();
                        },
                        register_in_schedule_fn: |app, schedule| {
                            use ::bevy::prelude::*;
                            app.add_systems(
                                schedule,
                                ::bevy_gauge::derived::apply_init_from::<#struct_name>
                                    .in_set(::bevy_gauge::derived::InitFromSet),
                            );
                        },
                    }
                }
            });
        }

        quote! { #(#registrations)* }
    };

    Ok(quote! {
        #attribute_derived_impl
        #write_back_impl
        #init_to_impl
        #init_from_impl
        #inventory_submits
    })
}

fn read_value_expr(path: &str, tag_expr: &Option<syn::Expr>) -> TokenStream {
    match tag_expr {
        Some(expr) => quote! { attrs.value_tagged(#path, #expr) },
        None => quote! { attrs.value(#path) },
    }
}

/// Parse `#[init_from("attribute_path")]`
fn parse_init_from(attr: &syn::Attribute) -> syn::Result<String> {
    match &attr.meta {
        syn::Meta::List(list) => {
            let lit: LitStr = syn::parse2(list.tokens.clone())?;
            Ok(lit.value())
        }
        _ => Err(syn::Error::new_spanned(
            attr,
            "expected `#[init_from(\"attribute_path\")]`",
        )),
    }
}

fn parse_path_and_tag(attr: &syn::Attribute) -> syn::Result<(AttributePath, Option<syn::Expr>)> {
    match &attr.meta {
        syn::Meta::Path(_) => Ok((AttributePath::Auto, None)),
        syn::Meta::List(list) => {
            let tokens = list.tokens.clone();
            let mut iter = tokens.into_iter().peekable();

            let first = iter.next().ok_or_else(|| {
                syn::Error::new_spanned(&list, "expected at least a path string")
            })?;

            let path = if let proc_macro2::TokenTree::Literal(_) = &first {
                let lit_str: LitStr = syn::parse2(first.clone().into())?;
                AttributePath::Explicit(lit_str.value())
            } else {
                return Err(syn::Error::new_spanned(
                    &first,
                    "expected a string literal for the attribute path",
                ));
            };

            let tag = if iter.peek().is_some() {
                if let Some(proc_macro2::TokenTree::Punct(p)) = iter.next() {
                    if p.as_char() != ',' {
                        return Err(syn::Error::new(p.span(), "expected `,`"));
                    }
                } else {
                    return Err(syn::Error::new_spanned(&list, "expected `,` after path"));
                }
                let rest: proc_macro2::TokenStream = iter.collect();
                if rest.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &list,
                        "expected a tag expression after `,`",
                    ));
                }
                Some(syn::parse2::<syn::Expr>(rest)?)
            } else {
                None
            };

            Ok((path, tag))
        }
        syn::Meta::NameValue(_) => Err(syn::Error::new_spanned(
            attr,
            "expected `#[read]`, `#[read(\"path\")]`, or `#[read(\"path\", TAG)]`",
        )),
    }
}
