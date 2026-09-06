use super::boundary_result_moves::{ObserveMoves, source};
use super::later_results::encoded_locals;
use super::*;
use std::collections::BTreeSet;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_psi::{BoundaryMachineResult, OperationKind, OperationResult, Terminator};

fn observed_source(completion: &str) -> String {
    let mut source = source(completion).replace(
        "reaches Factory + Sink",
        "reaches Factory + Sink + Producer",
    );
    source.push_str("machine Main::probe(value: u16) -> u16 reaches Producer { let result: u16 = Producer::choose(value, value, value); result }");
    source
}

fn start(artifact: &(Vec<u8>, Vec<u8>)) -> TerminalExecution {
    TerminalExecution::start_artifact(&artifact.0, &artifact.1, &AdmissionProfile::default(), &[])
        .unwrap()
}

fn assert_completion(
    artifact: &(Vec<u8>, Vec<u8>),
    expected: &[Vec<u128>],
    produced: &[u64],
    consumed: &[u64],
) {
    let expected = expected
        .iter()
        .map(|arguments| {
            arguments
                .iter()
                .map(|value| unsigned(16, *value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution = start(artifact);
        let mut observer = ObserveMoves::default();
        let mut fuel = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        for _ in 0..2048 {
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
        assert_eq!(observer.produced, produced);
        assert_eq!(observer.consumed, consumed);
        assert!(execution.live_affine_frontier().next().is_none());
        if let Some(reference) = &reference {
            assert_eq!(execution.effects(), reference);
        } else {
            reference = Some(execution.effects().to_vec());
        }
    }
}

#[test]
fn boundary_temporaries_supply_existing_ordinary_and_boundary_result_carriers() {
    for (completion, names, calls, produced, consumed) in [
        (
            "Main::consume(Factory::create(prefix, 19u16), prefix);",
            vec!["prefix", "first", "spare"],
            vec![vec![5]],
            vec![700, 701, 702],
            vec![702],
        ),
        (
            "let measured: u16 = Main::measure(Factory::create(prefix, 19u16), prefix); Host::finish(measured);",
            vec!["prefix", "first", "spare", "measured"],
            vec![vec![5, 17], vec![17]],
            vec![700, 701, 702],
            vec![702],
        ),
        (
            "let moved: Token = forward(Factory::create(prefix, 19u16), prefix); Sink::consume(moved, prefix);",
            vec!["prefix", "first", "spare", "moved"],
            vec![vec![5]],
            vec![700, 701, 702],
            vec![702],
        ),
        (
            "Sink::consume(Factory::create(prefix, 19u16), prefix);",
            vec!["prefix", "first", "spare"],
            vec![vec![5]],
            vec![700, 701, 702],
            vec![702],
        ),
        (
            "let measured: u16 = Sink::measure(Factory::create(prefix, 19u16), prefix, 23u16); Host::finish(measured);",
            vec!["prefix", "first", "spare", "measured"],
            vec![vec![5, 23], vec![23]],
            vec![700, 701, 702],
            vec![702],
        ),
        (
            "let replacement: Token = Sink::replace(Factory::create(prefix, 19u16), prefix, 23u16); Sink::consume(replacement, prefix);",
            vec!["prefix", "first", "spare", "replacement"],
            vec![vec![5, 23], vec![5]],
            vec![700, 701, 702, 703],
            vec![702, 703],
        ),
        (
            "Sink::consume(Sink::replace(forward(Factory::create(prefix, 19u16), prefix), prefix, 23u16), prefix);",
            vec!["prefix", "first", "spare"],
            vec![vec![5, 23], vec![5]],
            vec![700, 701, 702, 703],
            vec![702, 703],
        ),
    ] {
        let source = source(completion).replace("boundary trait Sink {", "boundary trait Sink { machine replace(token: Token, first: u16, second: u16) -> Token reaches Sink;");
        let checked = checked(&source);
        let artifact = encoded_locals(&checked, &names);
        let published =
            terminal_production::produce_terminal_artifact(&checked, "Main::main").unwrap();
        assert_eq!(
            decode_module(published.semantic_bytes()).unwrap(),
            decode_module(&artifact.0).unwrap()
        );
        let expected = [vec![5, 7], vec![11, 13], vec![5, 19]]
            .into_iter()
            .chain(calls)
            .collect::<Vec<_>>();
        assert_completion(&artifact, &expected, &produced, &consumed);
    }
}

#[test]
fn boundary_temporary_schedule_preserves_prefix_result_slots_ids_and_residual_cleanup() {
    for producer in ["static", "bodyless", "nominal"] {
        let mut source = observed_source("let measured: u16 = Sink::take(Main::probe(prefix), forward(Factory::create(Main::probe(11u16), Main::probe(22u16)), Main::probe(33u16)), Main::probe(44u16)); Host::finish(measured); Host::finish(prefix);")
            .replace("boundary trait Sink {", "boundary trait Sink { machine take(first: u16, token: Token, last: u16) -> u16 reaches Sink;");
        if producer == "bodyless" {
            source = source.replace("Factory::create(", "Maker::create(");
            source.push_str("pub data Maker {} boundary machine Maker::create(first: u16, second: u16) -> Token reaches Factory ensures true;");
        } else if producer == "nominal" {
            source = source.replace("machine Main::main()", "machine Main::main<machine Create>() where machine Create satisfies Factory::create;")
                .replace("Factory::create(", "Create(");
        }
        let checked = checked(&source);
        let artifact = encoded_locals(&checked, &["prefix", "first", "spare", "measured"]);
        let published =
            terminal_production::produce_terminal_artifact(&checked, "Main::main").unwrap();
        let module = decode_module(&artifact.0).unwrap();
        assert_eq!(decode_module(published.semantic_bytes()).unwrap(), module);
        let mut machines = BTreeSet::new();
        let mut blocks = BTreeSet::new();
        let mut operations = BTreeSet::new();
        for machine in &module.machines {
            assert!(machines.insert(machine.id));
            let mut places = BTreeSet::new();
            for place in &machine.structural_places {
                assert!(places.insert(place.id));
            }
            for block in &machine.blocks {
                assert!(blocks.insert(block.id));
                for operation in &block.operations {
                    assert!(operations.insert(operation.id));
                }
            }
        }
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let produced = entry
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
        assert_eq!(produced.len(), 3);
        for block in &entry.blocks {
            if let Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } = &block.terminator
            {
                assert_eq!(
                    trivial_affine_discards,
                    &[produced[1], produced[0]],
                    "only the earlier unconsumed results retain reverse cleanup: {producer}"
                );
            }
        }
        assert_completion(
            &artifact,
            &[
                vec![5, 7],
                vec![11, 13],
                vec![5, 5, 5],
                vec![11, 11, 11],
                vec![22, 22, 22],
                vec![11, 22],
                vec![33, 33, 33],
                vec![44, 44, 44],
                vec![5, 44],
                vec![44],
                vec![5],
            ],
            &[700, 701, 702],
            &[702],
        );
    }
}

#[test]
fn refused_temporary_production_retries_without_replaying_paid_scalar_operands() {
    let source = observed_source(
        "Sink::consume(Factory::create(Main::probe(19u16), Main::probe(23u16)), Main::probe(29u16));",
    );
    let artifact = encoded_locals(&checked(&source), &["prefix", "first", "spare"]);
    struct RefuseTemporary {
        observer: ObserveMoves,
        refuse: bool,
    }
    impl TerminalEffectHandler for RefuseTemporary {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("result handler")
        }
        fn handle_effect_result(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            if self.refuse
                && self.observer.produced.len() == 2
                && matches!(
                    effect,
                    TerminalEffect::BoundaryCall {
                        result: BoundaryMachineResult::Structural(_),
                        ..
                    }
                )
            {
                return Err(TerminalEffectRejection {
                    reason: "temporary not available yet".into(),
                });
            }
            self.observer.handle_effect_result(effect)
        }
    }
    let mut execution = start(&artifact);
    let mut observer = RefuseTemporary {
        observer: ObserveMoves::default(),
        refuse: true,
    };
    let mut fuel = TerminalFuelMeter::unbounded();
    assert!(matches!(
        execution.resume_with_effect_handler(&mut fuel, &mut observer),
        Err(TerminalInterpretError::EffectRejected { .. })
    ));
    assert_eq!(observer.observer.produced, [700, 701]);
    assert!(observer.observer.consumed.is_empty());
    assert_eq!(execution.live_affine_frontier().count(), 2);
    assert_eq!(
        observer.observer.results.calls,
        [vec![5, 7], vec![11, 13], vec![19, 19, 19], vec![23, 23, 23]].map(|arguments| arguments
            .into_iter()
            .map(|value| unsigned(16, value))
            .collect::<Vec<_>>())
    );
    let paid = execution.effects().to_vec();
    observer.refuse = false;
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut fuel, &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.effects().starts_with(&paid));
    assert_eq!(observer.observer.results.calls.len(), 7);
    assert_eq!(
        observer.observer.results.calls[4],
        [unsigned(16, 19), unsigned(16, 23)]
    );
    assert_eq!(observer.observer.results.calls[5], [unsigned(16, 29); 3]);
    assert_eq!(observer.observer.results.calls[6], [unsigned(16, 29)]);
    assert_eq!(observer.observer.produced, [700, 701, 702]);
    assert_eq!(observer.observer.consumed, [702]);
    assert!(execution.live_affine_frontier().next().is_none());
}

#[test]
fn crash_after_boundary_temporary_preserves_production_without_cleanup() {
    let source = format!(
        "machine abort() -> u16 crashes Abort {{ crash Abort; }}\n{}",
        source("Sink::consume(Factory::create(prefix, 19u16), abort());").replace(
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
                && occurrence.call_ordinal == 2
        })
        .unwrap()
        .terminal_operation;
    let mut execution = start(&artifact);
    let mut observer = ObserveMoves::default();
    let mut fuel = TerminalFuelMeter::with_allowance(0);
    let mut reached = false;
    for _ in 0..1024 {
        let TerminalExecutionStatus::SponsorExhausted(exhaustion) = execution
            .resume_with_effect_handler(&mut fuel, &mut observer)
            .unwrap()
        else {
            panic!("pause before crashing argument")
        };
        if exhaustion.site == terminal_fuel::FuelChargeSite::Operation(operand) {
            reached = true;
            break;
        }
        fuel.replenish(1).unwrap();
    }
    assert!(reached);
    assert_eq!(execution.live_affine_frontier().count(), 3);
    assert_eq!(observer.produced, [700, 701, 702]);
    assert!(observer.consumed.is_empty());
    let effects = execution.effects().to_vec();
    fuel.replenish(2048).unwrap();
    let status = execution
        .resume_with_effect_handler(&mut fuel, &mut observer)
        .unwrap();
    assert!(
        matches!(&status, TerminalExecutionStatus::Crashed(crash) if crash.cause == terminal_psi::CrashCause::Abort)
    );
    assert_eq!(execution.effects(), effects);
    for machine in &lowered.semantic_module.machines {
        for block in &machine.blocks {
            if let Terminator::ReturnUnit { edge, .. } = block.terminator {
                assert!(
                    fuel.usage()
                        .at(terminal_fuel::FuelChargeSite::Edge(edge))
                        .is_none()
                );
            }
        }
    }
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut fuel, &mut observer)
            .unwrap(),
        status
    );
    assert_eq!(execution.effects(), effects);
}

#[test]
fn boundary_temporary_custody_rejects_substitution_reordering_and_duplicate_cleanup() {
    let original = checked(&source(
        "Sink::consume(forward(Factory::create(prefix, 19u16), prefix), prefix);",
    ));
    let artifact = encoded_locals(&original, &["prefix", "first", "spare"]);
    for mutation in 0..5 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == main_machine(&original).symbol)
            .unwrap();
        let temporary = plan.operations.iter().position(|operation| matches!(operation, CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. } if coordinate.call_ordinal != 0)).unwrap();
        let consumer = plan
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                )
            })
            .unwrap();
        if mutation == 0 {
            plan.operations.swap(temporary, consumer);
        } else if mutation == 1 {
            let CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            } = &mut plan.operations[consumer]
            else {
                unreachable!()
            };
            structural_arguments[0].source =
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: 1,
                };
        } else if mutation == 2 {
            let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                discard_result_on_return,
                ..
            } = &mut plan.operations[temporary]
            else {
                unreachable!()
            };
            *discard_result_on_return = true;
        } else if mutation == 3 {
            let CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } =
                &mut plan.operations[temporary]
            else {
                unreachable!()
            };
            result.type_identity.push_str("::different_result");
        } else {
            let CheckedUnitEffectOperationPlan::StructuralCall { source_site, .. } =
                &plan.operations[consumer]
            else {
                unreachable!()
            };
            let different_site = *source_site;
            let CheckedUnitEffectOperationPlan::BoundaryStructuralCall { source_site, .. } =
                &mut plan.operations[temporary]
            else {
                unreachable!()
            };
            assert_ne!(*source_site, different_site);
            *source_site = different_site;
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
    let mut module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let consumed = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } if !structural_arguments.is_empty() => Some(structural_arguments[0].place),
            _ => None,
        })
        .unwrap();
    for block in &mut entry.blocks {
        if let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut block.terminator
        {
            trivial_affine_discards.push(consumed);
        }
    }
    assert!(
        terminal_verifier::verify_module(
            &module,
            &decode_proof_bundle(&artifact.1).unwrap(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}

#[test]
fn nominal_boundary_temporary_keeps_the_raw_callable_occurrence_identity() {
    let source = source("Sink::consume(Factory::create(prefix, 19u16), prefix);")
        .replace(
            "machine Main::main()",
            "machine Main::main<machine Create>() where machine Create satisfies Factory::create;",
        )
        .replace("Factory::create(", "Create(");
    let original = checked(&source);
    let artifact = encoded_locals(&original, &["prefix", "first", "spare"]);
    assert_completion(
        &artifact,
        &[vec![5, 7], vec![11, 13], vec![5, 19], vec![5]],
        &[700, 701, 702],
        &[702],
    );
    let machine = main_machine(&original);
    let (handle, occurrence) = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, occurrence)| {
            occurrence.statement_index == 3
                && occurrence.call_ordinal == 1
                && original
                    .typed
                    .machine_parameter_signature(occurrence.target_symbol)
                    .is_some_and(|(owner, _)| owner.symbol == machine.symbol)
        })
        .unwrap();
    let (_, requirement) = original
        .typed
        .machine_parameter_signature(occurrence.target_symbol)
        .unwrap();
    assert_ne!(occurrence.target_symbol, requirement.symbol);
    let mut changed = original.clone();
    changed
        .facts
        .flow
        .control
        .calls
        .get_mut(handle)
        .target_symbol = requirement.symbol;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
        "the same resolved signature cannot replace the authored callable parameter"
    );
}
