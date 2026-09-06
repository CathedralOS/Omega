use super::later_results::{SCALAR_HELPERS, encoded_locals};
use super::*;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_psi::{BoundaryMachineResult, OperationKind, OperationResult, Terminator};

pub(super) fn source(completion: &str) -> String {
    let host = if completion.contains("Host::finish") {
        " + Host"
    } else {
        ""
    };
    format!(
        r#"
        {SCALAR_HELPERS}
        pub data Token {{ flag: bool; }}
        boundary trait Factory {{
            machine create(first: u16, second: u16) -> Token reaches Factory;
        }}
        boundary trait Sink {{
            machine consume(token: Token, count: u16) reaches Sink;
            machine measure(token: Token, first: u16, second: u16) -> u16 reaches Sink;
        }}
        machine forward(token: Token, count: u16) -> Token {{ token }}
        data Main {{}}
        machine Main::consume(token: Token, count: u16) reaches Sink {{
            Sink::consume(token, count);
        }}
        machine Main::measure(token: Token, count: u16) -> u16 reaches Sink {{
            let result: u16 = Sink::measure(token, count, 17u16);
            result
        }}
        machine Main::main() reaches Factory + Sink{host} {{
            let prefix: u16 = 5u16;
            let first: Token = Factory::create(identity16(prefix), 7u16);
            let spare: Token = Factory::create(11u16, 13u16);
            {completion}
        }}
        "#
    )
}

#[derive(Default)]
pub(super) struct ObserveMoves {
    pub(super) results: ObserveResults,
    pub(super) produced: Vec<u64>,
    pub(super) consumed: Vec<u64>,
}

impl TerminalEffectHandler for ObserveMoves {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("result-bearing boundary handler required")
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            structural_arguments,
            ..
        } = effect
        else {
            panic!("boundary effect")
        };
        self.consumed.extend(
            structural_arguments
                .iter()
                .map(|value| value.opaque_identity),
        );
        let mut result = self.results.handle_effect_result(effect)?;
        if let TerminalEffectResult::Structural(value) = &mut result {
            value.opaque_identity += self.produced.len() as u64;
            self.produced.push(value.opaque_identity);
        }
        Ok(result)
    }
}

#[test]
fn ordinary_and_direct_boundary_consumers_transfer_the_exact_result_once() {
    for (completion, names, expected_calls) in [
        (
            "Main::consume(first, identity16(prefix));",
            vec!["prefix", "first", "spare"],
            vec![vec![unsigned(16, 5)]],
        ),
        (
            "let measured: u16 = Main::measure(first, identity16(prefix)); Host::finish(measured);",
            vec!["prefix", "first", "spare", "measured"],
            vec![
                vec![unsigned(16, 5), unsigned(16, 17)],
                vec![unsigned(16, 17)],
            ],
        ),
        (
            "let moved: Token = forward(first, identity16(prefix)); Main::consume(moved, identity16(prefix));",
            vec!["prefix", "first", "spare", "moved"],
            vec![vec![unsigned(16, 5)]],
        ),
        (
            "Sink::consume(first, identity16(prefix));",
            vec!["prefix", "first", "spare"],
            vec![vec![unsigned(16, 5)]],
        ),
        (
            "let measured: u16 = Sink::measure(first, identity16(prefix), 17u16); Host::finish(measured);",
            vec!["prefix", "first", "spare", "measured"],
            vec![
                vec![unsigned(16, 5), unsigned(16, 17)],
                vec![unsigned(16, 17)],
            ],
        ),
    ] {
        let checked = checked(&source(completion));
        let artifact = encoded_locals(&checked, &names);
        let published =
            terminal_production::produce_terminal_artifact(&checked, "Main::main").unwrap();
        assert_eq!(
            decode_module(published.semantic_bytes()).unwrap(),
            decode_module(&artifact.0).unwrap()
        );
        let module = decode_module(&artifact.0).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let boundary_results = entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match (&operation.kind, &operation.result) {
                (OperationKind::BoundaryCall { .. }, OperationResult::Structural(result)) => {
                    Some(result.place)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(boundary_results.len(), 2);
        let cleanup = entry
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => Some(trivial_affine_discards),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            cleanup,
            &[boundary_results[1]],
            "only the unconsumed boundary result is cleaned"
        );
        let expected = [
            vec![unsigned(16, 5), unsigned(16, 7)],
            vec![unsigned(16, 11), unsigned(16, 13)],
        ]
        .into_iter()
        .chain(expected_calls)
        .collect::<Vec<_>>();
        let mut reference = None;
        for incremental in [false, true] {
            let mut execution = TerminalExecution::start_artifact(
                &artifact.0,
                &artifact.1,
                &AdmissionProfile::default(),
                &[],
            )
            .unwrap();
            let mut observer = ObserveMoves::default();
            let mut fuel = if incremental {
                TerminalFuelMeter::with_allowance(0)
            } else {
                TerminalFuelMeter::unbounded()
            };
            let mut complete = false;
            for _ in 0..1024 {
                match execution
                    .resume_with_effect_handler(&mut fuel, &mut observer)
                    .unwrap()
                {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        assert!(incremental);
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(result) => {
                        assert_eq!(result, TerminalExecutionResult::Unit);
                        complete = true;
                        break;
                    }
                    status => panic!("unexpected status: {status:?}"),
                }
            }
            assert!(complete);
            assert_eq!(observer.results.calls, expected);
            assert_eq!(observer.produced, [700, 701]);
            assert_eq!(observer.consumed, [700]);
            assert!(execution.live_affine_frontier().next().is_none());
            if let Some(reference) = &reference {
                assert_eq!(execution.effects(), reference);
            } else {
                reference = Some(execution.effects().to_vec());
            }
        }
    }
}

#[test]
fn rejected_boundary_results_do_not_establish_or_transfer_before_retry() {
    let artifact = encoded_locals(
        &checked(&source("Main::consume(first, identity16(prefix));")),
        &["prefix", "first", "spare"],
    );
    let mut execution = TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut observer = ObserveMoves::default();
    observer.results.structural_response = StructuralResponse::Rejected;
    let mut fuel = TerminalFuelMeter::unbounded();
    assert!(matches!(
        execution.resume_with_effect_handler(&mut fuel, &mut observer),
        Err(TerminalInterpretError::EffectRejected { .. })
    ));
    assert!(execution.live_affine_frontier().next().is_none());
    assert!(observer.produced.is_empty());
    assert!(observer.consumed.is_empty());
    observer.results.structural_response = StructuralResponse::Correct;
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut fuel, &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.produced, [700, 701]);
    assert_eq!(observer.consumed, [700]);
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(
        execution
            .effects()
            .iter()
            .filter(|effect| matches!(
                effect,
                TerminalEffect::BoundaryCall {
                    result: BoundaryMachineResult::Structural(_),
                    ..
                }
            ))
            .count(),
        2
    );
}

#[test]
fn a_crashing_consumer_operand_never_transfers_or_cleans_boundary_results() {
    for (consumer, operand, abort_ordinal) in [
        ("Main::consume", "first", 1),
        ("Sink::consume", "first", 1),
        ("Sink::consume", "forward(first, prefix)", 2),
    ] {
        let source = format!(
            "machine abort() -> u16 crashes Abort {{ crash Abort; }}\n{}",
            source(&format!("{consumer}({operand}, abort());")).replace(
                "reaches Factory + Sink {",
                "reaches Factory + Sink crashes Abort {"
            )
        );
        let checked = checked(&source);
        let artifact = encoded_locals(&checked, &["prefix", "first", "spare"]);
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").unwrap();
        let state = checked.machine_states(main_machine(&checked))[0].symbol;
        let operand = lowered
            .source_call_occurrences
            .iter()
            .find(|occurrence| {
                occurrence.source_state == state
                    && occurrence.statement_index == 3
                    && occurrence.call_ordinal == abort_ordinal
            })
            .unwrap()
            .terminal_operation;
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let mut execution = TerminalExecution::start_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let mut observer = ObserveMoves::default();
        let mut fuel = TerminalFuelMeter::with_allowance(0);
        let mut reached_operand = false;
        for _ in 0..1024 {
            let TerminalExecutionStatus::SponsorExhausted(exhaustion) = execution
                .resume_with_effect_handler(&mut fuel, &mut observer)
                .unwrap()
            else {
                panic!("expected pause before crashing operand")
            };
            if exhaustion.site == terminal_fuel::FuelChargeSite::Operation(operand) {
                reached_operand = true;
                break;
            }
            fuel.replenish(1).unwrap();
        }
        assert!(reached_operand);
        assert_eq!(execution.live_affine_frontier().count(), 2);
        fuel.replenish(1024).unwrap();
        let status = execution
            .resume_with_effect_handler(&mut fuel, &mut observer)
            .unwrap();
        assert!(
            matches!(&status, TerminalExecutionStatus::Crashed(crash) if crash.cause == terminal_psi::CrashCause::Abort)
        );
        assert_eq!(observer.produced, [700, 701]);
        assert!(observer.consumed.is_empty());
        for block in &entry.blocks {
            if let Terminator::ReturnUnit { edge, .. } = block.terminator {
                assert!(
                    fuel.usage()
                        .at(terminal_fuel::FuelChargeSite::Edge(edge))
                        .is_none(),
                    "crash has no cleanup successor"
                );
            }
        }
        let effects = execution.effects().to_vec();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut fuel, &mut observer)
                .unwrap(),
            status
        );
        assert_eq!(execution.effects(), effects);
    }
}

#[test]
fn independent_verification_rejects_boundary_result_transfer_and_cleanup_forgery() {
    let artifact = encoded_locals(
        &checked(&source("Main::consume(first, identity16(prefix));")),
        &["prefix", "first", "spare"],
    );
    let original = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    for mutation in 0..4 {
        let mut module = original.clone();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let transferred = entry
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut operation.kind
                else {
                    return None;
                };
                let [argument] = structural_arguments.as_mut_slice() else {
                    return None;
                };
                match mutation {
                    0 => argument.access = terminal_psi::StructuralAccess::SharedBorrow,
                    1 => argument
                        .path
                        .push(terminal_psi::StructuralPathSegment::Field("flag".into())),
                    _ => {}
                }
                Some(argument.place)
            })
            .unwrap();
        if mutation == 2 {
            for block in &mut entry.blocks {
                if let Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } = &mut block.terminator
                {
                    trivial_affine_discards.push(transferred);
                }
            }
        } else if mutation == 3 {
            let result = entry
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match &mut operation.result {
                    OperationResult::Structural(result) if result.place == transferred => {
                        Some(result)
                    }
                    _ => None,
                })
                .unwrap();
            result.structural_type = semantic_vocabulary::StructuralTypeId::new(99).unwrap();
        }
        assert!(
            terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                .is_err(),
            "forged transfer or result mutation {mutation}"
        );
    }
}
