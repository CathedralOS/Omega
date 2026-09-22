//! Published cyclic Unit calls preserve literal contents through resumable execution.

use super::checked_source;
use crate::TerminalMachineSelection;
use crate::{LoweringError, lower_machine};
use terminal_interpreter::TerminalStructuralInputs;
mod mixed_and_custody;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};

#[test]
fn cyclic_literal_calls_preserve_selected_bytes_at_every_fuel_pause() {
    let checked = checked_source(
        r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
machine forward(first: &[u8], second: &[u8]) reaches Trace {
    Trace::write(first);
    Trace::write(second);
}
data Main { counter: u64 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 0;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> choose() _ -> done() }
    }
    state choose(&mut self) {
        transition self.counter == 0 { true -> first() _ -> later() }
    }
    state first(&mut self) {
        forward("first", "");
        transition { _ -> advance() }
    }
    state later(&mut self) {
        forward("later", "λ");
        transition { _ -> advance() }
    }
    state advance(&mut self) {
        Trace::write("tick");
        self.counter = self.counter + 1;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("cyclic literal calls publish the portable Terminal artifact");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(
        entry.ranked_scc.is_none(),
        "no manufactured termination certificate"
    );
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("persistent receiver survives publication")
    };
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    let execute = || {
        TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 1,
                    structural_type: receiver.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
        )
        .expect("canonical artifact reloads and independently verifies")
    };
    let expected = [
        "first", "", "tick", "later", "λ", "tick", "later", "λ", "tick", "done",
    ]
    .map(|text| text.as_bytes().to_vec())
    .to_vec();
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    let mut execution = execute();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(execution.resume(&mut meter, &mut trace).unwrap(), complete);
    assert_eq!(trace.0, expected);
    let total = meter.usage().total_units();
    for allowance in 0..total {
        let mut execution = execute();
        let mut meter = TerminalFuelMeter::with_allowance(allowance);
        let mut trace = ByteTrace::default();
        assert!(matches!(
            execution.resume(&mut meter, &mut trace).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        let prefix = trace.0.clone();
        let usage = meter.usage().clone();
        for _ in 0..3 {
            assert!(matches!(
                execution.resume(&mut meter, &mut trace).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert_eq!(
                trace.0, prefix,
                "zero-fuel resume repeats no effect at {allowance}"
            );
            assert_eq!(meter.usage(), &usage);
        }
        meter.replenish(total - allowance).unwrap();
        assert_eq!(execution.resume(&mut meter, &mut trace).unwrap(), complete);
        assert_eq!(
            trace.0, expected,
            "exact selected contents/order at fuel split {allowance}"
        );
        assert_eq!(meter.usage().total_units(), total);
    }
}

#[derive(Default)]
struct ByteTrace(Vec<Vec<u8>>);

impl TerminalEffectHandler for ByteTrace {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            panic!("only authored byte trace boundaries are observable")
        };
        assert!(arguments.is_empty());
        assert_eq!(structural_arguments.len(), 1);
        let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
            panic!("boundary retains exact immutable byte contents")
        };
        self.0.push(bytes.clone());
        Ok(())
    }
}

/// `self.place` is written once before the cycle and only read inside it, so
/// the divide's `1 <= self.place` discharges through a retained cyclic
/// invariant naming the field itself (`IntegerField` over the machine's own
/// receiver). The verifier independently establishes and preserves the
/// predicate at every actual arrival; the producer only proposed it.
#[test]
fn cyclic_field_divisor_retains_storage_observation_invariant() {
    let checked = checked_source(
        r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { counter: u64 in Wrapping; place: u32 in Wrapping; sq: u32 in Wrapping; d: u32 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 0;
    self.sq = 81;
    self.place = 100;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> digit() _ -> done() }
    }
    state digit(&mut self) {
        self.d = self.sq / self.place;
        self.counter = self.counter + 1;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("storage-observation invariant proves");
    let invariants = &lowered.semantic_module.scalar_block_invariants;
    assert!(
        invariants.iter().any(|invariant| {
            let predicate = match &invariant.predicate {
                semantic_vocabulary::Proposition::Implication { conclusion, .. } => conclusion,
                predicate => predicate,
            };
            matches!(
                predicate,
                semantic_vocabulary::Proposition::LessOrEqual(
                    _,
                    semantic_vocabulary::ScalarTerm::IntegerField { .. },
                )
            )
        }),
        "a retained invariant names the field bound: {invariants:?}"
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("cyclic field divisor publishes the portable Terminal artifact");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("persistent receiver survives publication")
    };
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("canonical artifact reloads and independently verifies");
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut trace).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(trace.0, [b"done".to_vec()]);
}

/// `self.place` is divided by ten every iteration, so the divide's
/// nonzero-divisor obligation `1 <= self.place` cannot be discharged by the
/// plain bound `counter < 3 -> place >= 1`: the last in-guard iteration
/// already divided `place` once more. The retained invariant is the lockstep
/// family `counter < k -> 10^(3-k) <= place`, each clause proved at the entry
/// and backedge arrivals — the vacuous `counter < 1` clause through a checked
/// integer contradiction — and the artifact still reloads, verifies, and
/// executes to `done` (TASKS.md GENERAL-CYCLIC-EXECUTION).
#[test]
fn cyclic_field_divisor_retains_lockstep_invariant() {
    let checked = checked_source(
        r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { counter: u64 in Wrapping; place: u32 in Wrapping; sq: u32 in Wrapping; d: u32 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 0;
    self.sq = 81;
    self.place = 100;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> digit() _ -> done() }
    }
    state digit(&mut self) {
        self.d = self.sq / self.place;
        self.place = self.place / 10;
        self.counter = self.counter + 1;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("the lockstep invariant proves every arrival");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let invariant = module
        .scalar_block_invariants
        .iter()
        .find(|invariant| {
            matches!(
                &invariant.predicate,
                semantic_vocabulary::Proposition::Conjunction(members)
                    if members.iter().any(|member| matches!(
                        member,
                        semantic_vocabulary::Proposition::Implication {
                            conclusion,
                            ..
                        } if matches!(
                            conclusion.as_ref(),
                            semantic_vocabulary::Proposition::LessOrEqual(
                                semantic_vocabulary::ScalarTerm::Integer {
                                    value: semantic_vocabulary::IntegerValue::Unsigned(100),
                                    ..
                                },
                                semantic_vocabulary::ScalarTerm::IntegerField { .. },
                            )
                        )
                    ))
            )
        })
        .expect("the lockstep family retains the strongest clause");
    assert_eq!(invariant.arrivals.len(), 2, "entry and backedge arrivals");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("persistent receiver survives publication")
    };
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("canonical artifact reloads and independently verifies");
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut trace).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(trace.0, [b"done".to_vec()]);
}

/// The `print_squares` `digit_div` shape keeps the guarded counter as a
/// plain `u64` — its increment is an exact add, not a wrapping one — and
/// writes the divisor and counter in a later block than the divide. The
/// same lockstep family `p < k -> 10^(3-k) <= place` must still prove the
/// backedge arrivals before the artifact publishes and runs to `done`.
#[test]
fn cyclic_field_divisor_lockstep_accepts_exact_counter_and_split_updates() {
    let checked = checked_source(
        r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { p: u64; place: u32 in Wrapping; sq: u32 in Wrapping; d: u32 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.p = 0;
    self.sq = 81;
    self.place = 100;
    transition { _ -> itoa_loop() }
    state itoa_loop(&mut self) {
        transition self.p < 3 { true -> digit_div() _ -> done() }
    }
    state digit_div(&mut self) {
        self.d = self.sq / self.place;
        transition { _ -> digit_write() }
    }
    state digit_write(&mut self) {
        self.place = self.place / 10;
        self.p = self.p + 1;
        transition { _ -> itoa_loop() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("the lockstep invariant proves the split-update cycle");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let invariant = module
        .scalar_block_invariants
        .iter()
        .find(|invariant| {
            matches!(
                &invariant.predicate,
                semantic_vocabulary::Proposition::Conjunction(members)
                    if members.iter().any(|member| matches!(
                        member,
                        semantic_vocabulary::Proposition::Implication {
                            conclusion,
                            ..
                        } if matches!(
                            conclusion.as_ref(),
                            semantic_vocabulary::Proposition::LessOrEqual(
                                semantic_vocabulary::ScalarTerm::Integer {
                                    value: semantic_vocabulary::IntegerValue::Unsigned(100),
                                    ..
                                },
                                semantic_vocabulary::ScalarTerm::IntegerField { .. },
                            )
                        )
                    ))
            )
        })
        .expect("the lockstep family retains the strongest clause");
    assert_eq!(invariant.arrivals.len(), 2, "entry and backedge arrivals");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let receiver = &entry.structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("split-update artifact reloads and independently verifies");
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut trace).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(trace.0, [b"done".to_vec()]);
}

/// A lockstep candidate that under-provisions its divisor is a proposal the
/// arrival check drops: `place = 4` reaches zero before `counter < 3` exits,
/// so neither the family nor the original guarded bound can prove, and the
/// divide keeps its checked nonzero obligation.
#[test]
fn cyclic_field_divisor_rejects_reachable_zero_divisor() {
    let checked = checked_source(
        r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { counter: u64 in Wrapping; place: u32 in Wrapping; sq: u32 in Wrapping; d: u32 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 0;
    self.sq = 81;
    self.place = 4;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> digit() _ -> done() }
    }
    state digit(&mut self) {
        self.d = self.sq / self.place;
        self.place = self.place / 10;
        self.counter = self.counter + 1;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#,
    );
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("Main::main")),
        Err(LoweringError::OperationProofUnavailable(_))
    ));
}

/// The lockstep certificate binds each clause to its exact cited evidence;
/// widening the loop guard after production makes the retained family stale,
/// and independent verification must refuse it even though the changed module
/// still lowers structurally.
#[test]
fn cyclic_field_divisor_lockstep_evidence_rejects_a_widened_guard() {
    let checked = checked_source(
        r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { counter: u64 in Wrapping; place: u32 in Wrapping; sq: u32 in Wrapping; d: u32 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 0;
    self.sq = 81;
    self.place = 100;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> digit() _ -> done() }
    }
    state digit(&mut self) {
        self.d = self.sq / self.place;
        self.place = self.place / 10;
        self.counter = self.counter + 1;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("the lockstep invariant proves every arrival");
    let mut widened = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let guard = widened
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Unsigned(3),
                }
            )
        })
        .expect("the loop guard carries its bound literal");
    guard.kind = terminal_psi::OperationKind::IntegerConstant {
        value: semantic_vocabulary::IntegerValue::Unsigned(4),
    };
    terminal_verifier::validate_module(&widened)
        .expect("the widened guard keeps structural validity");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert!(
        terminal_verifier::verify_module(
            &widened,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "the retained family cannot discharge a fourth iteration's divide"
    );
}

/// The guarded exit pins `counter` to an exact stored bound (`self.counter =
/// 9`), so the loop guard `counter < 3` is already contradicted on the
/// retiring arrival. The lockstep invariant `counter < 3 -> 1 <= divisor`
/// discharges there through the equality's weakened order legs — no order
/// citation ever exists for the stored bound — while the live step arrivals
/// still discharge the conclusion through the saved divisor equality.
#[test]
fn integer_guarded_exit_retains_equality_bound_invariant() {
    let source = r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { counter: u32 in Wrapping; divisor: u32 in Wrapping; result: u32 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 0;
    self.divisor = 5;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> step() _ -> done() }
    }
    state step(&mut self) {
        self.result = 100 / self.divisor;
        Trace::write("step");
        self.counter = self.counter + 1;
        transition self.counter < 3 { true -> head() _ -> retire() }
    }
    state retire(&mut self) {
        self.counter = 9;
        self.divisor = 0;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#;
    let checked = checked_source(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("the stored bound retires the guarded exit arrival");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(
        module
            .scalar_block_invariants
            .iter()
            .any(|invariant| matches!(
                invariant.predicate,
                semantic_vocabulary::Proposition::Implication { .. }
            )),
        "the divisor is not unconditionally nonzero at this header"
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let receiver = &entry.structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("equality-bound guarded exit reloads and independently verifies");
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut trace).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        trace.0,
        [
            b"step".to_vec(),
            b"step".to_vec(),
            b"step".to_vec(),
            b"done".to_vec()
        ]
    );
    let live = checked_source(&source.replace("self.counter = 9;", "self.counter = 2;"));
    assert!(
        terminal_production::TerminalProductionRequest::new(&live, "Main::main")
            .produce_artifact()
            .is_err(),
        "a retiring bound inside the guard keeps the zero divisor live"
    );
    let mut stale = module.clone();
    let bound = stale
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Unsigned(9)
                }
            )
        })
        .expect("the exit pins its stored bound");
    bound.kind = terminal_psi::OperationKind::IntegerConstant {
        value: semantic_vocabulary::IntegerValue::Unsigned(2),
    };
    terminal_verifier::validate_module(&stale)
        .expect("changing the bound keeps structural validity");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert!(
        terminal_verifier::verify_module(
            &stale,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a bound inside the guard cannot reuse the retired arrival's proof"
    );
}

#[test]
fn guarded_field_divisor_remains_valid_until_loop_exit() {
    let source = r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { ready: bool; counter: u32 in Wrapping; divisor: u32; result: u32; }
machine Main::main(&mut self) reaches Trace {
    self.ready = true;
    self.counter = 0;
    self.divisor = 5;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.ready { true -> step() _ -> done() }
    }
    state step(&mut self) {
        self.result = 100 / self.divisor;
        Trace::write("step");
        self.counter = self.counter + 1;
        transition self.counter < 3 { true -> head() _ -> retire() }
    }
    state retire(&mut self) {
        self.divisor = 0;
        self.ready = false;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#;
    let checked = checked_source(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("a guarded field bound is inductive across the exit backedge");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(
        module
            .scalar_block_invariants
            .iter()
            .any(|invariant| matches!(
                invariant.predicate,
                semantic_vocabulary::Proposition::Implication { .. }
            )),
        "the divisor is not unconditionally nonzero at this header"
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let receiver = &entry.structural_parameters[0];
    let ready_field = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            terminal_psi::OperationKind::BooleanStructuralField { field, .. } => Some(field),
            _ => None,
        })
        .expect("the loop reads its ready field");
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            boolean_fields: &[terminal_interpreter::TerminalStructuralBooleanFieldValue {
                argument_index: 0,
                path: Vec::new(),
                field: ready_field,
                value: false,
            }],
            ..Default::default()
        },
    )
    .expect("guarded loop reloads with every arrival independently checked");
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut trace).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        trace.0,
        [
            b"step".to_vec(),
            b"step".to_vec(),
            b"step".to_vec(),
            b"done".to_vec()
        ]
    );
    let invalid = checked_source(&source.replace("self.ready = false;", "self.ready = true;"));
    assert!(
        terminal_production::TerminalProductionRequest::new(&invalid, "Main::main")
            .produce_artifact()
            .is_err(),
        "a backedge with a live guard and zero divisor must reject"
    );
    let mut live_exit = module.clone();
    let exit_guard = live_exit
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanConstant { value: false }
            )
        })
        .expect("the exit clears its guard");
    exit_guard.kind = terminal_psi::OperationKind::BooleanConstant { value: true };
    terminal_verifier::validate_module(&live_exit)
        .expect("changing the guard keeps structural validity");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert!(
        terminal_verifier::verify_module(
            &live_exit,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "old arrival evidence cannot justify the newly live exit"
    );
}

#[test]
fn saved_field_value_keeps_its_proof_after_overwriting_the_field() {
    let source = r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { divisor: u32; result: u32; }
machine Main::main(&mut self) reaches Trace {
    self.divisor = 5;
    self.consume();
    transition self.result == 20 { true -> passed() _ -> failed() }
    state passed(&mut self) { Trace::write("saved"); }
    state failed(&mut self) { Trace::write("wrong"); }
}
machine Main::consume(&mut self) {
    transition self.divisor > 0 { true -> positive() _ -> done() }
    state positive(&mut self) {
        let saved: u32 = self.divisor;
        self.divisor = 0;
        self.result = 100 / saved;
    }
    state done(&mut self) { }
}
machine Main::clear(&mut self) { self.divisor = 0; }
"#;
    for mutating_call in [false, true] {
        let source = if mutating_call {
            source.replacen("        self.divisor = 0;", "        self.clear();", 1)
        } else {
            source.to_owned()
        };
        let checked = checked_source(&source);
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
            .produce_artifact()
            .expect("a copied field value retains its pre-write nonzero proof");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let receiver = &entry.structural_parameters[0];
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 1,
                    structural_type: receiver.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
        )
        .expect("saved-value proof independently reloads");
        let mut meter = TerminalFuelMeter::with_allowance(1000);
        let mut trace = ByteTrace::default();
        assert_eq!(
            execution.resume(&mut meter, &mut trace).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(trace.0, [b"saved".to_vec()]);
        // Moving the saved read after the write/call leaves a structurally
        // valid program, but its old nonzero certificate must no longer check.
        let mut late_read = module.clone();
        let block = late_read
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .find(|block| {
                block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::ExactIntegerDivide { .. }
                    )
                })
            })
            .unwrap();
        let right = block
            .operations
            .iter()
            .find_map(|operation| match operation.kind {
                terminal_psi::OperationKind::ExactIntegerDivide { right, .. } => Some(right),
                _ => None,
            })
            .unwrap();
        let read = block
            .operations
            .iter()
            .position(|operation| {
                operation
                    .result
                    .scalar_ref()
                    .is_some_and(|result| result.id == right)
            })
            .unwrap();
        assert!(matches!(
            block.operations[read].kind,
            terminal_psi::OperationKind::IntegerStructuralField { .. }
        ));
        let captured = block.operations.remove(read);
        let division = block
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::ExactIntegerDivide { .. }
                )
            })
            .unwrap();
        block.operations.insert(division, captured);
        terminal_verifier::validate_module(&late_read)
            .expect("late read still has valid types and dominance");
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        assert!(
            terminal_verifier::verify_module(
                &late_read,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "the saved-value certificate cannot justify a post-mutation read"
        );
    }
    let reread = source.replace("100 / saved", "100 / self.divisor");
    let tokens = super::Lexer::new(&reread).tokenize().unwrap();
    let syntax = super::parse_syntax_trees(&tokens).unwrap();
    let resolved = super::resolve(super::ResolutionRequest::new(&syntax)).unwrap();
    let typed = super::lower_symbol_resolved_trees(&resolved).unwrap();
    let Err(diagnostics) = super::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    ) else {
        panic!("a new read cannot inherit the saved value's bound");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("division by zero"))
    );
}

#[test]
fn incompatible_integer_guards_keep_the_dead_operation_checked() {
    let source = r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
data Main { counter: u32; divisor: u32; result: u32; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 1;
    self.divisor = 0;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> low() _ -> done() }
    }
    state low(&mut self) {
        transition self.counter >= 3 { true -> impossible() _ -> done() }
    }
    state impossible(&mut self) { self.result = 100 / self.divisor; }
    state done(&mut self) { Trace::write("done"); }
}
"#;
    let checked = checked_source(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("incompatible guards prove the dead operation's original obligation");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ExactIntegerDivide { .. }
            )),
        "retain and prove the dead division rather than silently pruning its obligation"
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("receiver")
    };
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut trace).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(trace.0, [b"done".to_vec()]);
    let possible = checked_source(&source.replace("self.counter >= 3", "self.counter >= 1"));
    assert!(
        terminal_production::TerminalProductionRequest::new(&possible, "Main::main")
            .produce_artifact()
            .is_err(),
        "a reachable zero divisor remains rejected"
    );
}
