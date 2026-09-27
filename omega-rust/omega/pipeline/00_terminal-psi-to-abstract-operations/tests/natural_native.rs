use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_psi::{
    OperationKind, ProofBundle, TerminalModule, TerminalNaturalRankComparison, TerminalRankedScc,
    Terminator,
};
use terminal_psi_to_abstract_operations::{ArtifactLoweringError, lower_artifact};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve writer");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type writer");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check writer");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("lower authored writer and its ordinary caller");
    (lowered.semantic_module, lowered.proof_bundle)
}

#[test]
fn native_admission_retains_the_accepted_ranked_cycle_roster() {
    let (module, proof) = writer();
    let profile = AdmissionProfile::default();
    let verified = terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let expected = verified.accepted_control_cycles().to_vec();
    assert_eq!(expected.len(), 1);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let evidence = terminal_codec::encode_proof_section(&module, &proof).unwrap();
    let admitted = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &evidence,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .expect("ranked module admits natively");
    assert_eq!(admitted.accepted_control_cycles(), expected.as_slice());
    let input = admitted
        .try_into_native_input(&[])
        .expect("empty placed-view roster");
    assert_eq!(input.accepted_control_cycles(), expected.as_slice());
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
            assert!(terminal_codec::encode_proof_section(&module, &changed).is_err());
            continue;
        }
        let evidence = terminal_codec::encode_proof_section(&module, &changed).unwrap();
        assert!(
            matches!(
                lower_artifact(
                    terminal_psi_to_abstract_operations::ArtifactSections {
                        semantic_bytes: &semantic,
                        proof_bytes: &evidence,
                        obligation_ledger_bytes: None
                    },
                    &AdmissionProfile::default()
                )
                .map(|admitted| admitted.into_plan()),
                Err(ArtifactLoweringError::Verification(_))
            ),
            "ordinary proof mutation {mutation}"
        );
        assert!(
            matches!(
                lower_artifact(
                    terminal_psi_to_abstract_operations::ArtifactSections {
                        semantic_bytes: &semantic,
                        proof_bytes: &evidence,
                        obligation_ledger_bytes: None
                    },
                    &AdmissionProfile::default()
                )
                .and_then(|admitted| admitted.try_into_native_input(&[])),
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
    let evidence = terminal_codec::encode_proof_section(&module, &proof).unwrap();
    assert!(matches!(
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &evidence,
                obligation_ledger_bytes: None
            },
            &AdmissionProfile::default()
        )
        .map(|admitted| admitted.into_plan()),
        Err(ArtifactLoweringError::Verification(
            terminal_verifier::VerificationError::RejectedControlCycle { .. }
        ))
    ));
    assert!(matches!(
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &evidence,
                obligation_ledger_bytes: None
            },
            &AdmissionProfile::default()
        )
        .and_then(|admitted| admitted.try_into_native_input(&[])),
        Err(ArtifactLoweringError::Verification(_))
    ));
}
