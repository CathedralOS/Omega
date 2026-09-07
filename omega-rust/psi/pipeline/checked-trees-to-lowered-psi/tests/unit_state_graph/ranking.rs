use super::*;
use terminal_psi::{TerminalNaturalRankComparison, TerminalRankedScc};

fn lower_writer() -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(&checked(WRITER), "Root::enter")
        .expect("the authored slice rank survives ordinary call-closure lowering")
}

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

#[test]
fn slice_ranked_writer_retains_its_witness_through_serialized_execution() {
    let lowered = lower_writer();
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("ordinary verification retains the natural component");
    assert_eq!(verified.accepted_control_cycles().len(), 1);
    let accepted = &verified.accepted_control_cycles()[0];
    assert_eq!(accepted.acceptance.decreases.len(), 2);
    assert_eq!(lowered.proof_bundle.control_cycles.len(), 1);
    let synopsis = terminal_codec::render_verified_proof_synopsis(&verified).unwrap();
    assert!(synopsis.contains("control-cycle"));
    assert!(synopsis.contains("Preserving"));
    assert!(synopsis.contains("Strict"));
    assert!(matches!(
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry),
        Err(terminal_fixed_fuel::FixedFuelError::ControlCycle(_))
    ));
    assert!(matches!(
        terminal_verifier::verify_module_for_native_ranked_countdown(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::Module(
            terminal_verifier::ModuleError::NonExecutableRankedScc(_)
        ))
    ));
    let executed = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("independent replay of the ranked writer");
    assert_eq!(executed.value(), TerminalExecutionResult::Unit);
    let bytes = executed
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("ordinary byte output");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(byte),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned byte");
            };
            u8::try_from(*byte).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(bytes, b"\n\x80ABZ\n!");
}

#[test]
fn natural_cycle_requires_its_exact_grouped_evidence() {
    let lowered = lower_writer();
    for mutation in 0..6 {
        let mut proof = lowered.proof_bundle.clone();
        match mutation {
            0 => proof.control_cycles.clear(),
            1 => proof.control_cycles.push(proof.control_cycles[0].clone()),
            2 => proof.control_cycles[0].certificate.edges.clear(),
            3 => proof.control_cycles[0].certificate.edges.reverse(),
            4 => {
                proof.control_cycles[0].certificate.ranking_relation =
                    semantic_vocabulary::RankingRelationId::new(1).unwrap();
            }
            5 => {
                let proof_admission::EvidenceRoute::CertificateDerived(envelope) =
                    &mut proof.control_cycles[0].certificate.well_foundedness
                else {
                    panic!("derived relation");
                };
                envelope.proof.conclusion = semantic_vocabulary::Proposition::Truth;
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &proof,
                &AdmissionProfile::default(),
            )
            .is_err(),
            "mutated grouped evidence {mutation}"
        );
    }
}

#[test]
fn natural_topology_cannot_omit_an_edge_or_accept_a_preserving_cycle() {
    let lowered = lower_writer();
    for mutation in 0..5 {
        let mut module = lowered.semantic_module.clone();
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| machine.ranked_scc.is_some())
            .unwrap();
        let Some(TerminalRankedScc::Natural(components)) = &mut machine.ranked_scc else {
            panic!("natural ranks");
        };
        let component = &mut components[0];
        match mutation {
            0 => {
                component.edges.pop();
            }
            1 => {
                component.ranks.pop();
            }
            2 => {
                for edge in &mut component.edges {
                    edge.comparison = TerminalNaturalRankComparison::Preserving;
                }
            }
            3 => {
                component.edges.reverse();
            }
            4 => {
                let strict = component
                    .edges
                    .iter_mut()
                    .find(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
                    .unwrap();
                strict.successor_rank = component
                    .ranks
                    .iter()
                    .find(|rank| rank.block == strict.source)
                    .unwrap()
                    .value;
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                terminal_verifier::validate_module(&module),
                Err(terminal_verifier::ModuleError::InvalidRankedScc(_))
            ),
            "topology mutation {mutation}"
        );
    }
}

#[test]
fn unchanged_tail_cannot_reuse_the_strict_comparison_certificate() {
    let lowered = lower_writer();
    let mut module = lowered.semantic_module.clone();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.ranked_scc.is_some())
        .unwrap();
    let Some(TerminalRankedScc::Natural(components)) = &machine.ranked_scc else {
        panic!("natural ranks");
    };
    let strict = *components[0]
        .edges
        .iter()
        .find(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .unwrap();
    let incoming_view = machine
        .blocks
        .iter()
        .find(|block| block.id == strict.target)
        .unwrap()
        .structural_parameters[0]
        .place;
    let source = machine
        .blocks
        .iter_mut()
        .find(|block| block.id == strict.source)
        .unwrap();
    let terminal_psi::Terminator::Jump {
        structural_arguments,
        ..
    } = &mut source.terminator
    else {
        panic!("selected tail edge");
    };
    structural_arguments[0].place = incoming_view;
    let length = source
        .operations
        .iter_mut()
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|value| value.id == strict.successor_rank)
        })
        .unwrap();
    length.kind = terminal_psi::OperationKind::ByteSequenceLength {
        source: incoming_view,
    };
    terminal_verifier::validate_module(&module)
        .expect("identity forwarding is well-typed and reconstructs the actual incoming rank");
    assert!(matches!(
        terminal_verifier::verify_module(
            &module,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::RejectedControlCycle { .. })
    ));
}

#[test]
fn checked_slice_rank_cannot_be_removed_or_redirected() {
    for mutation in 0..2 {
        let mut source = checked(WRITER);
        let plan = source
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| !plan.slice_length_ranks.is_empty())
            .unwrap();
        if mutation == 0 {
            plan.slice_length_ranks.clear();
        } else {
            plan.slice_length_ranks[0].parameter_position = 0;
        }
        assert!(checked_trees_to_lowered_psi::lower_machine(&source, "Root::enter").is_err());
    }
}

#[test]
fn serialized_ranked_writer_resumes_without_repeating_effects() {
    use terminal_fuel::TerminalFuelMeter;
    use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
    let lowered = lower_writer();
    let expected = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(1);
    for _ in 0..1000 {
        match execution.resume(&mut meter).unwrap() {
            TerminalExecutionStatus::Complete(_) => {
                assert_eq!(execution.effects(), expected.effects());
                return;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => {
                assert!(expected.effects().starts_with(execution.effects()));
                meter.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Crashed(crash) => {
                panic!("writer unexpectedly crashed: {crash:?}")
            }
        }
    }
    panic!("this finite writer input should complete within the test allowance");
}
