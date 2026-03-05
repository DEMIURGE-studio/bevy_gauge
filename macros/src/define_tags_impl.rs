use quote::{format_ident, quote};
use syn::{braced, parse_macro_input, Ident, Token};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

struct MacroArgs {
    struct_name: Ident,
    categories: Punctuated<Tag, Comma>,
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let struct_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let categories = input.parse_terminated(Tag::parse, Comma)?;
        Ok(MacroArgs { struct_name, categories })
    }
}

pub fn define_tags(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let args = parse_macro_input!(input as MacroArgs);
    let struct_name_ident = &args.struct_name;

    let mut counter = 0u32;
    let mut const_defs = Vec::new();
    let mut register_calls = Vec::new();

    for tag_node in &args.categories {
        gen_constants(tag_node, &mut counter, &mut const_defs, &mut register_calls);
    }

    let struct_name_str = struct_name_ident.to_string();

    let expanded = quote! {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct #struct_name_ident;

        impl #struct_name_ident {
            #(#const_defs)*

            /// Register every tag (leaves and groups) with a [`TagResolver`].
            ///
            /// Tags are registered both as short names (e.g., `"FIRE"`) and
            /// namespaced names (e.g., `"Tags::FIRE"`). If a short name
            /// collides with another namespace, the namespaced form must be used.
            pub fn register(resolver: &mut bevy_gauge::tags::TagResolver) {
                let _ns = #struct_name_str;
                #(#register_calls)*
            }
        }

        ::inventory::submit! {
            bevy_gauge::tags::TagRegistration {
                register_fn: |resolver| {
                    #struct_name_ident::register(resolver);
                }
            }
        }
    };

    expanded.into()
}

struct Tag {
    name: Ident,
    children: Vec<Tag>,
}

impl Parse for Tag {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let name: Ident = input.parse()?;
        let children = if input.peek(syn::token::Brace) {
            let content;
            let _brace_token = braced!(content in input);
            let child_list: Punctuated<Tag, Comma> =
                content.parse_terminated(Tag::parse, Comma)?;
            child_list.into_iter().collect()
        } else {
            Vec::new()
        };
        Ok(Tag { name, children })
    }
}

/// Recursively generates `TagMask` constant definitions and `resolver.register()` calls.
///
/// Returns the `TokenStream` expression for this node's mask value (used by
/// parent group nodes to OR children together).
fn gen_constants(
    tag_node: &Tag,
    counter: &mut u32,
    const_defs: &mut Vec<proc_macro2::TokenStream>,
    register_calls: &mut Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let name_str = tag_node.name.to_string();
    let const_ident = format_ident!("{}", name_str.to_uppercase());

    let mask_expr: proc_macro2::TokenStream;

    if tag_node.children.is_empty() {
        let bit_index = *counter;
        *counter += 1;
        mask_expr = quote! { bevy_gauge::tags::TagMask::bit(#bit_index) };
    } else {
        let child_exprs: Vec<_> = tag_node
            .children
            .iter()
            .map(|child| gen_constants(child, counter, const_defs, register_calls))
            .collect();

        // OR together via raw u64 bits so the result is a const expression.
        mask_expr = quote! {
            bevy_gauge::tags::TagMask::new(#(#child_exprs .0)|*)
        };
    }

    const_defs.push(quote! {
        pub const #const_ident: bevy_gauge::tags::TagMask = #mask_expr;
    });

    register_calls.push(quote! {
        resolver.register_namespaced(_ns, #name_str, Self::#const_ident);
    });

    // Return a simple reference to the const so parent OR expressions stay readable.
    quote! { Self::#const_ident }
}
