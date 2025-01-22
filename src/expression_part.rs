use std::marker::PhantomData;

use evalexpr::{DefaultNumericTypes, Node};

pub trait ExpressionBuilder {
    fn build(&self) -> Node<DefaultNumericTypes>;
}

pub struct DefaultExpressionBuilder {

}

pub trait StatConfig {
    /// Allows user defined sort criteria.
    fn get_idx(part: &str) -> usize;

    /// Informs how an expression string is mutated by an expression part string.
    fn mutate(expr_str: &mut String, expr_part: &str);
}

pub struct DefaultStatConfig;

impl StatConfig for DefaultStatConfig {
    fn get_idx(part: &str) -> usize {
        // Parse the first symbol in the string. Look for
        // +, -, *, /, or clamp
        
        // Split part into parts at " "
        // Match against the first element in the split. Example:
        // match split[0] {
        //  "+" => { ... }
        //  ... 
        //  "clamp" => { ... }
        // }
        todo!()
    }
    
    fn mutate(expr_str: &mut String, expr_part: &str) {
        // Combine expr parts with the expression
        // Allows user defined rules
        // For instance, take "Total = (B) * (C + D)"
        // If we add "+ A", and we know that "+" goes in our first block
        // we would output "Total = (A + B) * (C + D)"
        // Applying a "clamp(E, F)" rule would look like so:
        // "Total = clamp((A + B) * (C + D), E, F)"
        // and say we also apply a "clamp(G, H)", we can wrap again:
        // "Total = clamp(clamp((A + B) * (C + D), E, F), G, H)"

        // Do a similar split and match as above. If it's a +, find the first
        // top level open paranthesis and throw it in there. If it's *,
        // it should find the second top-level open parenthesis and throw
        todo!()
    }
}

pub struct ExpressionPart {
    pub stacks: usize,
    pub expr: String,
}

pub struct Expression<T: StatConfig> {
    pub expr: Node<DefaultNumericTypes>,
    pub parts: Vec<ExpressionPart>,
    _pd: PhantomData<T>,
}

/// It would be nice to be able to configure a StatConfig without having
/// to supply one every time. 
impl<T: StatConfig> Expression<T> {
    pub fn add_part(&mut self, part: &str) {
        let value = T::get_idx(part);

        // Get the idx of each part. Use this to sort the new part into the
        // expression. Make sure that the expression part is not already present.
        // If it is, increase the stacks and move on.
        for p in self.parts.iter_mut() {
            if p.expr == part {
                p.stacks += 1;
                return;
            }
        }

        self.compile();
    }

    pub fn remove_part(&mut self, part: &str) {
        for p in self.parts.iter_mut() {
            if p.expr == part {
                p.stacks -= 1;
            }

            if p.stacks == 0 {
                // remove me
            }
        }
        self.compile();
    }

    fn build_expr_string(&self) -> String {
        let mut expr = "Total = ()".to_string();

        for part in self.parts.iter() {
            T::mutate(&mut expr, &part.expr);
        }

        return expr;
    }

    pub fn compile(&mut self) {
        let expr = self.build_expr_string();

        self.expr = evalexpr::build_operator_tree(&expr).unwrap();
    }
}