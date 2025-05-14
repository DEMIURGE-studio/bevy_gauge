use quote::{format_ident, quote};
use syn::{braced, parse_macro_input, Ident};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// The procedural macro definition.
pub fn define_tags(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input as multiple top-level categories
    let root = parse_macro_input!(input as TagRoot);
    
    // We use a counter to assign unique bits to each leaf.
    let mut counter = 0u32;
    let mut constants = Vec::new();
    let mut match_arms = Vec::new();
    let mut tag_info_list: Vec<TagInfo> = Vec::new();
    
    // Process each top-level category
    for category in &root.categories {
        let category_name = category.name.to_string();
        let category_const = format_ident!("{}", category_name.to_uppercase());
        
        // Process this category and its children
        let category_expr = gen_constants(
            category, 
            &mut counter, 
            &mut constants, 
            &mut match_arms, 
            &mut tag_info_list, 
            None
        );
        
        // Store the category itself
        tag_info_list.push(TagInfo {
            _name: category_name,
            const_name: category_const.to_string(),
            _bit_expr: category_expr.clone(),
            parent_category: None,
        });
    }
    
    // Create category mapping arms
    let mut category_arms = Vec::new();
    for info in &tag_info_list {
        if let Some(parent) = &info.parent_category {
            let tag_const = format_ident!("{}", info.const_name);
            let parent_const = format_ident!("{}", parent);
            category_arms.push(quote! {
                #tag_const => #parent_const
            });
        }
    }
    
    // Build the output tokens as regular functions, not in a module
    let expanded = quote! {
        // Generated constant definitions.
        #(#constants)*
        
        // Match a tag string to its constant.
        pub fn match_tag(tag: &str) -> u32 {
            match tag {
                #(#match_arms),*,
                _ => 0,
            }
        }
        
        // Match a tag constant to its category constant.
        pub fn tag_category(tag: u32) -> u32 {
            match tag {
                #(#category_arms),*,
                // For any unrecognized tag or categories themselves, return the tag
                tag => tag,
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
            let _brace_token = braced!(content in input);
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

/// Root level container for multiple categories
struct TagRoot {
    categories: Punctuated<Tag, Comma>,
}

impl Parse for TagRoot {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let categories = input.parse_terminated(Tag::parse, Comma)?;
        Ok(TagRoot { categories })
    }
}

// A structure to hold information about each tag
struct TagInfo {
    _name: String,
    const_name: String,
    _bit_expr: proc_macro2::TokenStream,
    parent_category: Option<String>,
}

/// Recursively generates constant definitions and tracking tag hierarchy info.
fn gen_constants(
    tag: &Tag,
    counter: &mut u32,
    constants: &mut Vec<proc_macro2::TokenStream>,
    match_arms: &mut Vec<proc_macro2::TokenStream>,
    tag_info_list: &mut Vec<TagInfo>,
    parent_category: Option<String>,
) -> proc_macro2::TokenStream {
    // Convert the tag name (e.g. "fire") to uppercase (e.g. FIRE) for the constant.
    let name_str = tag.name.to_string();
    let const_ident = format_ident!("{}", name_str.to_uppercase());
    let const_name = const_ident.to_string();
    
    if tag.children.is_empty() {
        // This is a leaf: assign a unique bit.
        let bit = *counter;
        *counter += 1;
        let expr = quote! { 1 << #bit };
        
        constants.push(quote! {
            pub const #const_ident: u32 = #expr;
        });
        
        match_arms.push(quote! {
            #name_str => #const_ident
        });
        
        // Store tag info
        tag_info_list.push(TagInfo {
            _name: name_str,
            const_name,
            _bit_expr: expr.clone(),
            parent_category,
        });
        
        expr
    } else {
        // Internal node: compute the OR of all children.
        let mut child_exprs = Vec::new();
        
        // If there's no parent provided, this is a top-level category
        // Otherwise, use the parent that was passed in
        let next_parent = if parent_category.is_none() {
            Some(const_name.clone())
        } else {
            parent_category.clone()
        };
        
        for child in &tag.children {
            let child_expr = gen_constants(
                child, 
                counter, 
                constants, 
                match_arms, 
                tag_info_list, 
                next_parent.clone()
            );
            child_exprs.push(child_expr);
        }
        
        let expr = quote! { #(#child_exprs)|* };
        
        constants.push(quote! {
            pub const #const_ident: u32 = #expr;
        });
        
        match_arms.push(quote! {
            #name_str => #const_ident
        });
        
        // Store info for the category itself
        tag_info_list.push(TagInfo {
            _name: name_str,
            const_name,
            _bit_expr: expr.clone(),
            parent_category,
        });
        
        expr
    }
}