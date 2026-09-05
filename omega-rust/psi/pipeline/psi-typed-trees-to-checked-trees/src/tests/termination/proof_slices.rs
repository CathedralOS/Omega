use super::*;

const SEQUENCE: &str = r#"
    data Sequence {
        case Empty;
        case Cons(head: u64, tail: Sequence);
    }
"#;

fn typed(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn proof_slice_extraction_definition_accepts_guarded_tail_recursion() {
    for measure in ["items -> Slice::Length", "items"] {
        let source = format!(
            r#"{SEQUENCE}
            machine extract(items: &[u64]) -> Sequence
            terminates by {measure};
            {{
                transition items.len > 0 {{
                    true -> Sequence::Cons {{head: items[0], tail: extract(items[1..])}}
                    false -> Sequence::Empty
                }}
            }}"#
        );
        let program = typed(&source);
        let extractor = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "extract")
            .expect("extractor");
        assert!(
            psi_typed_trees::proof_only::classify(&program).is_proof_machine(&program, extractor)
        );
        let entry_parameter = &program.state_parameters(&program.machine_states(extractor)[0])[0];
        let subjects =
            psi_typed_trees::ranking::resolve_machine_witness_subjects(&program, extractor)
                .expect("resolved witness");
        assert!(
            matches!(subjects.as_slice(), [subject] if matches!(program.expression_table.expression(*subject),
            psi_typed_trees::expression::ExpressionNode::Name(path) if path.symbol == entry_parameter.symbol && path.head_symbol == entry_parameter.symbol))
        );
        lower_typed_trees(program)
            .unwrap_or_else(|diagnostics| panic!("{measure}: {diagnostics:#?}"));
    }
}

#[test]
fn proof_slice_recursion_requires_the_same_nonempty_tail_edge() {
    for (name, body) in [
        (
            "unchanged",
            "transition items.len > 0 { true -> extract(items, other) false -> Sequence::Empty }",
        ),
        (
            "zero_start",
            "transition items.len > 0 { true -> extract(items[0..], other) false -> Sequence::Empty }",
        ),
        (
            "unrelated_guard",
            "transition other.len > 0 { true -> extract(items[1..], other) false -> Sequence::Empty }",
        ),
        (
            "unrelated_tail",
            "transition items.len > 0 { true -> extract(other[1..], other) false -> Sequence::Empty }",
        ),
        (
            "false_arm",
            "transition items.len > 0 { true -> Sequence::Empty false -> extract(items[1..], other) }",
        ),
        ("unguarded", "extract(items[1..], other)"),
        (
            "rebound",
            "items = other; transition items.len > 0 { true -> extract(items[1..], other) false -> Sequence::Empty }",
        ),
    ] {
        let parameter = if name == "rebound" {
            "mut items"
        } else {
            "items"
        };
        let source = format!(
            r#"{SEQUENCE}
            machine extract({parameter}: &[u64], other: &[u64]) -> Sequence
            terminates by items -> Slice::Length;
            {{ {body} }}"#
        );
        let result = lower_typed_trees(typed(&source));
        let Err(diagnostics) = result else {
            panic!("{name}: unproved recursive edge accepted")
        };
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove the measure")
                || diagnostic
                    .message
                    .contains("cannot prove the `terminates by` ranking")),
            "{name}: rejection must include the unproved decrease, not merely an unrelated error: {diagnostics:#?}"
        );
    }
}

#[test]
fn nested_proof_slice_calls_cannot_borrow_another_edges_decrease() {
    for (name, body) in [
        (
            "nested_unchanged",
            "transition items.len > 0 { true -> Sequence::Cons {head: items[0], tail: extract(items, other)} false -> Sequence::Empty }",
        ),
        (
            "nested_zero_start",
            "transition items.len > 0 { true -> Sequence::Cons {head: items[0], tail: extract(items[0..], other)} false -> Sequence::Empty }",
        ),
        (
            "both_arms",
            "transition items.len > 0 { true -> Sequence::Cons {head: items[0], tail: extract(items[1..], other)} false -> Sequence::Cons {head: 0, tail: extract(items[1..], other)} }",
        ),
        (
            "recursive_guard",
            "transition (items.len > 0) && ((extract(items[1..], other)) == Sequence::Empty) { true -> Sequence::Cons {head: items[0], tail: extract(items[1..], other)} _ -> Sequence::Empty }",
        ),
        (
            "helper_state_shadow",
            "transition { _ -> step(other) } state step(items: &[u64]) -> Sequence { transition items.len > 0 { true -> Sequence::Cons {head: items[0], tail: extract(items[1..], items)} false -> Sequence::Empty } }",
        ),
    ] {
        let source = format!(
            r#"{SEQUENCE}
            machine extract(items: &[u64], other: &[u64]) -> Sequence
            terminates by items -> Slice::Length;
            {{ {body} }}"#
        );
        let Err(diagnostics) = lower_typed_trees(typed(&source)) else {
            panic!("{name}: accepted unproved proof recursion")
        };
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove the measure")
                || diagnostic
                    .message
                    .contains("cannot prove the `terminates by` ranking")),
            "{name}: {diagnostics:#?}"
        );
    }
}

#[test]
fn proof_slice_tail_cannot_override_a_different_ranking_view() {
    let source = format!(
        r#"{SEQUENCE}
        machine extract(items: &[u64]) -> Sequence
        terminates by items -> Nat::Descending;
        {{
            transition items.len > 0 {{
                true -> Sequence::Cons {{head: items[0], tail: extract(items[1..])}}
                false -> Sequence::Empty
            }}
        }}"#
    );
    assert!(lower_typed_trees(typed(&source)).is_err());
}

#[test]
fn runtime_slice_ranking_checks_every_recursive_edge() {
    let source = r#"
        machine walk(items: &[u64], retry: bool) -> u64
        terminates by items -> Slice::Length;
        {
            transition {
                items.len > 0 -> walk(items[1..], retry)
                retry -> walk(items, retry)
                _ -> 0
            }
        }
    "#;
    let Err(diagnostics) = lower_typed_trees(typed(source)) else {
        panic!("the shrinking first edge cannot justify the unchanged second edge");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")),
        "{diagnostics:#?}"
    );
}

#[test]
fn proof_slice_decrease_requires_exact_witness_and_bare_parameter_identity() {
    use psi_typed_trees::expression::ExpressionNode;
    use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    let source = format!(
        r#"{SEQUENCE}
        machine unrelated(items: &[u64]) -> u64 {{ 0 }}
        machine extract(items: &[u64]) -> Sequence
        terminates by items -> Slice::Length;
        {{ transition items.len > 0 {{
            true -> Sequence::Cons {{head: items[0], tail: extract(items[1..])}}
            false -> Sequence::Empty
        }} }}"#
    );
    for corruption in ["foreign_witness", "member_tail"] {
        let mut program = typed(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "extract")
            .expect("extract");
        let entry = &program.machine_states(machine)[0];
        let parameter = program.state_parameters(entry)[0].clone();
        let witness = psi_typed_trees::ranking::resolve_machine_witness_subjects(&program, machine)
            .expect("witness")[0];
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(entry.statement_nodes)[0]
        else {
            panic!("transition")
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            panic!("guard")
        };
        let TransitionTargetNode::Value(result) =
            program.statement_table.transition_target(transition.target)
        else {
            panic!("result")
        };
        let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(*result)
        else {
            panic!("Cons")
        };
        let tail = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == "tail")
            .expect("tail")
            .value;
        let ExpressionNode::Call(call) = program.expression_table.expression(tail) else {
            panic!("recursive call")
        };
        let argument = program.expression_table.expression_handles(call.arguments)[0];
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument) else {
            panic!("slice")
        };
        let collection = indexed.collection;
        assert!(psi_validation::slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
        if corruption == "foreign_witness" {
            let foreign = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "unrelated")
                .expect("unrelated");
            let symbol = program.state_parameters(&program.machine_states(foreign)[0])[0].symbol;
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(witness)
            else {
                panic!("name")
            };
            path.symbol = symbol;
            path.head_symbol = symbol;
            let symbols = path.member_symbols;
            program
                .expression_table
                .set_name_path_member_symbol_at_offset(symbols, 0, symbol);
        } else {
            let mut members = psi_arena::HandleSpan::empty();
            program.expression_table.push_name_path_member(
                &mut members,
                psi_typed_trees::name::Identifier::from("items"),
            );
            program.expression_table.push_name_path_member(
                &mut members,
                psi_typed_trees::name::Identifier::from("other"),
            );
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(collection)
            else {
                panic!("name")
            };
            path.members = members;
            assert!(!psi_validation::slice_tail_strictly_decreases(
                &program, guard, argument, &parameter
            ));
        }
        assert!(lower_typed_trees(program).is_err(), "{corruption}");
    }
}
