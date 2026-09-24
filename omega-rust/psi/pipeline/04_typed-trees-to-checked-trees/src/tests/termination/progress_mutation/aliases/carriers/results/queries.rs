use super::super::queries::{assert_origin, typed_fixture};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;

#[test]
fn a_shared_input_move_transports_the_actual_selected_case() {
    for (initializer, expected) in [
        ("Choice::Selected { context: &context }", Some("context")),
        ("Choice::Empty {}", None),
    ] {
        let program = typed_fixture(
            &format!(
                "let original: Choice = {initializer};
                      let carrier: Choice = identity(original);
                      let borrowed: &Context = carrier.context;"
            ),
            "data Choice { case Selected(context: &Context); case Empty; }
             machine identity(choice: Choice) -> Choice { choice }",
        );
        assert_origin(&program, expected);
    }
}

#[test]
fn a_caller_case_does_not_prove_an_unchecked_helper_payload_projection() {
    let program = typed_fixture(
        "let original: Choice = Choice::Selected { context: &context };
         let carrier: Carrier = unwrap(original);
         let borrowed: &Context = carrier.context;",
        "data Choice { case Selected(context: &Context); case Empty; }
         data Carrier { context: &Context; }
         machine unwrap(choice: Choice) -> Carrier { Carrier { context: choice.context } }",
    );
    assert_origin(&program, None);
}

#[test]
fn a_reconstructed_shared_result_selects_the_exact_input_field() {
    let program = typed_fixture(
        "let original: Pair = Pair { first: &context, second: &replacement };
         let carrier: Carrier = select(original);
         let borrowed: &Context = carrier.context;",
        "data Pair { first: &Context; second: &Context; }
         data Carrier { context: &Context; }
         machine select(pair: Pair) -> Carrier { Carrier { context: pair.second } }",
    );
    assert_origin(&program, Some("replacement"));
}

#[test]
fn a_shared_aggregate_result_requires_a_resolved_helper_identity() {
    let mut program = typed_fixture(
        "let carrier: Carrier = wrap(context); let borrowed: &Context = carrier.context;",
        "data Carrier { context: &Context; }
         machine wrap(context: &Context) -> Carrier { Carrier { context: context } }",
    );
    assert_origin(&program, Some("context"));
    let calls = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "wrap")
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(!calls.is_empty());
    for expression in calls {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            unreachable!()
        };
        call.target_symbol = SymbolHandle::invalid();
    }
    assert_origin(&program, None);
}

#[test]
fn a_shared_aggregate_result_cannot_excuse_caller_binding_exposure() {
    for exposed in ["context", "borrowed"] {
        let program = typed_fixture(
            &format!("let mut borrowed: &Context = &context;
                      let carrier: Carrier = forward(replacement, &mut {exposed});
                      let result: &Context = carrier.context;"),
            "data Carrier<'selected> { context: &'selected Context; }
             machine forward<'selected, 'binding>(selected: &'selected Context, binding: &'binding mut Context) -> Carrier<'selected> {
                 Carrier { context: selected }
             }",
        );
        assert_origin(&program, None);
    }
}
