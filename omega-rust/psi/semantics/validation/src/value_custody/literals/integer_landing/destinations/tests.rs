//! Integer landing destination tests.

use super::{
    ExpressionHandle, ExpressionNode, TypedTrees, anonymous_integer_landing_warnings,
    append_destination_literals,
};

mod arrays;
mod match_results;
mod operand_edges;
mod windows;

fn typed(source_text: &str) -> TypedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("anonymous_landing.omg".into(), source_text.to_owned())
        .source_id;
    crate::front_end::typed_program_from_source_map(sources, &[(source_id, source_text)])
}

#[test]
fn integer_range_endpoints_retain_fractional_landing_warnings() {
    for endpoint in ["(5 / 2) * 2", "1u64 * ((5 / 2) * 2)"] {
        let program = typed(&format!("machine value(input: u64[{endpoint}..=10]) {{}}"));
        let warnings = anonymous_integer_landing_warnings(&program);
        assert_eq!(warnings.len(), 1, "{endpoint}: {warnings:?}");
        assert!(warnings[0].message.contains("`5/2`"));
        assert!(warnings[0].message.contains("integer `5`"));
    }
    for (carrier, endpoint) in [("u64", "5 / 2"), ("u64", "6 / 2"), ("f64", "5 / 2 * 2")] {
        let program = typed(&format!(
            "machine value(input: {carrier}[{endpoint}..=10]) {{}}"
        ));
        assert!(anonymous_integer_landing_warnings(&program).is_empty());
    }
}

#[test]
fn call_argument_warnings_use_exact_parameters_and_skip_self() {
    for source_text in [
        "machine take(value: i32) {} machine run() { take(7 / 2 * 2); }",
        "machine take(value: i32) -> i32 { value } machine run() -> i32 { take(7 / 2 * 2) }",
        "machine take(value: i32) -> i32 { value } machine run() -> i32 { take(take(7 / 2 * 2)) }",
        "data Main {} machine Main::take(&self, value: i32) {} machine Main::run(&self) { self.take(7 / 2 * 2); }",
        "data Main {} machine Main::take(&self, value: i32) -> i32 { value } machine Main::run(&self) -> i32 { self.take(7 / 2 * 2) }",
    ] {
        let warnings = anonymous_integer_landing_warnings(&typed(source_text));
        assert_eq!(warnings.len(), 1, "{source_text}: {warnings:?}");
        assert!(warnings[0].message.contains("7/2"));
        assert!(warnings[0].message.contains("integer `7`"));
    }
    for (parameter_type, argument) in [
        ("i32", "7i32 / 2 * 2"),
        ("i32", "7 / 2"),
        ("u8", "513 / 2 * 2"),
        ("f64", "7 / 2 * 2"),
    ] {
        let source_text = format!(
            "machine take(value: {parameter_type}) {{}} machine run() {{ take({argument}); }}"
        );
        assert!(
            anonymous_integer_landing_warnings(&typed(&source_text)).is_empty(),
            "{source_text}"
        );
    }
}

const LARGE_ARGUMENT: &str = "18446744073709551616 / 18446744073709551616";

fn width_grants(program: &TypedTrees) -> Vec<ExpressionHandle> {
    let mut granted = Vec::new();
    append_destination_literals(program, &mut granted);
    granted
}

fn first_expression_call(program: &TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
        .expect("fixture has an expression call")
}

fn first_argument(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        panic!("expected call")
    };
    program.expression_table.expression_handles(call.arguments)[0]
}

#[test]
fn call_width_walk_rejects_stale_and_cyclic_argument_trees() {
    let source_text = format!(
        "machine take(value: i32) -> i32 {{ value }} machine run() -> i32 {{ take({LARGE_ARGUMENT}) }}"
    );
    let program = typed(&source_text);
    let call = first_expression_call(&program);
    let argument = first_argument(&program, call);
    let mut cyclic = program.clone();
    let ExpressionNode::Binary(binary) = cyclic.expression_table.expression_mut(argument) else {
        panic!("expected quotient")
    };
    binary.left = argument;
    assert!(width_grants(&cyclic).is_empty());
    let mut stale = program.clone();
    let arguments =
        stale
            .expression_table
            .insert_expression_handles([ExpressionHandle::from_parts(
                argument.arena_index(),
                argument.generation() + 1,
            )]);
    let ExpressionNode::Call(call) = stale.expression_table.expression_mut(call) else {
        unreachable!()
    };
    call.arguments = arguments;
    assert!(width_grants(&stale).is_empty());
}
