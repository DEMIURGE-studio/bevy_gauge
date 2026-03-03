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

struct BoundField {
    name: Ident,
    #[allow(dead_code)]
    ty: Type,
    direction: Direction,
    path: String,
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

    for field in &named.named {
        let field_name = field.ident.as_ref().unwrap();

        for attr in &field.attrs {
            let (direction, path) = if attr.path().is_ident("read") {
                (Direction::ReadFrom, parse_optional_path(attr)?)
            } else if attr.path().is_ident("write") {
                (Direction::WriteTo, parse_optional_path(attr)?)
            } else {
                continue;
            };

            let resolved_path = match path {
                AttributePath::Explicit(lit) => lit,
                AttributePath::Auto => format!("{}.{}", struct_name_str, field_name),
            };

            bound_fields.push(BoundField {
                name: field_name.clone(),
                ty: field.ty.clone(),
                direction,
                path: resolved_path,
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
        #attribute_derived_impl
        #write_back_impl
        #inventory_submits
    })
}

fn parse_optional_path(attr: &syn::Attribute) -> syn::Result<AttributePath> {
    match &attr.meta {
        syn::Meta::Path(_) => Ok(AttributePath::Auto),
        syn::Meta::List(list) => {
            let lit: LitStr = syn::parse2(list.tokens.clone())?;
            Ok(AttributePath::Explicit(lit.value()))
        }
        syn::Meta::NameValue(_) => Err(syn::Error::new_spanned(
            attr,
            "expected `#[read]` or `#[read(\"path\")]`",
        )),
    }
}
