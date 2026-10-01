use super::{
    AdmissionProfile, TerminalEffect, TerminalExecutionResult, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_psi::{TerminalNaturalRankComparison, TerminalRankedScc};

fn lower_writer() -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(WRITER),
        TerminalMachineSelection::Name("Root::enter"),
    )
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

const INDEXED_WRITER: &str = r#"
    boundary trait Output { machine write(byte: u8) reaches Output; }
    data Root {}
    machine Root::relay(&mut self, bytes: &[u8], other: &[u8], index: u64)
    terminates by (index, bytes.len) -> Nat::BoundedDistance;
    reaches Output {
        Output::write((index as u8 in Wrapping) as u8);
        transition index < bytes.len {
            true -> relay(bytes, other, index + 1)
            false -> done()
        }
        state done(&mut self) {}
    }
    machine Root::enter(&mut self) reaches Output {
        self.relay("ABC", "decoy", 0);
        self.relay("", "decoy", 0);
        self.relay("A", "decoy", 5);
    }
"#;

#[test]
fn collection_distance_rank_executes_empty_and_already_crossed_bounds() {
    assert_indexed_writer_execution(INDEXED_WRITER);
}

#[test]
fn increasing_to_collection_length_retains_the_same_rank() {
    assert_indexed_writer_execution(&INDEXED_WRITER.replace(
        "(index, bytes.len) -> Nat::BoundedDistance",
        "index -> Nat::IncreasingTo(bytes.len)",
    ));
}

fn assert_indexed_writer_execution(source: &str) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("collection distance retains an independently checked natural rank");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let receiver = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let executed = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[receiver],
            ..Default::default()
        },
        &mut AcceptTerminalEffects,
    )
    .expect("serialized replay verifies the original rank before execution");
    let values = executed
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("ordinary output");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned index");
            };
            *value
        })
        .collect::<Vec<_>>();
    assert_eq!(values, [0, 1, 2, 3, 0, 5]);
    let mut missing_proof = lowered.proof_bundle.clone();
    missing_proof.control_cycles.clear();
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing_proof,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "the producer's retained measure is not termination evidence"
    );
}

#[test]
fn collection_distance_rank_rejects_a_nonadvancing_cursor() {
    let source = INDEXED_WRITER.replace("index + 1", "index");
    let diagnostics = crate::front_end::checked_program_result(&source)
        .expect_err("an unchanged cursor cannot establish strict descent");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("terminat")
                || diagnostic.message.contains("decreas")),
        "{diagnostics:?}"
    );
}

#[test]
fn collection_distance_rank_cannot_substitute_another_collection_or_scalar() {
    for replacement in [2, 3] {
        let mut checked = crate::front_end::checked_program(INDEXED_WRITER);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| !plan.natural_ranks.is_empty())
            .unwrap();
        let rank = &mut plan.natural_ranks[0];
        let state = checked
            .typed
            .machines()
            .iter()
            .flat_map(|machine| checked.typed.machine_states(machine))
            .find(|state| state.symbol == rank.state)
            .unwrap();
        let parameter = &checked.typed.state_parameters(state)[replacement];
        rank.measure = checked_trees::CheckedNaturalRankMeasure::CollectionLengthDistance {
            collection: parameter.symbol,
            collection_position: u32::try_from(replacement).unwrap(),
        };
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .is_err(),
            "substituted distance bound {replacement}"
        );
    }
}

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
    // A slice-length rank rides the u64 carrier, so the composed cycle bound
    // exists mathematically but cannot fit a u64 ceiling. The writer reports
    // the component as unbounded by its rank (logical_work.md) rather than
    // fabricating a certificate.
    assert!(matches!(
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry),
        Err(
            terminal_fixed_fuel::FixedFuelError::UnboundedCycleComponent {
                cause: terminal_fixed_fuel::UnboundedCycleCause::UnboundedRank,
                ..
            }
        )
    ));
    let executed = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
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
        let mut source = crate::front_end::checked_program(WRITER);
        let plan = source
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| !plan.natural_ranks.is_empty())
            .unwrap();
        if mutation == 0 {
            plan.natural_ranks.clear();
        } else {
            plan.natural_ranks[0].parameter_position = 0;
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &source,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
    }
}

/// A remainder-endpoint countdown whose cyclic state reorders and renames its
/// formals: `walk` ranks `remaining`, and the entry transition binds that role
/// to `iterate`'s `pending` (position 2) while `cap` and `divisor` arrive as
/// `limit` and `width` (positions 1 and 0).
const REMAINDER_COUNTDOWN: &str = r#"
    machine walk(remaining: u64, cap: u64, divisor: u64)
        requires 0 <= remaining && remaining <= 5
            && 0 <= cap && cap <= 20
            && 1 <= divisor && divisor <= 5
    terminates by remaining in 0..(cap % divisor + 6);
    -> u64 {
        transition { _ -> iterate(divisor, cap, remaining) }
        state iterate(width: u64, limit: u64, pending: u64)
            requires 1 <= width && width <= 5
                && 0 <= limit && limit <= 20
                && 0 <= pending && pending <= 5
        {
            transition pending > 0 {
                true -> iterate(width - 0, limit - 0, pending - 1)
                false -> pending
            }
        }
    }
"#;

/// The published rank names the formal carrying the subject's entry role —
/// `pending`, not a name twin of `remaining` — in whichever producer owns the
/// cycle. The free countdown is a checked scalar graph: its runtime rank
/// judgment covers the remainder endpoint. A borrowed receiver keeps the same
/// cycle on the Unit state graph, and the composed plan names the same formal.
#[test]
fn named_state_rank_names_the_carried_formal() {
    let source = crate::front_end::checked_program(&format!(
        "{REMAINDER_COUNTDOWN}
        data Root {{}}
        machine Root::enter() -> u64 {{ walk(5, 17, 4) }}"
    ));
    let walk = source
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk resolves");
    let iterate = source
        .machine_states(walk)
        .iter()
        .find(|state| state.name.as_str() == "iterate")
        .expect("iterate resolves");
    let ranked = source
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(walk.symbol)
        .and_then(|graph| graph.ranked_scc.as_ref())
        .expect("the free countdown is a ranked scalar graph");
    let pending_scalar = source
        .state_parameters(iterate)
        .iter()
        .filter(|parameter| {
            source
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .position(|parameter| parameter.name.as_str() == "pending")
        .expect("iterate binds a scalar pending");
    assert_eq!(
        (
            ranked.header_state,
            ranked.rank_scalar_parameter_index as usize
        ),
        (iterate.symbol, pending_scalar),
        "the scalar graph ranks the formal carrying `remaining`'s entry role"
    );
    checked_trees_to_lowered_psi::lower_machine(
        &source,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("the carried rank lowers through the attached call closure");

    let countdown = REMAINDER_COUNTDOWN
        .replace(
            "machine walk(remaining",
            "machine Root::walk(&self, remaining",
        )
        .replace("state iterate(width", "state iterate(&self, width");
    let source = crate::front_end::checked_program(&format!(
        "data Root {{}}
        {countdown}
        machine Root::enter(&self) -> u64 {{ self.walk(5, 17, 4) }}"
    ));
    let walk = source
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::walk")
        .expect("walk resolves");
    let plan = source
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == walk.symbol)
        .expect("the receiver countdown keeps a composed plan");
    let iterate = source
        .machine_states(walk)
        .iter()
        .find(|state| state.name.as_str() == "iterate")
        .expect("iterate resolves");
    let pending = source
        .state_parameters(iterate)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.name.as_str() == "pending")
        .map(|(position, parameter)| (position, parameter.symbol))
        .expect("iterate binds pending");
    let rank = plan
        .natural_ranks
        .iter()
        .find(|rank| rank.state == iterate.symbol)
        .expect("iterate carries a natural rank");
    assert_eq!(
        (rank.parameter_position as usize, rank.parameter),
        pending,
        "the rank names the formal carrying `remaining`'s entry role"
    );
    checked_trees_to_lowered_psi::lower_machine(
        &source,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("the carried rank lowers through the attached call closure");
}

/// The same carried rank survives when the ranked callee is embedded in an
/// attached caller's Unit closure, the route `Main::main` takes.
#[test]
fn named_state_rank_lowers_inside_an_attached_caller() {
    let source = crate::front_end::checked_program(&format!(
        "{REMAINDER_COUNTDOWN}
        boundary trait Exit {{ machine exit(code: u8) reaches Exit; }}
        data Main {{}}
        machine Main::main(&mut self) reaches Exit {{
            let tick: u64 = 40 + 2;
            let outcome: u64 = walk(5, 17, 4);
            transition outcome == 0 && tick == 42 {{
                true -> passed()
                _ -> failed()
            }}
            state passed(&mut self) {{ Exit::exit(0u8); }}
            state failed(&mut self) {{ Exit::exit(1u8); }}
        }}"
    ));
    checked_trees_to_lowered_psi::lower_machine(
        &source,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("the embedded carried rank lowers through the Unit closure");
}
