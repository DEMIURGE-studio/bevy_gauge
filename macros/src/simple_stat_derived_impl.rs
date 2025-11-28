use quote::quote;
use syn::parse_macro_input;

pub fn derive_simple_stat_derived(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;
    
    // Convert the struct name to a string
    let name_str = name.to_string();
    
    let expanded = quote! {
        impl bevy_gauge::prelude::StatDerived for #name {
            fn should_update(&self, stats: &bevy_gauge::prelude::Stats) -> bool {
                let value = stats.get(#name_str);
                value != self.0
            }
        
            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::Stats) {
                let value = stats.get(#name_str);
                self.0 = value;
            }
        }
    };

    proc_macro::TokenStream::from(expanded).into()
}