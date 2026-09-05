use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees_to_checked_trees::lower_typed_trees;

#[derive(Clone, Copy)]
enum BranchForm {
    Separate,
    Combined,
    ExpressionTail,
}

fn checked_source(source: &str, form: BranchForm) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("type");
    if !matches!(form, BranchForm::Separate) {
        let machine = typed.machines()[0].clone();
        let nodes = typed.machine_states(&machine)[0].statement_nodes;
        let transitions: Vec<_> = typed
            .statement_table
            .statements(nodes)
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| match statement {
                StatementNode::Transition(transition) => Some((index, transition.target)),
                _ => None,
            })
            .collect();
        let [(first, _), (second, continuation)] = transitions.as_slice() else {
            panic!("two authored branch statements")
        };
        assert_eq!(*first + 1, *second);
        assert_eq!(*second + 1, nodes.count() as usize);
        if matches!(form, BranchForm::ExpressionTail) {
            let TransitionTargetNode::Value(expression) =
                typed.statement_table.transition_target(*continuation)
            else {
                panic!("tail value")
            };
            let expression = *expression;
            typed.statement_table.statements_mut(nodes)[*second] =
                StatementNode::Expression(expression);
        } else {
            let StatementNode::Transition(transition) =
                &mut typed.statement_table.statements_mut(nodes)[*first]
            else {
                unreachable!()
            };
            transition.continuation = *continuation;
            typed.machine_states_mut(&machine)[0].statement_nodes =
                arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
        }
    }
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn encoded(source: &str, form: BranchForm) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_source(source, form);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("encode semantics"),
        encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
    )
}

fn unsigned(width: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, width).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn assert_both_branches(source: &str, expected: TerminalScalarValue, form: BranchForm) {
    let (semantics, proof) = encoded(source, form);
    // Only artifact bytes survive from the producer; both choices execute
    // against the same decoded semantics and independently supplied inputs.
    for flag in [true, false] {
        assert_eq!(
            interpret_terminal_artifact(
                &semantics,
                &proof,
                &AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(flag)],
            )
            .unwrap_or_else(|error| panic!("{source}, flag {flag}: {error:#?}")),
            TerminalExecutionResult::Scalar(expected),
        );
    }
}

#[test]
fn guarded_anonymous_integer_returns_land_once_after_selected_evaluation() {
    for (scalar_type, expression, width, expected) in [
        ("u8", "300 - 293", 8, 7),
        ("u8", "(0 - 1) + 8", 8, 7),
        (
            "u64",
            "(18446744073709551615 + 1) - 1",
            64,
            u128::from(u64::MAX),
        ),
    ] {
        let source = format!(
            "machine value(flag: bool) -> {scalar_type}\nrequires {expected}{scalar_type} == {expected}{scalar_type}\nensures {expected}{scalar_type} == {expected}{scalar_type}\n{{ transition flag {{ true -> ({expression}) false -> {expected} }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            assert_both_branches(&source, unsigned(width, expected), form);
        }
    }
}

#[test]
fn guarded_returns_remap_saved_values_and_current_storage_in_both_arm_forms() {
    let source = "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ let mut current: u8 = 7; let saved: u8 = current; current = 8; transition flag { true -> saved false -> (current - 1) } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        assert_both_branches(source, unsigned(8, 7), form);
    }
}

#[test]
fn guarded_returns_share_control_with_named_state_successors() {
    for arms in [
        "true -> saved false -> finish(current - 1)",
        "true -> finish(current - 1) false -> saved",
    ] {
        let source = format!(
            "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{{ let mut current: u8 = 7; let saved: u8 = current; current = 8; transition flag {{ {arms} }} state finish(input: u8) -> u8 {{ input }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            assert_both_branches(&source, unsigned(8, 7), form);
        }
    }
}

#[test]
fn guarded_return_and_expression_tail_keep_distinct_selected_values() {
    let source = "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ let mut current: u8 = 7; let saved: u8 = current; current = 8; transition flag { true -> saved false -> (current - 1) } }";
    assert_both_branches(source, unsigned(8, 7), BranchForm::ExpressionTail);
}

#[test]
fn guarded_boolean_returns_preserve_short_circuit_and_saved_storage() {
    let source = "machine value(flag: bool) -> bool\nrequires true == true\nensures true == true\n{ let mut current: bool = false; let saved: bool = current; current = true; transition flag { true -> (current && !saved) false -> (!saved || current) } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        assert_both_branches(source, TerminalScalarValue::Boolean(true), form);
    }
}

#[test]
fn unselected_partial_arithmetic_does_not_execute() {
    let source = "machine value(denominator: u8) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ transition (1 <= denominator) { true -> (7u8 / denominator) false -> 7 } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        let (semantics, proof) = encoded(source, form);
        for (denominator, expected) in [(0, 7), (1, 7), (2, 3), (7, 1), (255, 0)] {
            assert_eq!(
                interpret_terminal_artifact(
                    &semantics,
                    &proof,
                    &AdmissionProfile::default(),
                    &[unsigned(8, denominator)]
                )
                .unwrap_or_else(|error| panic!("denominator {denominator}: {error:#?}")),
                TerminalExecutionResult::Scalar(unsigned(8, expected)),
            );
        }
    }
}

#[test]
fn unsigned_division_retains_guard_polarity_through_serialization() {
    for (condition, division_when_true) in [
        ("denominator == 0", false),
        ("0 == denominator", false),
        ("denominator <= 0", false),
        ("0 >= denominator", false),
        ("!(denominator != 0)", false),
        ("denominator != 0", true),
        ("!(denominator == 0)", true),
        ("0 < denominator", true),
        ("denominator >= 1", true),
    ] {
        let division = "7u8 / denominator";
        let (positive, negative) = if division_when_true {
            (division, "7")
        } else {
            ("7", division)
        };
        let source = format!(
            "machine value(denominator: u8) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{{ transition ({condition}) {{ true -> ({positive}) false -> ({negative}) }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            let (semantics, proof) = encoded(&source, form);
            for (denominator, expected) in [(0, 7), (1, 7), (2, 3), (7, 1), (255, 0)] {
                assert_eq!(
                    interpret_terminal_artifact(
                        &semantics,
                        &proof,
                        &AdmissionProfile::default(),
                        &[unsigned(8, denominator)],
                    )
                    .unwrap_or_else(|error| panic!(
                        "{source}, denominator {denominator}: {error:#?}"
                    )),
                    TerminalExecutionResult::Scalar(unsigned(8, expected)),
                );
            }
        }
    }
}

#[test]
fn signed_division_retains_both_nonzero_signs_on_the_selected_edge() {
    let signed = |value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        value: IntegerValue::Signed(value),
    };
    for (condition, division_when_true) in [
        ("denominator == 0", false),
        ("0 == denominator", false),
        ("!(denominator != 0)", false),
        ("denominator != 0", true),
        ("!(denominator == 0)", true),
    ] {
        let division = "7i8 / denominator";
        let (positive, negative) = if division_when_true {
            (division, "7")
        } else {
            ("7", division)
        };
        let source = format!(
            "machine value(denominator: i8) -> i8\nrequires 7i8 == 7i8\nensures 7i8 == 7i8\n{{ transition ({condition}) {{ true -> ({positive}) false -> ({negative}) }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            let (semantics, proof) = encoded(&source, form);
            for (denominator, expected) in [
                (-128, 0),
                (-7, -1),
                (-2, -3),
                (-1, -7),
                (0, 7),
                (1, 7),
                (2, 3),
                (127, 0),
            ] {
                assert_eq!(
                    interpret_terminal_artifact(
                        &semantics,
                        &proof,
                        &AdmissionProfile::default(),
                        &[signed(denominator)],
                    )
                    .unwrap_or_else(|error| panic!(
                        "{source}, denominator {denominator}: {error:#?}"
                    )),
                    TerminalExecutionResult::Scalar(signed(expected)),
                );
            }
        }
    }
}

#[test]
fn a_nonzero_guard_does_not_license_signed_division_overflow() {
    let source = "machine value(denominator: i8) -> i8\nrequires 0i8 == 0i8\nensures 0i8 == 0i8\n{ transition (denominator == 0) { true -> 0 false -> (-128i8 / denominator) } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        let checked = checked_source(source, form);
        let result = checked_trees_to_lowered_psi::lower_machine(&checked, "value");
        assert!(
            matches!(
                result,
                Err(checked_trees_to_lowered_psi::LoweringError::OperationProofUnavailable(_))
            ),
            "the nonzero divisor may still be -1: {result:?}",
        );
    }
}

#[test]
fn branch_return_coordinates_cannot_select_a_siblings_valid_value() {
    use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};
    let source = "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ transition flag { true -> 7 false -> 7 } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        let checked = checked_source(source, form);
        checked_trees_to_lowered_psi::lower_machine(&checked, "value")
            .expect("original branch lowers");
        for mutation in 0..3 {
            let mut changed = checked.clone();
            let CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut changed.facts.flow.terminal_scalar_graphs.machines[0].states[0].terminator
            else {
                panic!("conditional plan")
            };
            let chosen = if mutation == 2 { when_false } else { when_true };
            let CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = chosen
            else {
                panic!("return branch")
            };
            match mutation {
                0 | 2 => *is_continuation = !*is_continuation,
                _ => *statement_ordinal += 1,
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "mutation {mutation}"
            );
        }
    }
}
