extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result as SynResult};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Error, LitFloat, LitStr, Token};

// Intermediate parsing structures

struct StatAssignment {
    path: LitStr,
    _arrow: Token![=>],
    value: StatValue,
}

enum StatValue {
    Single(ValueItem),
    Array(syn::ExprArray),
}

enum ValueItem {
    Literal(LitFloat),
    StrExpression(LitStr),
    // To support arbitrary expressions if needed in the future, though current focus is f32/str
    // OtherExpr(Expr),
}

impl Parse for StatAssignment {
    fn parse(input: ParseStream) -> SynResult<Self> {
        Ok(StatAssignment {
            path: input.parse()?,
            _arrow: input.parse()?,
            value: input.parse()?,
        })
    }
}

impl Parse for StatValue {
    fn parse(input: ParseStream) -> SynResult<Self> {
        if input.peek(syn::token::Bracket) {
            let array: syn::ExprArray = input.parse()?;
            Ok(StatValue::Array(array))
        } else {
            Ok(StatValue::Single(input.parse()?))
        }
    }
}

impl Parse for ValueItem {
    fn parse(input: ParseStream) -> SynResult<Self> {
        if input.peek(LitFloat) {
            Ok(ValueItem::Literal(input.parse()?))
        } else if input.peek(LitStr) {
            Ok(ValueItem::StrExpression(input.parse()?))
        } else {
            // Attempt to parse as a general expression if it's not a float or string literal directly.
            // This could be useful if an expression like `Expression::new(...)` is passed directly.
            // However, current usage seems to rely on direct f32 or string for expressions.
            // For simplicity and to match current expectations for f32 / expr strings:
            Err(Error::new(
                input.span(),
                "expected a float literal (e.g., 10.0) or a string literal for an expression (e.g., \"Str / 2.0\")",
            ))
        }
    }
}

struct MacroInput {
    assignments: Punctuated<StatAssignment, Token![,]>
}

impl Parse for MacroInput {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let assignments = Punctuated::<StatAssignment, Token![,]>::parse_terminated(input)?;
        Ok(MacroInput { assignments })
    }
}

fn generate_modifier_set_add_calls(assignments: &Punctuated<StatAssignment, Token![,]>) -> Vec<proc_macro2::TokenStream> {
    let mut add_calls = Vec::new();
    for assignment in assignments {
        let path_str = &assignment.path;
        match &assignment.value {
            StatValue::Single(item) => {
                match item {
                    ValueItem::Literal(lit_float) => {
                        add_calls.push(quote! {
                            let processed_path = bevy_gauge::prelude::Konfig::process_path(#path_str);
                            modifier_set.add(&processed_path, #lit_float);
                        });
                    }
                    ValueItem::StrExpression(lit_str) => {
                        add_calls.push(quote! {
                            let processed_path = bevy_gauge::prelude::Konfig::process_path(#path_str);
                            modifier_set.add(&processed_path, #lit_str);
                        });
                    }
                }
            }
            StatValue::Array(expr_array) => {
                for elem_expr in &expr_array.elems {
                    // We need to convert elem_expr back to something we can use with quote!
                    // This part is tricky because elem_expr is already an Expr.
                    // We want to pass the original literal (f32 or &str) to .add()
                    // This proc macro is better at parsing structure than re-interpreting Expr types.
                    // The current ValueItem::parse will ensure items are f32 or string literals if not in an array.
                    // For arrays, syn::ExprArray gives Expr. We need to ensure these Exprs are what we expect.

                    // A simpler way for arrays in proc_macro, if we stick to f32/str, is to parse them explicitly:
                    // Modify StatValue::Array to hold Punctuated<ValueItem, Token![,]> directly.
                    // This requires more complex parsing for StatValue itself.

                    // Assuming expr_array.elems are Expr::Lit containing LitFloat or LitStr
                    // This is a simplification; proper handling requires trying to downcast Expr or re-parsing.
                    // Given the linter error context, the issue is likely type homogeneity in declarative macros.
                    // Proc macros fix this by generating distinct .add() calls.
                    add_calls.push(quote! {
                        let processed_path = bevy_gauge::prelude::Konfig::process_path(#path_str);
                        modifier_set.add(&processed_path, #elem_expr);
                    });
                }
            }
        }
    }
    add_calls
}

pub fn mod_set(input: TokenStream) -> TokenStream {
    let parsed_input = parse_macro_input!(input as MacroInput);
    let add_calls = generate_modifier_set_add_calls(&parsed_input.assignments);

    let expanded = quote! {{
        let mut modifier_set = bevy_gauge::prelude::ModifierSet::default();
        #(#add_calls)*
        modifier_set
    }};

    TokenStream::from(expanded)
}

pub fn stats(input: TokenStream) -> TokenStream {
    let parsed_input = parse_macro_input!(input as MacroInput);
    let add_calls = generate_modifier_set_add_calls(&parsed_input.assignments);

    let expanded = quote! {{
        // Ensure that you bring the required traits and types into scope.
        // Using $crate is not valid in proc macros for referring to the calling crate.
        // The paths must be absolute or correctly discoverable.
        use bevy_gauge::prelude::{StatsInitializer, ModifierSet};

        let mut modifier_set = ModifierSet::default();
        #(#add_calls)*
        StatsInitializer::new(modifier_set)
    }};

    TokenStream::from(expanded)
}

// ===== instant! macro =====

struct InstantAssignment {
    path: LitStr,
    op: InstantOp,
    value: StatValue,
}

enum InstantOp { Set, Add, Sub }

impl Parse for InstantOp {
    fn parse(input: ParseStream) -> SynResult<Self> {
        if input.peek(Token![+=]) {
            let _t: Token![+=] = input.parse()?;
            Ok(InstantOp::Add)
        } else if input.peek(Token![-=]) {
            let _t: Token![-=] = input.parse()?;
            Ok(InstantOp::Sub)
        } else if input.peek(Token![=]) {
            let _t: Token![=] = input.parse()?;
            Ok(InstantOp::Set)
        } else {
            Err(Error::new(input.span(), "expected one of '=', '+=', '-='"))
        }
    }
}

impl Parse for InstantAssignment {
    fn parse(input: ParseStream) -> SynResult<Self> {
        Ok(InstantAssignment {
            path: input.parse()?,
            op: input.parse()?,
            value: input.parse()?,
        })
    }
}

struct InstantInput {
    assignments: Punctuated<InstantAssignment, Token![,]>
}

impl Parse for InstantInput {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let assignments = Punctuated::<InstantAssignment, Token![,]>::parse_terminated(input)?;
        Ok(InstantInput { assignments })
    }
}

pub fn instant(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as InstantInput);

    let mut pushes = Vec::new();
    for item in parsed.assignments.iter() {
        let path = &item.path;
        match (&item.op, &item.value) {
            // Single value
            (InstantOp::Set, StatValue::Single(ValueItem::Literal(val))) => {
                pushes.push(quote! {
                    let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                    instant.add_set(&processed_path, #val);
                });
            }
            (InstantOp::Set, StatValue::Single(ValueItem::StrExpression(expr))) => {
                pushes.push(quote! {
                    let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                    instant.add_set(&processed_path, #expr);
                });
            }
            (InstantOp::Add, StatValue::Single(ValueItem::Literal(val))) => {
                pushes.push(quote! {
                    let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                    instant.add_add(&processed_path, #val);
                });
            }
            (InstantOp::Add, StatValue::Single(ValueItem::StrExpression(expr))) => {
                pushes.push(quote! {
                    let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                    instant.add_add(&processed_path, #expr);
                });
            }
            (InstantOp::Sub, StatValue::Single(ValueItem::Literal(val))) => {
                pushes.push(quote! {
                    let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                    instant.add_sub(&processed_path, #val);
                });
            }
            (InstantOp::Sub, StatValue::Single(ValueItem::StrExpression(expr))) => {
                pushes.push(quote! {
                    let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                    instant.add_sub(&processed_path, #expr);
                });
            }

            // Array values: emit one op per element
            (InstantOp::Set, StatValue::Array(expr_array)) => {
                for elem_expr in &expr_array.elems {
                    pushes.push(quote! {
                        let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                        instant.add_set(&processed_path, #elem_expr);
                    });
                }
            }
            (InstantOp::Add, StatValue::Array(expr_array)) => {
                for elem_expr in &expr_array.elems {
                    pushes.push(quote! {
                        let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                        instant.add_add(&processed_path, #elem_expr);
                    });
                }
            }
            (InstantOp::Sub, StatValue::Array(expr_array)) => {
                for elem_expr in &expr_array.elems {
                    pushes.push(quote! {
                        let processed_path = bevy_gauge::prelude::Konfig::process_path(#path);
                        instant.add_sub(&processed_path, #elem_expr);
                    });
                }
            }
        }
    }

    let expanded = quote! {{
        let mut instant = bevy_gauge::prelude::InstantModifierSet::default();
        #(#pushes)*
        instant
    }};

    TokenStream::from(expanded)
}