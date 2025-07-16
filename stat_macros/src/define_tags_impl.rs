use quote::{format_ident, quote};
use syn::{braced, parse_macro_input, Ident, Token};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// Input parsing structures
struct MacroArgs {
    struct_name: Ident,
    categories: Punctuated<Tag, Comma>,
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let struct_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?; // Expect a comma after the struct name
        let categories = input.parse_terminated(Tag::parse, Comma)?;
        Ok(MacroArgs { struct_name, categories })
    }
}

/// The procedural macro definition.
pub fn define_tags(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input as a struct name followed by top-level categories
    let args = parse_macro_input!(input as MacroArgs);
    let struct_name_ident = &args.struct_name;
    
    let mut counter = 0u32;
    let mut associated_const_defs = Vec::new(); // Definitions like `pub const NAME: u32 = ...;`
    let mut match_tag_arms = Vec::new();        // Arms like `"name" => Self::NAME,`
    let mut category_map_arms = Vec::new();     // Arms like `Self::NAME => Self::ROOT_NAME,`
    let mut defined_group_idents = Vec::new();  // Idents like `GROUP_NAME` for `Self::GROUP_NAME`
    
    // Process each top-level category
    for top_level_category_tag_node in &args.categories {
        let top_level_category_name_str = top_level_category_tag_node.name.to_string();
        // Const ident for the top-level category itself (e.g., DAMAGE_TYPE)
        let top_level_category_const_ident = format_ident!("{}", top_level_category_name_str.to_uppercase());
        
        // This call processes the top-level category node itself and all its descendants.
        // It populates all_const_defs, match_tag_arms, category_map_arms, and defined_group_idents.
        gen_constants(
            top_level_category_tag_node, 
            &mut counter, 
            &mut associated_const_defs, 
            &mut match_tag_arms, 
            &mut category_map_arms,
            &mut defined_group_idents,
            &top_level_category_const_ident, // This is the root category for itself and its children
            true, // Mark that this call is for processing a top-level category node
        );
    }
    
    let expanded = quote! {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct #struct_name_ident;

        impl #struct_name_ident {
            #(#associated_const_defs)*
            
            fn generated_tag_category(tag_value: u32) -> u32 {
                match tag_value {
                    #(#category_map_arms)*
                    val => val, 
                }
            }
        }
        
        impl bevy_gauge::tags::TagSet for #struct_name_ident {
            fn match_tag(&self, tag_str: &str) -> u32 {
                match tag_str.trim().to_lowercase().as_str() {
                    #(#match_tag_arms)*
                    _ => 0,
                }
            }
            
            fn tag_category_for_bit(&self, tag_bit: u32) -> u32 {
                Self::generated_tag_category(tag_bit)
            }
            
            fn tag_category_for_group(&self, group_tag: u32) -> u32 {
                Self::generated_tag_category(group_tag)
            }
            
            fn all_defined_groups(&self) -> &'static [u32] {
                &[#(Self::#defined_group_idents),*]
            }
        }
    };
    
    expanded.into()
}

/// A tag node with a name and optional children.
struct Tag {
    name: Ident,
    children: Vec<Tag>,
}

impl Parse for Tag {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        // Parse the tag name (an identifier).
        let name: Ident = input.parse()?;
        let children = if input.peek(syn::token::Brace) {
            // Capture the content inside the braces.
            let content;
            let _brace_token = braced!(content in input); // syn::token::Brace implied by braced!
            // Parse a comma-separated list of child tags.
            let child_list: Punctuated<Tag, Comma> =
                content.parse_terminated(Tag::parse, Comma)?;
            child_list.into_iter().collect()
        } else {
            Vec::new()
        };
        Ok(Tag { name, children })
    }
}

/// Recursively generates constant definitions and populates lists for the TagSet implementation.
fn gen_constants(
    tag_node: &Tag, // Current tag node being processed (e.g., 'fire', 'elemental', 'damage_type')
    counter: &mut u32, // Bit counter for leaf nodes
    associated_const_defs: &mut Vec<proc_macro2::TokenStream>, // Accumulates all `pub const X: u32 = ...;`
    match_tag_arms: &mut Vec<proc_macro2::TokenStream>, // Accumulates `"name" => CONST_IDENT,`
    // Accumulates `CONST_IDENT => ROOT_CATEGORY_CONST_IDENT,` for non-root items
    category_map_arms: &mut Vec<proc_macro2::TokenStream>,
    // Accumulates `CONST_IDENT` for all group tags (non-leaf nodes)
    defined_group_idents: &mut Vec<Ident>,
    // Const ident of the root category this tag_node belongs to (e.g., DAMAGE_TYPE).
    root_category_const_ident: &Ident,
    // True if `tag_node` is one of the initial top-level categories (e.g., 'damage_type').
    is_processing_a_top_level_category_node: bool,
) -> proc_macro2::TokenStream { // Returns the bitmask expression for the current `tag_node`
    let name_str = tag_node.name.to_string();
    let current_const_ident = format_ident!("{}", name_str.to_uppercase());
    
    // Add to match_tag arms (e.g., "fire" => FIRE,)
    match_tag_arms.push(quote! { #name_str => Self::#current_const_ident, });
    
    let current_tag_expr: proc_macro2::TokenStream;
    
    if tag_node.children.is_empty() {
        // Leaf node (e.g., fire, cold, physical)
        let bit_index = *counter;
        *counter += 1;
        current_tag_expr = quote! { 1u32 << #bit_index };
        
        // If this leaf is NOT itself a top-level category, map it to its root category in `generated_tag_category`.
        // e.g., For FIRE (leaf, not top-level), add `FIRE => DAMAGE_TYPE,`.
        // If 'tag_node' is a top-level category that happens to be a leaf (e.g. `define_tags!(S, lonely_top_level_tag)`),
        // `is_processing_a_top_level_category_node` will be true, and it won't be added here.
        // It will correctly map to itself in `generated_tag_category` via the `val => val` fallback.
        if !is_processing_a_top_level_category_node {
             category_map_arms.push(quote! { Self::#current_const_ident => Self::#root_category_const_ident, });
        }
        
    } else {
        // Internal node OR a top-level category node that has children
        // (e.g., 'elemental', or 'damage_type' itself).
        
        // This node represents a group. Add its const ident to `defined_group_idents`.
        // This includes top-level categories (like DAMAGE_TYPE) and intermediate groups (like ELEMENTAL).
        defined_group_idents.push(current_const_ident.clone());
        
        // If this group is an intermediate group (i.e., NOT a top-level category itself),
        // map it to its root category in `generated_tag_category`.
        // e.g., For ELEMENTAL (intermediate group), add `ELEMENTAL => DAMAGE_TYPE,`.
        // Top-level categories (like DAMAGE_TYPE) are not mapped here; they use the `val => val` fallback.
        if !is_processing_a_top_level_category_node {
            category_map_arms.push(quote! { Self::#current_const_ident => Self::#root_category_const_ident, });
        }
        
        let mut child_exprs = Vec::new();
        for child_tag_node in &tag_node.children {
            // For children, 'is_processing_a_top_level_category_node' is always false.
            // The 'root_category_const_ident' (e.g. DAMAGE_TYPE) is passed down.
            let child_expr = gen_constants(
                child_tag_node, 
                counter, 
                associated_const_defs, 
                match_tag_arms, 
                category_map_arms, 
                defined_group_idents,
                root_category_const_ident, // Pass down the same root category
                false, // Children are never top-level category nodes in this recursive call context
            );
            child_exprs.push(child_expr);
        }
        
        // Combine child expressions with OR. If no children (empty group), result is 0.
        if child_exprs.is_empty() {
            current_tag_expr = quote! { 0u32 };
        } else {
            current_tag_expr = quote! { #(#child_exprs)|* };
        }
    }
    
    // Define the `pub const` for the current tag_node (leaf, intermediate group, or top-level category).
    associated_const_defs.push(quote! {
        pub const #current_const_ident: u32 = #current_tag_expr;
    });
    
    current_tag_expr // Return the expression for this tag (used by parent nodes)
}