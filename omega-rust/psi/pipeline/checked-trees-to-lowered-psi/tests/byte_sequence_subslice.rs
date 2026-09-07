use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, interpret_terminal_artifact_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
    data Relay {}
    machine Relay::write(bytes: &[u8]) reaches Output { Output::write(bytes[..]); }
    data Helper {}
    machine Helper::write(bytes: &[u8]) reaches Output {
        Relay::write(bytes[..]);
        Output::write(bytes);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::write("\x80A");
        Helper::write("");
        Output::write("last");
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

#[test]
fn source_subslice_crosses_helpers_and_preserves_original_view_and_continuation() {
    let checked = checked(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("source subslice has an ordinary helper call plan");
    assert_eq!(
        execute(&lowered),
        [
            vec![128, 65],
            vec![128, 65],
            vec![],
            vec![],
            b"last".to_vec()
        ]
    );
    let selections = optimization::PsiOptimizationSelections::new([
        optimization::PsiOptimization::DeadPureScalarElimination,
    ])
    .unwrap();
    let optimized = lowered_psi_to_lowered_psi::run_psi_optimization(lowered, selections).unwrap();
    assert_eq!(
        execute(optimized.lowered()),
        [
            vec![128, 65],
            vec![128, 65],
            vec![],
            vec![],
            b"last".to_vec()
        ]
    );
}

fn execute(lowered: &lowered_psi::LoweredPsi) -> Vec<Vec<u8>> {
    let result = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    result
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall {
                byte_sequence_arguments,
                ..
            } = effect
            else {
                panic!("byte boundary")
            };
            byte_sequence_arguments[0]
                .clone()
                .expect("exact byte contents")
        })
        .collect()
}

#[test]
fn unavailable_entry_length_contract_does_not_authorize_a_source_tail() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
        data Helper {}
        machine Helper::write(bytes: &[u8])
        requires bytes.len > 0;
        reaches Output
        { Output::write(bytes[1..]); }
    "#;
    let checked = checked(source);
    // Source checking retains the explicit endpoint, but a byte-length entry
    // contract is not yet represented in Terminal. It cannot become a trusted
    // body assumption just because the source checker accepted it.
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Helper::write").unwrap_err();
    assert!(
        matches!(
            error,
            checked_trees_to_lowered_psi::LoweringError::OperationProofUnavailable(_)
        ),
        "{error:?}"
    );
}

#[test]
fn byte_subslice_is_evaluated_between_surrounding_scalar_calls() {
    let source = r#"
        boundary trait Output { machine write(first: u8, bytes: &[u8], last: u8) reaches Output; }
        data Scalar {}
        machine Scalar::identity(value: u8) -> u8
        requires 0u8 == 0u8
        ensures result == value
        { value }
        data Relay {}
        machine Relay::write(first: u8, bytes: &[u8], last: u8) reaches Output {
            Output::write(first, bytes, last);
        }
        data Helper {}
        machine Helper::write(bytes: &[u8]) reaches Output {
            Relay::write(Scalar::identity(7), bytes[..], Scalar::identity(9));
        }
        data Root {}
        machine Root::enter() reaches Output { Helper::write("raw"); }
    "#;
    for callee in ["Relay::write", "Output::write"] {
        let source = source.replace("Relay::write(Scalar", &format!("{callee}(Scalar"));
        let checked = checked(&source);
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
        let machine = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        matches!(
                            operation.kind,
                            terminal_psi::OperationKind::ByteSequenceSubslice { .. }
                        )
                    })
            })
            .expect("caller has derived view");
        let mut current = machine.entry;
        let mut visited = Vec::new();
        let mut sequence = Vec::new();
        loop {
            assert!(!visited.contains(&current), "acyclic argument evaluation");
            visited.push(current);
            let block = machine
                .blocks
                .iter()
                .find(|block| block.id == current)
                .unwrap();
            sequence.extend(
                block
                    .operations
                    .iter()
                    .filter_map(|operation| match operation.kind {
                        terminal_psi::OperationKind::Call { .. } => Some("scalar"),
                        terminal_psi::OperationKind::ByteSequenceSubslice { .. } => {
                            Some("subslice")
                        }
                        _ => None,
                    }),
            );
            match block.terminator {
                terminal_psi::Terminator::Jump { target, .. } => current = target,
                terminal_psi::Terminator::ReturnUnit { .. } => break,
                _ => panic!("straight-line call evaluation"),
            }
        }
        assert_eq!(sequence, ["scalar", "subslice", "scalar"], "{callee}");
        assert_eq!(execute(&lowered), [b"raw".to_vec()]);
    }
}

#[test]
fn changed_subslice_source_range_or_custody_rejects() {
    use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan};
    let checked = checked(SOURCE);
    let plan_index = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .position(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation,
            CheckedUnitEffectOperationPlan::CallUnit { structural_arguments, .. }
                if structural_arguments.iter().any(|argument| matches!(argument.source,
                    CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. })))
            })
        })
        .unwrap();
    for mutation in 0..7 {
        let mut changed = checked.clone();
        let plan = &mut changed.facts.flow.terminal_unit_effects.machines[plan_index];
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            coordinate,
            ..
        } = &mut plan.operations[0]
        else {
            panic!("ordinary subslice call");
        };
        let argument = &mut structural_arguments[0];
        let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            parameter_index,
            expression,
            ..
        } = &mut argument.source
        else {
            panic!("source range");
        };
        match mutation {
            0 => *parameter_index += 1,
            1 => *expression = arena::Handle::invalid(),
            2 => argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow,
            3 => coordinate.call_ordinal += 1,
            6 => {
                changed
                    .facts
                    .operators
                    .uses
                    .append(checked_trees::CheckedOperatorUseFact {
                        expression: *expression,
                        selected_operator_symbol: plan.machine,
                        ..Default::default()
                    });
            }
            4 | 5 => {
                let typed_trees::expression::ExpressionNode::Indexed(indexed) =
                    changed.typed.expression_table.expression(*expression)
                else {
                    panic!("indexed range");
                };
                let range_handle = indexed.index;
                let typed_trees::expression::ExpressionNode::Range(range) =
                    changed.typed.expression_table.expression_mut(range_handle)
                else {
                    panic!("range");
                };
                if mutation == 4 {
                    range.end_inclusive = true;
                } else {
                    range.start = *expression;
                }
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn subslice_arguments_preserve_scalar_and_nested_structural_boundary_results() {
    let source = r#"
        pub data Token { flag: bool; }
        boundary trait Output {
            machine measure(bytes: &[u8]) -> u64 reaches Output;
            machine create(bytes: &[u8]) -> Token reaches Output;
            machine finish(token: Token, bytes: &[u8], count: u64) reaches Output;
        }
        data Helper {}
        machine Helper::write(bytes: &[u8]) reaches Output {
            let count: u64 = Output::measure(bytes[..]);
            Output::finish(Output::create(bytes[..]), bytes[..], count);
        }
        data Root {}
        machine Root::enter() reaches Output { Helper::write("raw"); }
    "#;
    let lowered =
        checked_trees_to_lowered_psi::lower_machine(&checked(source), "Root::enter").unwrap();
    let module =
        terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap()).unwrap();
    let proof =
        terminal_codec::decode_proof_bundle(&encode_proof_bundle(&lowered.proof_bundle).unwrap())
            .unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequenceSubslice { .. }
            ))
            .count(),
        3
    );
}
