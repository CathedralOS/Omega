use source_files_to_tokens::Lexer;
use syntax_trees::{SyntaxTrees, expression::ExpressionNode, statement::StatementNode};

#[test]
fn field_omission_is_one_final_standalone_marker_and_survives_copy() {
    for (literal, omission, fields) in [
        ("Item { left: 7u32, right: 0u32 }", false, 2),
        ("Item { left: 7u32, .. }", true, 1),
        ("Item { .. }", true, 0),
        ("Item::Ready { left: 7u32, .., }", true, 1),
    ] {
        let text = format!("const VALUE: Item = {literal};");
        let tokens = Lexer::new(&text).tokenize().unwrap();
        let syntax = crate::parse_syntax_trees(&tokens).unwrap();
        let (handle, literal) = syntax
            .expressions
            .iter_expressions()
            .find_map(|(handle, node)| {
                if let ExpressionNode::StructLiteral(literal) = node {
                    Some((handle, literal))
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(literal.zero_omitted_fields, omission);
        assert_eq!(
            syntax.expressions.struct_fields(literal.fields).len(),
            fields
        );
        let mut copied = SyntaxTrees::default();
        let copied_handle = copied.copy_expression_from(&syntax, handle);
        let ExpressionNode::StructLiteral(copied_literal) =
            copied.expressions.expression(copied_handle)
        else {
            panic!("copied literal")
        };
        assert_eq!(copied_literal.zero_omitted_fields, omission);
    }
    for literal in [
        "Item { .. other }",
        "Item { .., left: 7u32 }",
        "Item { .., .. }",
        "Item { ... }",
        "Item { left: 7u32, .. other }",
    ] {
        let text = format!("const VALUE: Item = {literal};");
        let tokens = Lexer::new(&text).tokenize().unwrap();
        assert!(crate::parse_syntax_trees(&tokens).is_err(), "{literal}");
    }
}

#[test]
fn call_roots_retain_distinct_target_occurrences_through_syntax_copy() {
    let text = "const FIRST: u64 = outer(inner()); const SECOND: u64 = library::read(); const THIRD: u64 = receiver.read();";
    check_call_extents(
        text,
        &[
            ("outer(inner())", "outer", "outer"),
            ("inner()", "inner", "inner"),
            ("library::read()", "read", "read"),
            ("receiver.read()", "read", "read"),
        ],
    );
    check_call_extents(
        "const COMPUTED: u64 = (factory())(9);",
        &[
            ("(factory())(9)", "factory()", "$callable"),
            ("factory()", "factory", "factory"),
        ],
    );
}

fn check_call_extents(text: &str, expected: &[(&str, &str, &str)]) {
    let tokens = Lexer::new(text).tokenize().expect("tokens");
    let syntax = crate::parse_syntax_trees(&tokens).expect("syntax");
    let mut copied = SyntaxTrees::new(source::SourceId::default());
    copied.extend_from(&syntax);
    for program in [&syntax, &copied] {
        let mut pending = program
            .root_items()
            .filter_map(|item| match item {
                syntax_trees::item::Item::Const(definition) => Some(definition.value),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut spans = Vec::new();
        let mut target_spans = Vec::new();
        let mut calls = Vec::new();
        while let Some(expression) = pending.pop() {
            let ExpressionNode::Call(call) = program.expressions.expression(expression) else {
                panic!("call");
            };
            let span = program.expressions.source_span(expression);
            let target_span = call.target.source_span();
            let full = &text[span.span.start..span.span.end];
            let (_, target_text, target_name) = expected
                .iter()
                .find(|(expected, _, _)| *expected == full)
                .unwrap_or_else(|| panic!("unexpected call extent {full:?}"));
            assert_eq!(
                &text[target_span.span.start..target_span.span.end],
                *target_text
            );
            assert_eq!(call.target.as_str(program), *target_name);
            assert_eq!(span.source_id, target_span.source_id);
            assert!(
                span.span.start <= target_span.span.start && target_span.span.end <= span.span.end
            );
            assert!(
                !spans.contains(&span),
                "nested and sibling calls have distinct full occurrences"
            );
            assert!(
                !target_spans.contains(&target_span),
                "nested and sibling targets keep their distinct token occurrences"
            );
            spans.push(span);
            target_spans.push(target_span);
            calls.push(full);
            pending.extend(
                program
                    .expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .filter(|argument| {
                        matches!(
                            program.expressions.expression(*argument),
                            ExpressionNode::Call(_)
                        )
                    }),
            );
            if call.callee.is_valid()
                && matches!(
                    program.expressions.expression(call.callee),
                    ExpressionNode::Call(_)
                )
            {
                pending.push(call.callee);
            }
        }
        assert_eq!(spans.len(), expected.len());
        assert!(expected.iter().all(|(full, _, _)| calls.contains(full)));
    }
}

#[test]
fn static_and_value_call_targets_remain_distinct_after_syntax_copy() {
    let text = r#"
        machine example() {
            library::emit();
            receiver.emit();
            let first: u64 = library::read();
            let second: u64 = receiver.read();
        }
    "#;
    let tokens = Lexer::new(text).tokenize().expect("tokenize calls");
    let syntax = crate::parse_syntax_trees(&tokens).expect("parse calls");
    let mut copied = SyntaxTrees::new(source::SourceId::default());
    copied.extend_from(&syntax);
    for program in [&syntax, &copied] {
        let mut statements = Vec::new();
        let mut expressions = Vec::new();
        for item in program.root_items() {
            let syntax_trees::item::Item::Machine(machine) = item else {
                continue;
            };
            for state in program.items.state_handles(machine.states) {
                let state = program.items.state(*state);
                for statement in program.items.statements(state.statements) {
                    if let StatementNode::Call(call) = program.statements.statement(*statement) {
                        statements.push(call.target_is_static);
                    }
                    if let StatementNode::LocalData(local) =
                        program.statements.statement(*statement)
                    {
                        let ExpressionNode::Call(call) =
                            program.expressions.expression(local.initial_value)
                        else {
                            panic!("local initializer remains a call");
                        };
                        expressions.push(call.target_is_static);
                    }
                }
            }
        }
        assert_eq!(statements, [true, false]);
        assert_eq!(expressions, [true, false]);
    }
}

#[test]
fn structural_static_argument_probe_preserves_indexed_comparisons() {
    for comparison in ["left < values[0]", "left < values[0] > (right)"] {
        let source = format!(
            "machine compare(left: u64, right: u64, values: [u64; 1]) -> bool {{ {comparison} }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokens");
        let syntax = crate::parse_syntax_trees(&tokens).expect("ordinary comparison syntax");
        assert!(
            syntax.type_references.generic_nodes().is_empty(),
            "abandoned static argument probes publish no generic types"
        );
        assert!(!syntax.expressions.iter_expressions().any(|(_, expression)| matches!(expression, ExpressionNode::Call(call) if !call.machine_arguments.is_empty())));
    }
}

#[test]
fn applied_requirement_call_keeps_its_trait_application_as_static_receiver() {
    let text = "const READ: i32 = Parent<u64>::read(); const NOTED: i32 = Sink<u8, bool>::note<Handler>(value); const COMPARED: bool = left < right;";
    let tokens = Lexer::new(text).tokenize().expect("tokens");
    let syntax = crate::parse_syntax_trees(&tokens).expect("syntax");
    let values = syntax
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Const(definition) => Some(definition.value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let applied = |expression, trait_name: &str, arity: u32, member: &str, static_arguments| {
        let ExpressionNode::Call(call) = syntax.expressions.expression(expression) else {
            panic!("applied requirement call");
        };
        assert!(call.target_is_static);
        assert_eq!(call.target.as_str(&syntax), member);
        assert_eq!(call.machine_arguments.len(), static_arguments);
        let ExpressionNode::TypeExpression(application) =
            syntax.expressions.expression(call.receiver)
        else {
            panic!("the applied trait is the static receiver");
        };
        let syntax_trees::types::TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = syntax.type_references.type_reference(*application)
        else {
            panic!("generic trait application");
        };
        assert_eq!(base_name.as_str(&syntax), trait_name);
        assert_eq!(arguments.count(), arity);
    };
    applied(values[0], "Parent", 1, "read", 0);
    applied(values[1], "Sink", 2, "note", 1);
    assert!(matches!(
        syntax.expressions.expression(values[2]),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn boolean_type_operands_preserve_constructor_and_value_contexts() {
    let cases = [
        (
            "local",
            "machine choose<T>() -> bool { let selected: bool = T == Box<u64>; selected }",
        ),
        (
            "guard",
            "machine choose<T>() -> bool { transition { T == Box<u64> -> true _ -> false } }",
        ),
        (
            "constructor",
            "data Box<T> { value: T; } machine build() -> Box<u64> { (Box { value: 1u64 }) }",
        ),
        (
            "call",
            "machine choose(value: u64) -> u64 { read<u64>(value) }",
        ),
        (
            "comparison",
            "machine compare(left: u64, right: u64, values: [u64; 1]) -> bool { left < values[0] > (right) }",
        ),
    ];
    let mut equalities = 0;
    for (role, text) in cases {
        let tokens = Lexer::new(text).tokenize().expect("tokens");
        let syntax =
            crate::parse_syntax_trees(&tokens).unwrap_or_else(|error| panic!("{role}: {error:?}"));
        match role {
            "local" | "guard" => equalities += syntax.expressions.iter_expressions().filter(|(_, node)| matches!(node, ExpressionNode::Binary(binary)
                if binary.operator == syntax_trees::expression::BinaryOperator::Equal
                    && matches!(syntax.expressions.expression(binary.right), ExpressionNode::TypeExpression(_)))).count(),
            "constructor" => assert!(syntax.expressions.iter_expressions().any(|(_, node)| matches!(node, ExpressionNode::StructLiteral(_)))),
            "call" => assert!(syntax.expressions.iter_expressions().any(|(_, node)| matches!(node, ExpressionNode::Call(call) if call.machine_arguments.len() == 1))),
            "comparison" => assert!(syntax.type_references.generic_nodes().is_empty()),
            _ => unreachable!(),
        }
    }
    assert_eq!(
        equalities, 2,
        "local values and guards share static type syntax"
    );
}

#[test]
fn boolean_type_operands_do_not_turn_qualified_type_arguments_into_indices() {
    let text = "machine initialize<P, T>() -> AccessOutcome<Extent in Granted & Resident<P, T>, AccessReturn<Extent in Granted & Vacant, T>, Failure> {}";
    let tokens = Lexer::new(text).tokenize().expect("tokens");
    let syntax = crate::parse_syntax_trees(&tokens).expect("qualified type arguments");
    let machine = syntax
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let state = syntax
        .items
        .state(syntax.items.state_handles(machine.states)[0]);
    let syntax_trees::types::TypeReferenceNode::Generic { arguments, .. } =
        syntax.type_references.type_reference(state.return_type)
    else {
        panic!("applied result");
    };
    let arguments = syntax.type_references.type_reference_handles(*arguments);
    assert!(
        matches!(
            syntax.type_references.type_reference(arguments[0]),
            syntax_trees::types::TypeReferenceNode::Constrained { .. }
        ),
        "a constrained type retains its type role"
    );
    assert!(
        !syntax
            .type_references
            .const_expression_nodes()
            .iter()
            .any(|(_, expression)| matches!(
                syntax.expressions.expression(*expression),
                ExpressionNode::TypeExpression(_)
            )),
        "a type description supplies no integer/index value"
    );
}
