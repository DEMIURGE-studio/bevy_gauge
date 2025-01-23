use std::marker::PhantomData;

use evalexpr::{DefaultNumericTypes, Node};

pub trait StatConfig {
    /// Allows user defined sort criteria.
    fn get_idx(part: &str) -> usize;
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
        if part.starts_with("+") || part.starts_with("-") {
            0
        } else if part.starts_with("*") {
            1
        } else if part.starts_with("/") {
            2
        } else if part.starts_with("clamp") {
            3
        } else {
            panic!();
        }
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

impl<T: StatConfig> Expression<T> {
    pub fn new() -> Self {
        Expression {
            expr: evalexpr::build_operator_tree("0").unwrap(),
            parts: Vec::new(),
            _pd: PhantomData,
        }
    }

    pub fn add_part(&mut self, part: &str) {
        let value = T::get_idx(part);

        // If this part is already present, just increase stacks & return
        for p in &mut self.parts {
            if p.expr == part {
                p.stacks += 1;
                self.compile();
                return;
            }
        }

        // Insert in ascending order of precedence
        let mut idx = self.parts.len();
        for (i, p) in self.parts.iter().enumerate() {
            let list_value = T::get_idx(&p.expr);
            if list_value > value {
                idx = i;
                break;
            }
        }

        self.parts.insert(idx, ExpressionPart { stacks: 1, expr: part.to_string() });
        self.compile();
    }

    pub fn remove_part(&mut self, part: &str) {
        if let Some(pos) = self.parts.iter().position(|p| p.expr == part) {
            let p = &mut self.parts[pos];
            if p.stacks > 0 {
                p.stacks -= 1;
            }
            if p.stacks == 0 {
                self.parts.remove(pos);
            }
            self.compile();
        }
    }

    fn build_expr_string(&self) -> String {
        // If we have no parts, default to "Total = 0"
        if self.parts.is_empty() {
            return "Total = 0".to_string();
        }
    
        let mut current_expr = String::new();
    
        // We'll track the "active operator group" and a list of operands for that group.
        // E.g. if we see `+ A`, `+ B`, `+ C` in a row, we store them as ["A", "B", "C"] 
        // under the "plus" operator, then produce "(A + B + C)".
        let mut operator_group: Option<char> = None; // e.g. '+', '-', '*', '/'
        let mut operand_list: Vec<String> = Vec::new();
    
        // A helper function that emits the current group as a single subexpression.
        // e.g. if operator_group is '+', and operand_list is ["self.AddedLife","SomeThird","SomeForth"],
        // we produce "(self.AddedLife + SomeThird + SomeForth)".
        let mut flush_group = |cur_expr: &mut String,
                               op: Option<char>,
                               list: &mut Vec<String>| {
            if let Some(op_char) = op {
                if !list.is_empty() {
                    let joined = if list.len() == 1 {
                        // Only one operand => "operand"
                        list[0].clone()
                    } else {
                        // e.g. "(op1 + op2 + op3)"
                        let connector = format!(" {} ", op_char);
                        format!("({})", list.join(&connector))
                    };
                    if cur_expr.is_empty() {
                        *cur_expr = joined;
                    } else {
                        // e.g. existing => "(cur_expr ... )"
                        // new => "(cur_expr <op_char> joined)"
                        *cur_expr = format!("({} {} {})", *cur_expr, op_char, joined);
                    }
                    list.clear();
                }
            }
        };
    
        // We also handle "clamp(...)" specially: if we see a clamp, we flush any existing group
        // then wrap the clamp around the entire current expression. 
        // Or if the clamp is the *first* operator, we just store it as the entire expr.
    
        // A small function to handle "clamp(1,10) => clamp(cur_expr, 1,10)".
        let mut apply_clamp = |cur_expr: &mut String, clamp_args: &str| {
            if cur_expr.is_empty() {
                // If we have no prior expression, "clamp(0, 1, 10)" or something
                // but that's up to you whether that makes sense. 
                *cur_expr = format!("clamp(0, {})", clamp_args);
            } else {
                *cur_expr = format!("clamp({}, {})", *cur_expr, clamp_args);
            }
        };
    
        // Now we iterate over all parts in sorted order:
        for part in &self.parts {
            let trimmed = part.expr.trim();
    
            // we may have multiple stacks, so we do each "stack" occurrence in a loop
            for _ in 0..part.stacks {
                if let Some(args) = trimmed.strip_prefix("clamp(").and_then(|s| s.strip_suffix(')')) {
                    // If we see a clamp, we "flush" any plus or multiply group we had
                    flush_group(&mut current_expr, operator_group, &mut operand_list);
                    operator_group = None;
                    // Now wrap the entire expr in clamp
                    apply_clamp(&mut current_expr, args);
                } else if trimmed.starts_with('+') || trimmed.starts_with('-') {
                    // + or - => treat them as the same "add" group, but we can store 
                    // the sign in the operand string. e.g. if part is "- Something", store that as
                    // "(0 - Something)" if it's the first, or we unify them.
                    let operand = &trimmed[1..].trim();
                    // Distinguish operator?
                    // For simplicity, treat '+' and '-' as the same group => "add"
                    let group_char = '+';
    
                    // If we are in a different group from previous, flush previous
                    if operator_group != Some(group_char) {
                        flush_group(&mut current_expr, operator_group, &mut operand_list);
                        operator_group = Some(group_char);
                    }
                    if trimmed.starts_with('-') {
                        // If it's the first item => "0 - operand", else store a unary minus
                        if current_expr.is_empty() && operand_list.is_empty() {
                            operand_list.push(format!("(0 - {})", operand));
                        } else {
                            operand_list.push(format!("(0 - {})", operand));
                        }
                    } else {
                        operand_list.push(operand.to_string());
                    }
                } else if trimmed.starts_with('*') {
                    // multiply group
                    let group_char = '*';
                    let operand = &trimmed[1..].trim();
    
                    if operator_group != Some(group_char) {
                        flush_group(&mut current_expr, operator_group, &mut operand_list);
                        operator_group = Some(group_char);
                    }
                    operand_list.push(operand.to_string());
                } else if trimmed.starts_with('/') {
                    // division => treat it as its own group or part?
                    let group_char = '/';
                    let operand = &trimmed[1..].trim();
    
                    if operator_group != Some(group_char) {
                        flush_group(&mut current_expr, operator_group, &mut operand_list);
                        operator_group = Some(group_char);
                    }
                    // The next operand might be multiple terms, e.g. "/ (A + B)"
                    operand_list.push(format!("({})", operand));
                } else {
                    panic!("Unknown or unexpected part format: `{}`", trimmed);
                }
            }
        }
    
        // End of loop: flush any leftover group
        flush_group(&mut current_expr, operator_group, &mut operand_list);
    
        if current_expr.is_empty() {
            "Total = 0".to_string()
        } else {
            format!("Total = {}", current_expr)
        }
    }
    
    pub fn compile(&mut self) {
        let expr = self.build_expr_string();

        self.expr = evalexpr::build_operator_tree(&expr).unwrap();
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_add() {
        // Create an Expression that uses DefaultStatConfig
        let mut expr = Expression::<DefaultStatConfig>::new();

        // Add a part: "+ self.AddedLife"
        expr.add_part("+ self.AddedLife");

        // Check that the built expression string is as expected
        let built = expr.build_expr_string();
        assert_eq!(built, "Total = self.AddedLife");

        // Also, you could check the compiled node by evaluating it, 
        // but you'd need to define variables in evalexpr context, etc.
        // For instance:
        //
        // use evalexpr::*;
        // let mut context = HashMapContext::new();
        // context.set_value("self.AddedLife".into(), 100.into()).unwrap();
        // let eval_result = expr.expr.eval_with_context(&context);
        // assert_eq!(eval_result.unwrap(), Value::from(100));
    }

    #[test]
    fn test_plus_then_multiply() {
        let mut expr = Expression::<DefaultStatConfig>::new();

        expr.add_part("* self.Multiplier");
        expr.add_part("+ self.Base");

        let built = expr.build_expr_string();
        // Because of our code, we expect something like:
        // "Total = (self.Base * self.Multiplier)"
        // (since the first operator is '+', we get "self.Base", 
        //  then the second operator is '*', we get "(self.Base * self.Multiplier)")
        assert_eq!(built, "Total = (self.Base * self.Multiplier)");
    }

    #[test]
    fn test_clamp() {
        let mut expr = Expression::<DefaultStatConfig>::new();

        expr.add_part("clamp(1, 10)");
        expr.add_part("+ Added");

        let built = expr.build_expr_string();
        // Because we have no prior +/-, the code replaces current_expr with clamp(0, 1, 10)
        assert_eq!(built, "Total = clamp(Added, 1, 10)");
    }

    #[test]
    fn test_complex_case() {
        let mut expr = Expression::<DefaultStatConfig>::new();

        expr.add_part("+ self.AddedLife");
        expr.add_part("+ (self.TotalStrength / 5)");
        expr.add_part("clamp(1, 100)");
        expr.add_part("* self.IncreasedLife");
        expr.add_part("+ SomeThirdModifier");
        expr.add_part("+ SomeForthModifier");

        let built = expr.build_expr_string();
        assert_eq!(built, "Total = clamp(((self.AddedLife + (self.TotalStrength / 5) + SomeThirdModifier + SomeForthModifier) * self.IncreasedLife), 1, 100)");
    }
}