//! Closed generic constant calls retain their authored application for replay.

use super::{evaluate, evaluate_fully, integer_encoding};
use typed_trees::expression::ExpressionNode;

#[test]
fn generic_calls_in_executable_helpers_replay_after_ordinary_checking() {
    let typed = evaluate_fully(
        &[(
            "main.omg",
            "machine identity<T>(value: T) -> T { value }
         machine wrapper(value: u64) -> u64 { identity<u64>(value) }
         const VALUE: u64 = wrapper(7);",
        )],
        &[],
    );
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check executable generic helper call");
    super::super::validate_retained_invocations(&checked.typed, None)
        .expect("retained recipe can call an already-specialized executable helper");
}

#[test]
fn generic_calls_compose_type_const_and_nested_applications() {
    let syntax = evaluate(
        "machine identity<T>(value: T) -> T { value }
         machine forward<T>(value: T) -> T { identity<T>(value) }
         machine amount<const N: u64>() -> u64 { N }
         const VALUE: u64 = forward<u64>(identity<u64>(amount<7>())) + amount<9>();",
    )
    .expect("complete ordinary specializations compose");
    integer_encoding(&syntax, "VALUE", 16);
    for (carrier, value, expected) in [
        ("bool", "true", "boolean4:true"),
        ("f32", "-0.0f32", "float:f32:80000000"),
        ("f64", "1.5f64", "float:f64:3ff8000000000000"),
    ] {
        let text = format!(
            "machine identity<T>(value: T) -> T {{ value }}
             const VALUE: {carrier} = identity<{carrier}>({value});"
        );
        let typed = evaluate_fully(&[("main.omg", &text)], &[]);
        assert_eq!(
            typed.const_declarations()[0]
                .canonical_value_encoding
                .as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn generic_calls_reject_invalid_complete_tuples_and_false_premises() {
    let mut accepted = Vec::new();
    for source in [
        "machine identity<T>(value: T) -> T { value } const UNUSED: u64 = identity<bool>(7);",
        "machine identity<T>(value: T) -> T { value } const UNUSED: u64 = identity<u64, u8>(7);",
        "machine amount<const N: u8>() -> u64 { 7 } const UNUSED: u64 = amount<256>();",
        "machine guarded<const N: u64>(ready: bool) -> u64 requires ready; { N }
         const UNUSED: u64 = guarded<7>(false);",
        "machine amount<const N: u64>() -> u64 { N } const UNUSED: u64 = amount<7u8>();",
    ] {
        if evaluate(source).is_ok() {
            accepted.push(source);
        }
    }
    assert!(
        accepted.is_empty(),
        "invalid applications accepted: {accepted:?}"
    );
}

#[test]
fn generic_call_replay_rejects_alternate_instance_and_changed_argument() {
    let typed = evaluate_fully(
        &[(
            "main.omg",
            "machine amount<const N: u64>() -> u64 { N }
         const FIRST: u64 = amount<7>(); const SECOND: u64 = amount<9>();
         machine runtime() -> u64 { amount<9>() }",
        )],
        &[],
    );
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("ordinary checking preserves constant recipes");
    super::super::validate_retained_invocations(&checked.typed, None)
        .expect("receiving replay also works after ordinary specialization");
    let declaration = checked.typed.const_declarations()[0].clone();
    let ExpressionNode::Call(original) = checked
        .typed
        .expression_table
        .expression(declaration.authored_initializer)
    else {
        panic!("retained original call");
    };
    assert_eq!(
        original.machine_arguments.len(),
        1,
        "authored static tuple survives"
    );
    let instance = checked
        .typed
        .machine_specializations
        .iter()
        .find(|instance| instance.const_arguments == ["9"])
        .expect("runtime amount<9> specialization");
    let concrete = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == instance.instance)
        .unwrap();
    let alternate = checked.typed.machine_states(concrete)[0].symbol;
    let mut changed = checked.typed.clone();
    let ExpressionNode::Call(call) = changed
        .expression_table
        .expression_mut(declaration.authored_initializer)
    else {
        panic!("call");
    };
    call.target_symbol = alternate;
    call.machine_arguments = Box::default();
    assert!(
        super::super::validate_retained_invocations(&changed, None).is_err(),
        "another valid instance cannot replace the authored tuple"
    );

    let second = checked.typed.const_declarations()[1].authored_initializer;
    let ExpressionNode::Call(second_call) = checked.typed.expression_table.expression(second)
    else {
        panic!("second call");
    };
    let mut changed = checked.typed.clone();
    let ExpressionNode::Call(call) = changed
        .expression_table
        .expression_mut(declaration.authored_initializer)
    else {
        panic!("call");
    };
    call.machine_arguments = second_call.machine_arguments.clone();
    assert!(
        super::super::validate_retained_invocations(&changed, None).is_err(),
        "the exact invocation is reevaluated instead of trusting its old folded value"
    );
}
