//! Integer landing destination tests.

use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StatementNode, TransitionTargetNode,
    TypedTrees, anonymous_integer_landing_warnings, append_destination_literals,
    call_argument_destinations,
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
fn proof_integer_peer_landing_warns_on_exact_fractional_intermediates() {
    for arithmetic in ["embed(7i32) / (1 / 2 * 4)", "(1 / 2 * 4) % embed(7i32)"] {
        let program = typed(&format!("machine value() ensures {arithmetic} == 0 {{}}"));
        let warnings = anonymous_integer_landing_warnings(&program);
        assert_eq!(warnings.len(), 1, "{arithmetic}: {warnings:?}");
        assert!(warnings[0].message.contains("`1/2`"));
        assert!(warnings[0].message.contains("integer `2`"));
    }
    let anonymous = typed("machine value() ensures (1 / 2 * 4) == 2 {}");
    assert!(anonymous_integer_landing_warnings(&anonymous).is_empty());
}

#[test]
fn fractional_landing_warning_retains_exact_value_and_original_span() {
    let program = typed("machine value() -> u32 { (4097 / 4096) * 4096 }");
    let warnings = anonymous_integer_landing_warnings(&program);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].message.contains("4097/4096"));
    assert!(warnings[0].message.contains("integer `4097`"));
    assert!(warnings[0].message.contains("type an operand"));
    let origin = program.expression_table.iter_expressions().find_map(|(handle, node)| {
        matches!(node, ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Divide).then_some(handle)
    }).unwrap();
    assert_eq!(
        warnings[0].source_span,
        Some(program.expression_table.source_span(origin))
    );
}

#[test]
fn constructed_scalar_fields_report_their_fractional_origins() {
    for (definition, constructor) in [
        ("data Number { value: i32; }", "Number"),
        ("data Number { case Value(value: i32); }", "Number::Value"),
    ] {
        let source = format!(
            "{definition} machine value() {{ let number: Number = {constructor} {{ value: 0.1 * 70 }}; }}"
        );
        let program = typed(&source);
        let warnings = anonymous_integer_landing_warnings(&program);
        let [warning] = warnings.as_slice() else {
            panic!("one constructed field warning: {warnings:?}");
        };
        assert!(warning.message.contains("fractional intermediate `1/10`"));
        assert!(warning.message.contains("integer `7`"));
        let offset = source.find("0.1").expect("fractional field source");
        assert_eq!(
            warning.source_span.expect("authored origin").span,
            source::Span::new(offset, offset + 3)
        );
    }
}

#[test]
fn decimal_fraction_warning_retains_the_first_exact_source_value() {
    let program = typed("machine value() -> i32 { 0.1 + 0.9 }");
    let warnings = anonymous_integer_landing_warnings(&program);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].message.contains("1/10"));
    assert!(warnings[0].message.contains("integer `1`"));
    let origin = program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Float(literal) if literal.text() == "0.1")
                .then_some(handle)
        })
        .expect("first decimal leaf");
    assert_eq!(
        warnings[0].source_span,
        Some(program.expression_table.source_span(origin))
    );
}

#[test]
fn fractional_landing_warnings_share_local_assignment_and_return_destinations() {
    for source_text in [
        "machine value() { let result: i32 = 7 / 2 * 2; }",
        "data Main { result: i32; } machine Main::value(&mut self) { self.result = 7 / 2 * 2; }",
        "machine value() -> i32 { 7 / 2 * 2 }",
        "machine value() -> i32 { transition true { true -> 7 / 2 * 2 false -> 0 } }",
    ] {
        let warnings = anonymous_integer_landing_warnings(&typed(source_text));
        assert_eq!(warnings.len(), 1, "{source_text}: {warnings:?}");
        assert!(warnings[0].message.contains("7/2"));
        assert!(warnings[0].message.contains("integer `7`"));
    }
}

#[test]
fn fractional_landing_warnings_exclude_unlanded_typed_and_float_results() {
    for (result_type, expression) in [
        ("i32", "8 / 2"),
        ("i32", "7 / 2"),
        ("i32", "7i32 / 2 * 2"),
        ("u8", "513 / 2 * 2"),
        ("f32", "7 / 2 * 2"),
    ] {
        let source_text = format!("machine value() -> {result_type} {{ {expression} }}");
        assert!(
            anonymous_integer_landing_warnings(&typed(&source_text)).is_empty(),
            "{source_text}"
        );
    }
}

#[test]
fn fractional_landing_warnings_include_casts_and_actual_integer_peers() {
    for source_text in [
        "machine value() -> i32 { (7 / 2 * 2) as i32 }",
        "machine value(input: i32 [0..=1]) -> i32 { input * (7 / 2 * 2) }",
        "machine value(input: i32 [0..=1]) -> i32 { (7 / 2 * 2) * input }",
        "machine value() -> i32 { 1i32 * (7 / 2 * 2) }",
        "machine sample() -> i32 { 1 } machine value() -> i32 { sample() * (7 / 2 * 2) }",
    ] {
        let warnings = anonymous_integer_landing_warnings(&typed(source_text));
        assert_eq!(warnings.len(), 1, "{source_text}: {warnings:?}");
        assert!(warnings[0].message.contains("7/2"));
        assert!(warnings[0].message.contains("integer `7`"));
    }
    for source_text in [
        "machine value(input: i32) -> i32 { input * (7i32 / 2 * 2) }",
        "machine value(input: f64) -> f64 { input * (7 / 2 * 2) }",
        "machine value() -> i32 { (7i32 / 2 * 2) as i32 }",
    ] {
        assert!(
            anonymous_integer_landing_warnings(&typed(source_text)).is_empty(),
            "{source_text}"
        );
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

#[test]
fn constructed_field_width_grants_do_not_escape_to_other_fields() {
    let mut program = typed(&format!(
        "data Pair {{ integer: i32; floating: f64; }}
             machine construct() {{ let pair: Pair = Pair {{ integer: {LARGE_ARGUMENT}, floating: 0.0 }}; }}"
    ));
    assert_eq!(width_grants(&program).len(), 2);
    let (expression, mut literal) = program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| match node {
            ExpressionNode::StructLiteral(literal) => Some((handle, literal.clone())),
            _ => None,
        })
        .expect("record constructor");
    let mut fields = program
        .expression_table
        .struct_fields(literal.fields)
        .to_vec();
    fields[1].value = fields[0].value;
    literal.fields = program.expression_table.insert_struct_fields(fields);
    *program.expression_table.expression_mut(expression) = ExpressionNode::StructLiteral(literal);
    assert!(
        width_grants(&program).is_empty(),
        "a floating-field edge retains its own width obligation"
    );
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
fn exact_call_argument_width_custody_is_independent_of_destination_policy() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        for body in [
            format!("take({LARGE_ARGUMENT});"),
            format!("let result: i32{policy} = take({LARGE_ARGUMENT});"),
            format!("take(take({LARGE_ARGUMENT}));"),
        ] {
            let source_text = format!(
                "machine take(value: i32{policy}) -> i32{policy} {{ value }} machine run() {{ {body} }}"
            );
            assert_eq!(width_grants(&typed(&source_text)).len(), 2, "{source_text}");
        }
        let source_text = format!("machine run() -> i32{policy} {{ {LARGE_ARGUMENT} }}");
        assert_eq!(width_grants(&typed(&source_text)).len(), 2, "{source_text}");
    }
}

#[test]
fn mutable_owned_scalar_arguments_keep_their_initial_landing_destination() {
    for argument in ["7 / 2 * 2", LARGE_ARGUMENT] {
        for body in [
            format!("take({argument});"),
            format!("let saved: i32 = take({argument});"),
        ] {
            let source_text = format!(
                "machine take(mut value: i32) -> i32 {{ value }} machine run() {{ {body} }}"
            );
            let program = typed(&source_text);
            assert_eq!(
                anonymous_integer_landing_warnings(&program).len(),
                usize::from(argument != LARGE_ARGUMENT),
                "{source_text}"
            );
            assert_eq!(
                width_grants(&program).len(),
                if argument == LARGE_ARGUMENT { 2 } else { 0 },
                "{source_text}"
            );
        }
    }
    for argument in [
        "513 / 2 * 2",
        "18446744073709551616",
        "18446744073709551617 / 2",
    ] {
        let source_text =
            format!("machine take(mut value: u8) {{}} machine run() {{ take({argument}); }}");
        let program = typed(&source_text);
        assert!(
            anonymous_integer_landing_warnings(&program).is_empty(),
            "{source_text}"
        );
        assert!(width_grants(&program).is_empty(), "{source_text}");
    }
    // A mutable reference still is not an owned integer destination.
    let source_text =
        format!("machine take(value: &mut i32) {{}} machine run() {{ take({LARGE_ARGUMENT}); }}");
    let program = typed(&source_text);
    assert!(anonymous_integer_landing_warnings(&program).is_empty());
    assert!(width_grants(&program).is_empty());
}

#[test]
fn named_transition_arguments_share_exact_destination_width_and_warning_queries() {
    for argument in ["7 / 2 * 2", LARGE_ARGUMENT] {
        let source_text = format!(
            "machine run() -> i32 {{ transition {{ _ -> finish({argument}) }} state finish(value: i32) -> i32 {{ value }} }}"
        );
        let program = typed(&source_text);
        assert_eq!(
            anonymous_integer_landing_warnings(&program).len(),
            usize::from(argument != LARGE_ARGUMENT)
        );
        assert_eq!(
            width_grants(&program).len(),
            if argument == LARGE_ARGUMENT { 2 } else { 0 }
        );
        let (statement, transition) = program
            .statement_table
            .iter_statements(program.machine_states(&program.machines()[0])[0].statement_nodes)
            .find_map(|(handle, statement)| {
                if let StatementNode::Transition(transition) = statement {
                    Some((handle, *transition))
                } else {
                    None
                }
            })
            .expect("fixture has a named transition");
        let target = program
            .statement_table
            .transition_target(transition.target)
            .clone();
        let TransitionTargetNode::Named { path, .. } = &target else {
            panic!("expected named target")
        };
        for wrong in [
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::from_parts(
                path.symbol.arena_index(),
                path.symbol.generation() + 1,
            ),
            program.machines()[0].symbol,
        ] {
            let mut invalid = program.clone();
            let mut target = target.clone();
            let TransitionTargetNode::Named { path, .. } = &mut target else {
                unreachable!()
            };
            path.symbol = wrong;
            let target = invalid.statement_table.insert_transition_target(target);
            let StatementNode::Transition(transition) =
                invalid.statement_table.statement_mut(statement)
            else {
                unreachable!()
            };
            transition.target = target;
            assert!(anonymous_integer_landing_warnings(&invalid).is_empty());
            assert!(width_grants(&invalid).is_empty());
        }
    }
}

#[test]
fn call_destinations_reject_missing_ambiguous_generic_and_wrong_arity_targets() {
    let source_text = format!(
        "machine take(value: i32) -> i32 {{ value }} machine run() -> i32 {{ take({LARGE_ARGUMENT}) }}"
    );
    let program = typed(&source_text);
    let expression = first_expression_call(&program);
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        unreachable!()
    };
    let target = call.target_symbol;
    assert!(call_argument_destinations(&program, target, 1).is_some());
    assert!(call_argument_destinations(&program, target, 0).is_none());
    assert!(call_argument_destinations(&program, symbols::SymbolHandle::invalid(), 1).is_none());
    assert!(
        call_argument_destinations(
            &program,
            symbols::SymbolHandle::from_parts(target.arena_index(), target.generation() + 1),
            1
        )
        .is_none()
    );
    let mut unresolved = program.clone();
    let ExpressionNode::Call(call) = unresolved.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    call.target_symbol = symbols::SymbolHandle::invalid();
    assert!(width_grants(&unresolved).is_empty());
    let (machine, _) =
        crate::machine_calls::calls::machine_state_by_symbol(&program, target).unwrap();
    let owner = machine.symbol;
    let mut ambiguous = program.clone();
    ambiguous.push_machine(machine.clone());
    assert!(width_grants(&ambiguous).is_empty());
    let mut generic = program.clone();
    generic
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == owner)
        .unwrap()
        .lifetime_parameters
        .push(typed_trees::name::Identifier::default());
    assert!(width_grants(&generic).is_empty());
}

#[test]
fn call_width_grants_do_not_escape_to_other_argument_or_receiver_edges() {
    let source_text = format!(
        "machine take(value: i32) -> i32 {{ value }} machine other(value: f64) -> i32 {{ 0 }} machine run() -> i32 {{ let saved: i32 = take({LARGE_ARGUMENT}); other(0.0f64) }}"
    );
    let program = typed(&source_text);
    let calls: Vec<_> = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
        .collect();
    assert_eq!(calls.len(), 2);
    let argument = first_argument(&program, calls[0]);
    assert_eq!(width_grants(&program).len(), 2);
    let mut shared = program.clone();
    let arguments = shared
        .expression_table
        .insert_expression_handles([argument]);
    let ExpressionNode::Call(call) = shared.expression_table.expression_mut(calls[1]) else {
        unreachable!()
    };
    call.arguments = arguments;
    assert!(width_grants(&shared).is_empty());
    let mut receiver = program.clone();
    let ExpressionNode::Call(call) = receiver.expression_table.expression_mut(calls[0]) else {
        unreachable!()
    };
    call.receiver = argument;
    assert!(width_grants(&receiver).is_empty());
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
