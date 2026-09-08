use abstract_operations::AbstractOperation;
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_psi::{
    OperationKind, ProofBundle, TerminalModule, TerminalNaturalRankComparison, TerminalRankedScc,
    Terminator,
};
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, NativeArtifactOperationPlan, lower_artifact_sections,
    lower_artifact_sections_for_native_ranked_countdown,
    lower_artifact_sections_for_native_realization, lower_artifact_sections_for_optimization,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const WRITER: &str = r#"
    boundary trait Output { machine write(byte: u8) reaches Output; }
    machine relay(bytes: &[u8], newline: bool)
    terminates by bytes -> Slice::Length;
    reaches Output {
        transition bytes.len > 0 {
            true -> emit(bytes[0], bytes[1..], newline)
            false -> finish(newline)
        }
        state emit(byte: u8, bytes: &[u8], newline: bool) {
            Output::write(byte);
            transition bytes.len > 0 {
                true -> emit(bytes[0], bytes[1..], newline)
                false -> finish(newline)
            }
        }
        state finish(newline: bool) {
            transition newline {
                true -> emit_newline(10u8)
                false -> done()
            }
        }
        state emit_newline(byte: u8) { Output::write(byte); }
        state done() {}
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("", false);
        relay("", true);
        relay("\x80AB", false);
        relay("Z", true);
        Output::write(33u8);
    }
"#;

fn writer() -> (TerminalModule, ProofBundle) {
    let tokens = Lexer::new(WRITER).tokenize().expect("tokenize writer");
    let syntax = parse_syntax_trees(&tokens).expect("parse writer");
    let resolved = lower_syntax_trees(&syntax).expect("resolve writer");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type writer");
    let checked = lower_typed_trees(typed).expect("check writer");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("lower authored writer and its ordinary caller");
    (lowered.semantic_module, lowered.proof_bundle)
}

#[test]
fn natural_slice_writer_uses_ordinary_native_admission_without_losing_its_cycle() {
    let (module, proof) = writer();
    let profile = AdmissionProfile::default();
    let verified = terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    assert_eq!(verified.accepted_control_cycles().len(), 1);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let evidence = terminal_codec::encode_proof_bundle(&proof).unwrap();
    let ordinary = lower_artifact_sections(&semantic, &evidence, &profile)
        .expect("ordinary lowering already accepts verified natural cycles");
    for machine in &module.machines {
        let function = ordinary
            .functions
            .iter()
            .find(|function| function.machine == machine.id)
            .unwrap();
        assert_eq!(function.entry, machine.entry);
        assert_eq!(function.block_entries.len(), machine.blocks.len());
        for (block_ordinal, (block, entry)) in machine
            .blocks
            .iter()
            .zip(&function.block_entries)
            .enumerate()
        {
            assert_eq!(entry.block, block.id);
            assert_eq!(entry.structural_parameters, block.structural_parameters);
            let end = function
                .block_entries
                .get(block_ordinal + 1)
                .map_or(function.operations.len(), |next| next.operation_offset);
            let operation = &function.operations[end - 1];
            match (&block.terminator, operation) {
                (
                    Terminator::Jump {
                        edge,
                        target,
                        structural_arguments,
                        ..
                    },
                    AbstractOperation::Jump {
                        psi_edge,
                        target: actual_target,
                        structural_bindings,
                        ..
                    },
                ) => {
                    assert_eq!((psi_edge, actual_target), (edge, target));
                    assert_eq!(
                        structural_bindings
                            .iter()
                            .map(|binding| &binding.argument)
                            .collect::<Vec<_>>(),
                        structural_arguments.iter().collect::<Vec<_>>()
                    );
                    let destination = machine
                        .blocks
                        .iter()
                        .find(|block| block.id == *target)
                        .unwrap();
                    assert_eq!(
                        structural_bindings
                            .iter()
                            .map(|binding| binding.parameter)
                            .collect::<Vec<_>>(),
                        destination
                            .structural_parameters
                            .iter()
                            .map(|parameter| parameter.place)
                            .collect::<Vec<_>>()
                    );
                }
                (
                    Terminator::Conditional {
                        condition,
                        when_true,
                        when_false,
                    },
                    AbstractOperation::Conditional {
                        condition: actual_condition,
                        when_true: actual_true,
                        when_false: actual_false,
                    },
                ) => {
                    assert_eq!(actual_condition, condition);
                    for (expected, actual) in [(when_true, actual_true), (when_false, actual_false)]
                    {
                        assert_eq!(
                            (actual.psi_edge, actual.target),
                            (expected.edge, expected.target)
                        );
                        assert_eq!(
                            actual
                                .structural_bindings
                                .iter()
                                .map(|binding| &binding.argument)
                                .collect::<Vec<_>>(),
                            expected.structural_arguments.iter().collect::<Vec<_>>()
                        );
                    }
                }
                (
                    Terminator::ReturnUnit { edge, .. },
                    AbstractOperation::ReturnUnit { psi_edge, .. },
                ) => assert_eq!(psi_edge, edge),
                _ => panic!("source terminator changed: {:?}", block.terminator),
            }
        }
    }
    let natural = module
        .machines
        .iter()
        .find_map(|machine| match &machine.ranked_scc {
            Some(TerminalRankedScc::Natural(components)) => Some(components),
            _ => None,
        })
        .unwrap();
    assert!(
        natural[0]
            .edges
            .iter()
            .any(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
    );
    let selected = lower_artifact_sections_for_native_realization(&semantic, &evidence, &profile)
        .expect("natural ranks are ordinary admission, not the legacy countdown exception");
    let optimization = lower_artifact_sections_for_optimization(&semantic, &evidence, &profile)
        .expect("the native continuation retains the same verified natural graph");
    assert_eq!(optimization.plan(), &ordinary);
    assert_eq!(optimization.context().module(), &module);
    assert_eq!(optimization.context().proof_bundle(), &proof);
    assert_eq!(selected, NativeArtifactOperationPlan::Ordinary(ordinary));
}

#[test]
fn explicit_countdown_entrance_does_not_admit_a_natural_slice_writer() {
    let (module, proof) = writer();
    assert!(matches!(
        lower_artifact_sections_for_native_ranked_countdown(
            &terminal_codec::encode_module(&module).unwrap(),
            &terminal_codec::encode_proof_bundle(&proof).unwrap(),
            &AdmissionProfile::default(),
        ),
        Err(ArtifactLoweringError::Verification(_))
    ));
}

#[test]
fn native_natural_routing_requires_exact_grouped_evidence() {
    let (module, proof) = writer();
    for mutation in 0..3 {
        let mut changed = proof.clone();
        match mutation {
            0 => changed.control_cycles.clear(),
            1 => changed.control_cycles[0].certificate.edges.reverse(),
            2 => {
                changed.control_cycles[0].certificate.ranking_relation =
                    semantic_vocabulary::RankingRelationId::new(u64::MAX).unwrap()
            }
            _ => unreachable!(),
        }
        let semantic = terminal_codec::encode_module(&module).unwrap();
        assert!(
            terminal_verifier::verify_module(&module, &changed, &AdmissionProfile::default())
                .is_err()
        );
        if mutation == 1 {
            // The artifact encoder also refuses noncanonical edge order.
            assert!(terminal_codec::encode_proof_bundle(&changed).is_err());
            continue;
        }
        let evidence = terminal_codec::encode_proof_bundle(&changed).unwrap();
        assert!(
            matches!(
                lower_artifact_sections(&semantic, &evidence, &AdmissionProfile::default()),
                Err(ArtifactLoweringError::Verification(_))
            ),
            "ordinary proof mutation {mutation}"
        );
        assert!(
            matches!(
                lower_artifact_sections_for_native_realization(
                    &semantic,
                    &evidence,
                    &AdmissionProfile::default()
                ),
                Err(ArtifactLoweringError::Verification(_))
            ),
            "native proof mutation {mutation}"
        );
    }
}

#[test]
fn unchanged_tail_cannot_reuse_the_strict_native_cycle_proof() {
    let (mut module, proof) = writer();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.ranked_scc.is_some())
        .unwrap();
    let Some(TerminalRankedScc::Natural(components)) = &machine.ranked_scc else {
        panic!("natural component");
    };
    let strict = *components[0]
        .edges
        .iter()
        .find(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .unwrap();
    let incoming = machine
        .blocks
        .iter()
        .find(|block| block.id == strict.target)
        .unwrap()
        .structural_parameters[0]
        .place;
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| block.id == strict.source)
        .unwrap();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut block.terminator
    else {
        panic!("strict tail jump");
    };
    structural_arguments[0].place = incoming;
    block
        .operations
        .iter_mut()
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == strict.successor_rank)
        })
        .unwrap()
        .kind = OperationKind::ByteSequenceLength { source: incoming };
    terminal_verifier::validate_module(&module).expect("identity forwarding remains well typed");
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let evidence = terminal_codec::encode_proof_bundle(&proof).unwrap();
    assert!(matches!(
        lower_artifact_sections(&semantic, &evidence, &AdmissionProfile::default()),
        Err(ArtifactLoweringError::Verification(
            terminal_verifier::VerificationError::RejectedControlCycle { .. }
        ))
    ));
    assert!(matches!(
        lower_artifact_sections_for_native_realization(
            &semantic,
            &evidence,
            &AdmissionProfile::default()
        ),
        Err(ArtifactLoweringError::Verification(_))
    ));
}
