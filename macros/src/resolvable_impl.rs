use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Fields, Ident, Type};

use crate::field_kind::{classify_type, FieldKind};

fn is_skip(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("skip"))
}

/// How generated code reaches one field's attribute.
struct FieldAccess {
    /// An `Option<f32>` expression: the attribute's current value, or `None`
    /// if the entity doesn't have it (the field is then left alone).
    value: TokenStream,
    /// A `&str` expression: the attribute path, for delegating to a composite.
    path: TokenStream,
}

impl FieldAccess {
    /// The field is the whole value (a single resolvable field): use the prefix itself.
    fn transparent() -> Self {
        Self {
            value: quote! { attrs.try_value(prefix) },
            path: quote! { prefix },
        }
    }

    /// The field's entry in the `__paths` slice fetched from the type's cache.
    fn cached(index: usize) -> Self {
        Self {
            value: quote! { attrs.try_get(__paths[#index].id) },
            path: quote! { &*__paths[#index].path },
        }
    }
}

/// `should_resolve` check for one field. `target` is a place expression for
/// the field (`self.name` or `*binding`).
fn gen_should_resolve(target: &TokenStream, kind: FieldKind, ty: &Type, access: &FieldAccess) -> TokenStream {
    let FieldAccess { value, path } = access;
    match kind {
        FieldKind::Float => quote! {
            if (#value).is_some_and(|__v| _gauge::resolvable::float_differs(#target as f32, __v)) {
                return true;
            }
        },
        FieldKind::Integer => quote! {
            if (#value).is_some_and(|__v| #target != __v.round() as #ty) {
                return true;
            }
        },
        FieldKind::Bool => quote! {
            if (#value).is_some_and(|__v| #target != (__v != 0.0)) {
                return true;
            }
        },
        FieldKind::Composite => quote! {
            if _gauge::resolvable::AttributeResolvable::should_resolve(&#target, #path, attrs) {
                return true;
            }
        },
    }
}

/// `resolve` assignment for one field. `target` as in [`gen_should_resolve`].
fn gen_resolve(target: &TokenStream, kind: FieldKind, ty: &Type, access: &FieldAccess) -> TokenStream {
    let FieldAccess { value, path } = access;
    match kind {
        FieldKind::Float => quote! {
            if let Some(__v) = #value { #target = __v as #ty; }
        },
        FieldKind::Integer => quote! {
            if let Some(__v) = #value { #target = __v.round() as #ty; }
        },
        FieldKind::Bool => quote! {
            if let Some(__v) = #value { #target = __v != 0.0; }
        },
        FieldKind::Composite => quote! {
            _gauge::resolvable::AttributeResolvable::resolve(&mut #target, #path, attrs);
        },
    }
}

/// A named field that takes part in resolution.
struct ResolvableField<'a> {
    name: &'a Ident,
    ty: &'a Type,
    kind: FieldKind,
}

fn resolvable_fields(named: &syn::FieldsNamed) -> Vec<ResolvableField<'_>> {
    named
        .named
        .iter()
        .filter(|f| !is_skip(f))
        .map(|f| ResolvableField {
            name: f.ident.as_ref().unwrap(),
            ty: &f.ty,
            kind: classify_type(&f.ty),
        })
        .collect()
}

/// Checks and assignments for a set of named fields, plus what they need set
/// up first.
struct NamedFields {
    /// A `static` path cache, when the fields need one.
    cache: TokenStream,
    /// Statement binding `__paths` from the cache, when the fields need one.
    fetch: TokenStream,
    should: Vec<TokenStream>,
    resolve: Vec<TokenStream>,
}

/// One resolvable field reads the prefix directly; several each get
/// `prefix.field`, cached under `cache_name`.
fn gen_named_fields(
    fields: &[ResolvableField],
    cache_name: &Ident,
    target: impl Fn(&Ident) -> TokenStream,
) -> NamedFields {
    let transparent = fields.len() == 1;
    let (cache, fetch) = if transparent || fields.is_empty() {
        (quote! {}, quote! {})
    } else {
        let suffixes = fields.iter().map(|f| format!(".{}", f.name));
        (
            quote! {
                static #cache_name: _gauge::resolvable::FieldPathCache =
                    _gauge::resolvable::FieldPathCache::new(&[#(#suffixes),*]);
            },
            quote! { let __paths = #cache_name.get(prefix); },
        )
    };
    let access = |i: usize| {
        if transparent { FieldAccess::transparent() } else { FieldAccess::cached(i) }
    };
    let should = fields
        .iter()
        .enumerate()
        .map(|(i, f)| gen_should_resolve(&target(f.name), f.kind, f.ty, &access(i)))
        .collect();
    let resolve = fields
        .iter()
        .enumerate()
        .map(|(i, f)| gen_resolve(&target(f.name), f.kind, f.ty, &access(i)))
        .collect();
    NamedFields { cache, fetch, should, resolve }
}

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let (statics, should_body, resolve_body) = match &input.data {
        syn::Data::Struct(data) => derive_struct(&data.fields)?,
        syn::Data::Enum(data) => derive_enum(data)?,
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "AttributeResolvable cannot be derived on unions",
            ))
        }
    };

    let gauge = crate::path::gauge_root();
    Ok(quote! {
        const _: () = {
            use #gauge as _gauge;

            #statics

            impl #impl_generics _gauge::resolvable::AttributeResolvable for #name #ty_generics #where_clause {
                #[allow(unused_variables, clippy::unnecessary_cast)]
                fn should_resolve(
                    &self,
                    prefix: &str,
                    attrs: &_gauge::attributes::Attributes,
                ) -> bool {
                    #should_body
                    false
                }

                #[allow(unused_variables, clippy::unnecessary_cast)]
                fn resolve(
                    &mut self,
                    prefix: &str,
                    attrs: &_gauge::attributes::Attributes,
                ) {
                    #resolve_body
                }
            }
        };
    })
}

/// Returns (statics, should_resolve body, resolve body).
fn derive_struct(fields: &Fields) -> syn::Result<(TokenStream, TokenStream, TokenStream)> {
    match fields {
        Fields::Named(named) => {
            let fields = resolvable_fields(named);
            let NamedFields { cache, fetch, should, resolve } =
                gen_named_fields(&fields, &format_ident!("__GAUGE_PATHS"), |n| quote! { self.#n });
            Ok((
                cache,
                quote! { #fetch #(#should)* },
                quote! { #fetch #(#resolve)* },
            ))
        }
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            // Newtype — transparent
            let ty = &unnamed.unnamed[0].ty;
            let kind = classify_type(ty);
            let target = quote! { self.0 };
            let access = FieldAccess::transparent();
            Ok((
                quote! {},
                gen_should_resolve(&target, kind, ty, &access),
                gen_resolve(&target, kind, ty, &access),
            ))
        }
        Fields::Unnamed(unnamed) => Err(syn::Error::new_spanned(
            unnamed,
            "AttributeResolvable requires named fields for multi-field structs",
        )),
        // Unit struct — always no-op
        Fields::Unit => Ok((quote! {}, quote! {}, quote! {})),
    }
}

/// Returns (statics, should_resolve body, resolve body). Fields are matched by
/// name; the variant name is not part of the path.
fn derive_enum(data: &syn::DataEnum) -> syn::Result<(TokenStream, TokenStream, TokenStream)> {
    let mut statics = Vec::new();
    let mut should_arms = Vec::new();
    let mut resolve_arms = Vec::new();

    for (index, variant) in data.variants.iter().enumerate() {
        let vname = &variant.ident;

        match &variant.fields {
            Fields::Unit => {
                should_arms.push(quote! { Self::#vname => {} });
                resolve_arms.push(quote! { Self::#vname => {} });
            }
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                // Single-field tuple variant — transparent
                let field = &unnamed.unnamed[0];
                if is_skip(field) {
                    should_arms.push(quote! { Self::#vname(_) => {} });
                    resolve_arms.push(quote! { Self::#vname(_) => {} });
                    continue;
                }
                let kind = classify_type(&field.ty);
                let target = quote! { *_v };
                let access = FieldAccess::transparent();
                let should = gen_should_resolve(&target, kind, &field.ty, &access);
                let resolve = gen_resolve(&target, kind, &field.ty, &access);
                should_arms.push(quote! { Self::#vname(_v) => { #should } });
                resolve_arms.push(quote! { Self::#vname(_v) => { #resolve } });
            }
            Fields::Unnamed(unnamed) => {
                return Err(syn::Error::new_spanned(
                    unnamed,
                    format!(
                        "AttributeResolvable: variant `{vname}` has multiple unnamed fields. \
                         Use named fields instead."
                    ),
                ));
            }
            Fields::Named(named) => {
                let fields = resolvable_fields(named);
                let NamedFields { cache, fetch, should, resolve } = gen_named_fields(
                    &fields,
                    &format_ident!("__GAUGE_PATHS_{}", index),
                    |n| quote! { *#n },
                );
                statics.push(cache);

                let pattern: Vec<TokenStream> = named
                    .named
                    .iter()
                    .map(|f| {
                        let n = f.ident.as_ref().unwrap();
                        if is_skip(f) { quote! { #n: _ } } else { quote! { #n } }
                    })
                    .collect();

                should_arms.push(quote! {
                    Self::#vname { #(#pattern),* } => { #fetch #(#should)* }
                });
                resolve_arms.push(quote! {
                    Self::#vname { #(#pattern),* } => { #fetch #(#resolve)* }
                });
            }
        }
    }

    Ok((
        quote! { #(#statics)* },
        quote! { match self { #(#should_arms)* } },
        quote! { match self { #(#resolve_arms)* } },
    ))
}
