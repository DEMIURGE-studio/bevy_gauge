use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, Ident, LitStr, Type};

use crate::field_kind::{classify_type, FieldKind};

#[derive(Clone, Copy)]
enum Direction {
    ReadFrom,
    WriteTo,
}

enum AttributePath {
    Explicit(String),
    Auto,
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
                if matches!(direction, Direction::WriteTo)
                    && classify_type(&field.ty) == FieldKind::Composite
                {
                    return Err(syn::Error::new_spanned(
                        &field.ty,
                        "#[write] needs an f32, f64, integer or bool field; \
                         composite fields are read-only (use #[read])",
                    ));
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
            if classify_type(&field.ty) == FieldKind::Composite {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "#[init_to] needs an f32, f64, integer or bool field",
                ));
            }
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
                        if _gauge::resolvable::float_differs(self.#name as f32, _val) {
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
                    if _gauge::resolvable::AttributeResolvable::should_resolve(
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
                FieldKind::Float => {
                    let ty = &f.ty;
                    quote! {
                        self.#name = (#val_expr) as #ty;
                    }
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
                    _gauge::resolvable::AttributeResolvable::resolve(
                        &mut self.#name, #path, attrs,
                    );
                },
            }
        }).collect();

        quote! {
            impl _gauge::derived::AttributeDerived for #struct_name {
                fn should_update(
                    &self,
                    attrs: &_gauge::attributes::Attributes,
                ) -> bool {
                    #(#should_update_checks)*
                    false
                }

                fn update_from_attributes(
                    &mut self,
                    attrs: &_gauge::attributes::Attributes,
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
                        if _gauge::resolvable::float_differs(self.#name as f32, _val) {
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
                    if _gauge::resolvable::AttributeResolvable::should_resolve(
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
                    attributes.set_base(entity, #path, self.#name as f32);
                },
                FieldKind::Integer => quote! {
                    attributes.set_base(entity, #path, self.#name as f32);
                },
                FieldKind::Bool => quote! {
                    attributes.set_base(entity, #path, if self.#name { 1.0 } else { 0.0 });
                },
                // Rejected with a spanned error while parsing the fields.
                FieldKind::Composite => quote! {},
            }
        }).collect();

        quote! {
            impl _gauge::derived::WriteBack for #struct_name {
                fn should_write_back(
                    &self,
                    attrs: &_gauge::attributes::Attributes,
                ) -> bool {
                    #(#should_writeback_checks)*
                    false
                }

                fn write_back<F: ::bevy::ecs::query::QueryFilter>(
                    &self,
                    entity: ::bevy::prelude::Entity,
                    attributes: &mut _gauge::attributes_mut::AttributesMut<'_, '_, F>,
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
                    attributes.set_base(entity, #path, self.#name as f32);
                },
                FieldKind::Integer => quote! {
                    attributes.set_base(entity, #path, self.#name as f32);
                },
                FieldKind::Bool => quote! {
                    attributes.set_base(entity, #path, if self.#name { 1.0 } else { 0.0 });
                },
                // Rejected with a spanned error while parsing the fields.
                FieldKind::Composite => quote! {},
            }
        }).collect();

        quote! {
            impl _gauge::derived::InitTo for #struct_name {
                fn init_to_attributes<F: ::bevy::ecs::query::QueryFilter>(
                    &self,
                    entity: ::bevy::prelude::Entity,
                    attributes: &mut _gauge::attributes_mut::AttributesMut<'_, '_, F>,
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
                FieldKind::Float => {
                    let ty = &f.ty;
                    quote! {
                        self.#name = attrs.value(#path) as #ty;
                    }
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
                    _gauge::resolvable::AttributeResolvable::resolve(
                        &mut self.#name, #path, attrs,
                    );
                },
            }
        }).collect();

        quote! {
            impl _gauge::derived::InitFrom for #struct_name {
                fn init_from_attributes(
                    &mut self,
                    attrs: &_gauge::attributes::Attributes,
                ) {
                    #(#init_assignments)*
                }
            }
        }
    } else {
        TokenStream::new()
    };

    let inventory_submits = {
        let registrations = [
            (has_reads, quote! { attribute_derived }),
            (has_writes, quote! { write_back }),
            (has_init_to, quote! { init_to }),
            (has_init_from, quote! { init_from }),
        ]
        .into_iter()
        .filter(|(present, _)| *present)
        .map(|(_, kind)| quote! { _gauge::_register_attribute!(#kind, #struct_name); });

        quote! { #(#registrations)* }
    };

    let gauge = crate::path::gauge_root();
    Ok(quote! {
        #[allow(clippy::unnecessary_cast)]
        const _: () = {
            use #gauge as _gauge;
            #attribute_derived_impl
            #write_back_impl
            #init_to_impl
            #init_from_impl
            #inventory_submits
        };
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
                syn::Error::new_spanned(list, "expected at least a path string")
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
                    return Err(syn::Error::new_spanned(list, "expected `,` after path"));
                }
                let rest: proc_macro2::TokenStream = iter.collect();
                if rest.is_empty() {
                    return Err(syn::Error::new_spanned(
                        list,
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
