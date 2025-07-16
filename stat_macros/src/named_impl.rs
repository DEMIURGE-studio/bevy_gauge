use quote::quote;
use syn::parse_macro_input;

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