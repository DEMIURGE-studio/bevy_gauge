use bevy_gauge::prelude::*;
use evalexpr::HashMapContext;

#[test]
fn expression_evaluate_defaults_missing_variables_to_zero() {
    // Given an expression that references an undefined variable
    let expr = Expression::new("a + 2").expect("expression should compile");

    // With an empty context (no variables set)
    let context = HashMapContext::new();

    // We expect evaluate to treat missing variables as 0 and compute 0 + 2 = 2
    // Currently, evalexpr errors on missing variables and our evaluate() maps any error to 0.0,
    // so this assertion is expected to FAIL until evaluate is updated to prefill missing vars.
    let result = expr.evaluate(&context);
    assert_eq!(result, 2.0);
}





