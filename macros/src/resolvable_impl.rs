use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, Ident, Type};

/// Classify a type as a known terminal or composite (delegates to AttributeResolvable).
#[derive(Clone, Copy, PartialEq)]
enum FieldKind {
    /// f32, f64 — known terminal
    Float,
    /// Integer types — known terminal
    Integer,
    /// bool — known terminal
    Bool,
    /// Unknown type — delegate to AttributeResolvable
    Composite,
}

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

struct ResolvableField {
    name: Ident,
    ty: Type,
    kind: FieldKind,
}

fn is_skip(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("skip"))
}

/// Generate `should_resolve` check for a single field at a given path expression.
fn gen_should_resolve(field: &ResolvableField, path_expr: &TokenStream) -> TokenStream {
    let name = &field.name;
    match field.kind {
        FieldKind::Float => quote! {
            if (self.#name - attrs.value(#path_expr)).abs() > f32::EPSILON {
                return true;
            }
        },
        FieldKind::Integer => {
            let ty = &field.ty;
            quote! {
                if self.#name != attrs.value(#path_expr).round() as #ty {
                    return true;
                }
            }
        }
        FieldKind::Bool => quote! {
            if self.#name != (attrs.value(#path_expr) != 0.0) {
                return true;
            }
        },
        FieldKind::Composite => quote! {
            if ::bevy_gauge::resolvable::AttributeResolvable::should_resolve(
                &self.#name, #path_expr, attrs,
            ) {
                return true;
            }
        },
    }
}

/// Generate `resolve` assignment for a single field at a given path expression.
fn gen_resolve(field: &ResolvableField, path_expr: &TokenStream) -> TokenStream {
    let name = &field.name;
    match field.kind {
        FieldKind::Float => quote! {
            self.#name = attrs.value(#path_expr);
        },
        FieldKind::Integer => {
            let ty = &field.ty;
            quote! {
                self.#name = attrs.value(#path_expr).round() as #ty;
            }
        }
        FieldKind::Bool => quote! {
            self.#name = attrs.value(#path_expr) != 0.0;
        },
        FieldKind::Composite => quote! {
            ::bevy_gauge::resolvable::AttributeResolvable::resolve(
                &mut self.#name, #path_expr, attrs,
            );
        },
    }
}

/// Generate should_resolve/resolve for a variant's binding name (enum match arm).
fn gen_should_resolve_binding(
    binding: &Ident,
    kind: FieldKind,
    ty: &Type,
    path_expr: &TokenStream,
) -> TokenStream {
    match kind {
        FieldKind::Float => quote! {
            if (*#binding - attrs.value(#path_expr)).abs() > f32::EPSILON {
                return true;
            }
        },
        FieldKind::Integer => quote! {
            if *#binding != attrs.value(#path_expr).round() as #ty {
                return true;
            }
        },
        FieldKind::Bool => quote! {
            if *#binding != (attrs.value(#path_expr) != 0.0) {
                return true;
            }
        },
        FieldKind::Composite => quote! {
            if ::bevy_gauge::resolvable::AttributeResolvable::should_resolve(
                #binding, #path_expr, attrs,
            ) {
                return true;
            }
        },
    }
}

fn gen_resolve_binding(
    binding: &Ident,
    kind: FieldKind,
    ty: &Type,
    path_expr: &TokenStream,
) -> TokenStream {
    match kind {
        FieldKind::Float => quote! {
            *#binding = attrs.value(#path_expr);
        },
        FieldKind::Integer => quote! {
            *#binding = attrs.value(#path_expr).round() as #ty;
        },
        FieldKind::Bool => quote! {
            *#binding = attrs.value(#path_expr) != 0.0;
        },
        FieldKind::Composite => quote! {
            ::bevy_gauge::resolvable::AttributeResolvable::resolve(
                #binding, #path_expr, attrs,
            );
        },
    }
}

/// Build the path expression for a field. If `transparent` is true (single
/// resolvable field), use the prefix directly. Otherwise append `.field_name`.
fn field_path_expr(field_name: &str, transparent: bool) -> TokenStream {
    if transparent {
        quote! { prefix }
    } else {
        let suffix = format!(".{field_name}");
        quote! { &format!("{prefix}{}", #suffix) }
    }
}

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;

    match &input.data {
        syn::Data::Struct(data) => derive_struct(name, &data.fields),
        syn::Data::Enum(data) => derive_enum(name, data),
        syn::Data::Union(_) => Err(syn::Error::new_spanned(
            name,
            "AttributeResolvable cannot be derived on unions",
        )),
    }
}

fn derive_struct(name: &Ident, fields: &Fields) -> syn::Result<TokenStream> {
    match fields {
        Fields::Named(named) => {
            let resolvable: Vec<ResolvableField> = named
                .named
                .iter()
                .filter(|f| !is_skip(f))
                .map(|f| ResolvableField {
                    name: f.ident.clone().unwrap(),
                    ty: f.ty.clone(),
                    kind: classify_type(&f.ty),
                })
                .collect();

            let transparent = resolvable.len() == 1;

            let should_checks: Vec<TokenStream> = resolvable
                .iter()
                .map(|f| {
                    let path = field_path_expr(&f.name.to_string(), transparent);
                    gen_should_resolve(f, &path)
                })
                .collect();

            let resolve_stmts: Vec<TokenStream> = resolvable
                .iter()
                .map(|f| {
                    let path = field_path_expr(&f.name.to_string(), transparent);
                    gen_resolve(f, &path)
                })
                .collect();

            Ok(quote! {
                impl ::bevy_gauge::resolvable::AttributeResolvable for #name {
                    fn should_resolve(
                        &self,
                        prefix: &str,
                        attrs: &::bevy_gauge::attributes::Attributes,
                    ) -> bool {
                        #(#should_checks)*
                        false
                    }

                    fn resolve(
                        &mut self,
                        prefix: &str,
                        attrs: &::bevy_gauge::attributes::Attributes,
                    ) {
                        #(#resolve_stmts)*
                    }
                }
            })
        }
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            // Newtype — transparent
            let inner_ty = &unnamed.unnamed[0].ty;
            let kind = classify_type(inner_ty);

            let should_body = gen_should_resolve_binding(
                &syn::parse_quote!(_inner),
                kind,
                inner_ty,
                &quote! { prefix },
            );
            let resolve_body = gen_resolve_binding(
                &syn::parse_quote!(_inner),
                kind,
                inner_ty,
                &quote! { prefix },
            );

            Ok(quote! {
                impl ::bevy_gauge::resolvable::AttributeResolvable for #name {
                    fn should_resolve(
                        &self,
                        prefix: &str,
                        attrs: &::bevy_gauge::attributes::Attributes,
                    ) -> bool {
                        let _inner = &self.0;
                        #should_body
                        false
                    }

                    fn resolve(
                        &mut self,
                        prefix: &str,
                        attrs: &::bevy_gauge::attributes::Attributes,
                    ) {
                        let _inner = &mut self.0;
                        #resolve_body
                    }
                }
            })
        }
        Fields::Unnamed(unnamed) => {
            Err(syn::Error::new_spanned(
                unnamed,
                "AttributeResolvable requires named fields for multi-field structs",
            ))
        }
        Fields::Unit => {
            // Unit struct — always no-op
            Ok(quote! {
                impl ::bevy_gauge::resolvable::AttributeResolvable for #name {
                    fn should_resolve(
                        &self,
                        _prefix: &str,
                        _attrs: &::bevy_gauge::attributes::Attributes,
                    ) -> bool {
                        false
                    }

                    fn resolve(
                        &mut self,
                        _prefix: &str,
                        _attrs: &::bevy_gauge::attributes::Attributes,
                    ) {}
                }
            })
        }
    }
}

fn derive_enum(name: &Ident, data: &syn::DataEnum) -> syn::Result<TokenStream> {
    let mut should_arms = Vec::new();
    let mut resolve_arms = Vec::new();

    for variant in &data.variants {
        let vname = &variant.ident;

        match &variant.fields {
            Fields::Unit => {
                // Unit variant — no-op
                should_arms.push(quote! { Self::#vname => {} });
                resolve_arms.push(quote! { Self::#vname => {} });
            }
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                // Single-field tuple variant — transparent
                let ty = &unnamed.unnamed[0].ty;
                if is_skip(&unnamed.unnamed[0]) {
                    should_arms.push(quote! { Self::#vname(_) => {} });
                    resolve_arms.push(quote! { Self::#vname(_) => {} });
                    continue;
                }
                let kind = classify_type(ty);
                let binding: Ident = syn::parse_quote!(_v);

                let should_check =
                    gen_should_resolve_binding(&binding, kind, ty, &quote! { prefix });
                let resolve_stmt =
                    gen_resolve_binding(&binding, kind, ty, &quote! { prefix });

                should_arms.push(quote! {
                    Self::#vname(#binding) => { #should_check }
                });
                resolve_arms.push(quote! {
                    Self::#vname(#binding) => { #resolve_stmt }
                });
            }
            Fields::Unnamed(unnamed) => {
                return Err(syn::Error::new_spanned(
                    unnamed,
                    format!(
                        "AttributeResolvable: variant `{}` has multiple unnamed fields. \
                         Use named fields instead.",
                        vname
                    ),
                ));
            }
            Fields::Named(named) => {
                let resolvable: Vec<(&Ident, &Type, FieldKind)> = named
                    .named
                    .iter()
                    .filter(|f| !is_skip(f))
                    .map(|f| {
                        let ident = f.ident.as_ref().unwrap();
                        (ident, &f.ty, classify_type(&f.ty))
                    })
                    .collect();

                let skipped: Vec<&Ident> = named
                    .named
                    .iter()
                    .filter(|f| is_skip(f))
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();

                let transparent = resolvable.len() == 1;

                // Build pattern bindings
                let all_field_names: Vec<&Ident> = named
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();

                let pattern_bindings: Vec<TokenStream> = all_field_names
                    .iter()
                    .map(|n| {
                        if skipped.contains(n) {
                            quote! { #n: _ }
                        } else {
                            quote! { #n }
                        }
                    })
                    .collect();

                let should_checks: Vec<TokenStream> = resolvable
                    .iter()
                    .map(|(field_name, ty, kind)| {
                        let path =
                            field_path_expr(&field_name.to_string(), transparent);
                        gen_should_resolve_binding(field_name, *kind, ty, &path)
                    })
                    .collect();

                let resolve_stmts: Vec<TokenStream> = resolvable
                    .iter()
                    .map(|(field_name, ty, kind)| {
                        let path =
                            field_path_expr(&field_name.to_string(), transparent);
                        gen_resolve_binding(field_name, *kind, ty, &path)
                    })
                    .collect();

                should_arms.push(quote! {
                    Self::#vname { #(#pattern_bindings),* } => {
                        #(#should_checks)*
                    }
                });
                resolve_arms.push(quote! {
                    Self::#vname { #(#pattern_bindings),* } => {
                        #(#resolve_stmts)*
                    }
                });
            }
        }
    }

    Ok(quote! {
        impl ::bevy_gauge::resolvable::AttributeResolvable for #name {
            fn should_resolve(
                &self,
                prefix: &str,
                attrs: &::bevy_gauge::attributes::Attributes,
            ) -> bool {
                match self {
                    #(#should_arms)*
                }
                false
            }

            fn resolve(
                &mut self,
                prefix: &str,
                attrs: &::bevy_gauge::attributes::Attributes,
            ) {
                match self {
                    #(#resolve_arms)*
                }
            }
        }
    })
}
