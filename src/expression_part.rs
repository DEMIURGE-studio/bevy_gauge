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
        let mut current_expr = String::new();
    
        for part in &self.parts {
            let trimmed = part.expr.trim();
    
            if trimmed.starts_with('+') {
                // e.g. "+ self.AddedLife"
                let operand = trimmed[1..].trim(); // everything after '+'
                if current_expr.is_empty() {
                    // First operator is '+': just use the operand
                    current_expr = operand.to_string();
                } else {
                    current_expr = format!("({} + {})", current_expr, operand);
                }
            } else if trimmed.starts_with('-') {
                // e.g. "- self.SomeValue"
                let operand = trimmed[1..].trim();
                if current_expr.is_empty() {
                    // First operator is '-': use (0 - operand)
                    current_expr = format!("(0 - {})", operand);
                } else {
                    current_expr = format!("({} - {})", current_expr, operand);
                }
            } else if trimmed.starts_with('*') {
                // e.g. "* self.Multiplier"
                let operand = trimmed[1..].trim();
                if current_expr.is_empty() {
                    // No more "0 * operand", just operand.
                    current_expr = operand.to_string();
                } else {
                    current_expr = format!("({} * {})", current_expr, operand);
                }
            } else if trimmed.starts_with('/') {
                // e.g. "/ self.Divisor"
                let operand = trimmed[1..].trim();
                if current_expr.is_empty() {
                    // No more "0 / operand", just operand.
                    current_expr = operand.to_string();
                } else {
                    // If the user typed "/ X + Y", we might enclose them: (current / (X + Y))
                    current_expr = format!("({} / ({}))", current_expr, operand);
                }
            } else if let Some(args) = trimmed.strip_prefix("clamp(").and_then(|s| s.strip_suffix(')')) {
                // e.g. "clamp(1, 10)" => clamp(current_expr, 1, 10)
                current_expr = format!("clamp({}, {})", current_expr, args);
            } else {
                panic!("Unknown or unexpected part format: `{}`", trimmed);
            }
        }
    
        // If no parts were added at all, default to 0
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

        let built = expr.build_expr_string();
        assert_eq!(built, "Total = clamp(((self.AddedLife + (self.TotalStrength / 5)) * self.IncreasedLife), 1, 100)");
    }
}