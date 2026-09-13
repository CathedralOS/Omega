use super::*;

fn program(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("symbols");
    let mut program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("types");
    // Isolate execution from synthesis: preserve resolved membership operands,
    // then exercise the explicit typed operation directly.
    let memberships = program.expression_table.expression_entries().filter_map(|(handle, node)| {
        let ExpressionNode::Binary(binary) = node else { return None; };
        (binary.operator == BinaryOperator::Equal
            && matches!(program.expression_table.expression(binary.right), ExpressionNode::Name(path)
                if path.head_symbol != path.symbol))
            .then_some(handle)
    }).collect::<Vec<_>>();
    assert!(!memberships.is_empty());
    for handle in memberships {
        let ExpressionNode::Binary(binary) = program.expression_table.expression_mut(handle) else {
            panic!("membership");
        };
        binary.operator = BinaryOperator::CaseMembership;
    }
    program
}

fn classifier(program: &TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| match node {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::CaseMembership => {
                Some(binary.right)
            }
            _ => None,
        })
        .expect("classifier")
}

#[test]
fn case_membership_observes_nominal_tags_without_payload_comparison() {
    for (case, expected) in [("Data { value: 9 }", 7), ("Empty", 0)] {
        let program = program(&format!(
            "data Message [copy] {{ case Data(value: i32); case Empty; }}
             machine main() -> i32 {{ let value: Message = Message::{case};
                 transition value in Message::Data {{ true -> 7 false -> 0 }} }}"
        ));
        assert!(
            matches!(Evaluator::new(&program, &[]).run_const_machine("main"), Ok(value) if value == expected)
        );
    }
}

#[test]
fn case_membership_preserves_short_circuiting_and_one_subject_evaluation() {
    let program = program(
        "data Message [copy] { case Data(value: i32); case Empty; }
         machine read(calls: &mut i32 in Wrapping) -> Message { calls = calls + 1; Message::Data { value: 9 } }
         machine main() -> i32 { let mut calls: i32 in Wrapping = 0;
             let matched: bool = read(&mut calls) in Message::Data;
             let skipped: bool = false && (read(&mut calls) in Message::Data);
             transition matched && !skipped && calls == 1 { true -> 7 false -> 0 } }"
    );
    assert!(matches!(
        Evaluator::new(&program, &[]).run_const_machine("main"),
        Ok(7)
    ));
}

#[test]
fn case_membership_rejects_invalid_classifier_and_foreign_runtime_owner() {
    let original = program(
        "data Message [copy] { case Data(value: i32); case Empty; }
         data Foreign [copy] { case Data(value: i32); case Empty; }
         machine main() -> i32 { let value: Message = Message::Data { value: 9 };
             transition value in Message::Data { true -> 7 false -> 0 } }",
    );
    let classifier = classifier(&original);
    let mut invalid = original.clone();
    *invalid.expression_table.expression_mut(classifier) = ExpressionNode::Boolean(true);
    assert!(
        matches!(Evaluator::new(&invalid, &[]).run_const_machine("main"), Err(Halt::Trap(message)) if message.contains("exact case classifier"))
    );

    let foreign = original
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Foreign")
        .expect("foreign")
        .symbol;
    let literal = original
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::StructLiteral(_)).then_some(handle)
        })
        .expect("constructor");
    let mut invalid = original.clone();
    let ExpressionNode::StructLiteral(literal) = invalid.expression_table.expression_mut(literal)
    else {
        panic!("constructor");
    };
    literal.type_symbol = foreign;
    literal.type_name = typed_trees::name::Identifier::generated("Foreign");
    literal.case_symbol = original
        .data_definitions()
        .iter()
        .find(|data| data.symbol == foreign)
        .and_then(|data| {
            original
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == "Data" =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
        });
    assert!(
        matches!(Evaluator::new(&invalid, &[]).run_const_machine("main"), Err(Halt::Trap(message)) if message.contains("different nominal owner"))
    );

    let mut invalid = original;
    let ExpressionNode::Name(path) = invalid.expression_table.expression_mut(classifier) else {
        panic!("classifier");
    };
    path.head_symbol = foreign;
    assert!(
        matches!(Evaluator::new(&invalid, &[]).run_const_machine("main"), Err(Halt::Trap(message)) if message.contains("exact case classifier"))
    );
}

#[test]
fn case_membership_executes_checked_nested_structural_equality_synthesis() {
    for (right_case, right_active, expected) in [
        ("Data { value: 9, checksum: 3 }", "true", 7),
        ("Data { value: 8, checksum: 3 }", "true", 0),
        ("Empty", "true", 0),
        ("Data { value: 9, checksum: 3 }", "false", 0),
    ] {
        let source = format!(
            "trait Equatable {{ machine equals(&self, rhs: &Self) -> bool; }}
             data Message {{ case Empty; case Data(value: i32, checksum: i32); }}
             MessageEquatable: Message satisfies Equatable;
             data Envelope {{ active: bool; message: Message; }}
             EnvelopeEquatable: Envelope satisfies Equatable;
             machine main() -> i32 {{
                 let left: Envelope = Envelope {{ active: true, message: Message::Data {{ value: 9, checksum: 3 }} }};
                 let right: Envelope = Envelope {{ active: {right_active}, message: Message::{right_case} }};
                 transition left == right {{ true -> 7 false -> 0 }}
             }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("symbols");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("types");
        assert!(typed.expression_table.expression_entries().any(|(_, node)| {
            matches!(node, ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::CaseMembership)
        }));
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let outcome = crate::interpret_entry(&checked, "main", &[]);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, expected, "{source}");
    }
}
