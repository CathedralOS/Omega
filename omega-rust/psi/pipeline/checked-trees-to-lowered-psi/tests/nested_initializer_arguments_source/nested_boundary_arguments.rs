use super::boundary_result_moves::{ObserveMoves, source};
use super::later_results::encoded_locals;
use super::*;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_psi::{OperationKind, OperationResult, Terminator};

#[test]
fn nested_boundary_custody_rejects_reordering_substitution_and_duplicate_cleanup() {
    let original = checked(&source(
        "Sink::consume(forward(forward(first, prefix), prefix), identity16(prefix));",
    ));
    let artifact = encoded_locals(&original, &["prefix", "first", "spare"]);
    for mutation in 0..3 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == main_machine(&original).symbol)
            .unwrap();
        let producers = plan
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                )
                .then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(producers.len(), 2);
        if mutation == 0 {
            plan.operations.swap(producers[0], producers[1]);
        } else if let CheckedUnitEffectOperationPlan::StructuralCall {
            structural_arguments,
            discard_result_on_return,
            ..
        } = &mut plan.operations[producers[1]]
        {
            if mutation == 1 {
                structural_arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: 1,
                    };
            } else {
                *discard_result_on_return = true;
            }
        }
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err());
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
            &AdmissionProfile::default(),
        )
        .is_err()
    );
}

#[test]
fn nested_boundary_arguments_preserve_effect_order_result_slots_and_cleanup() {
    for declaration in ["static", "nominal", "bodyless"] {
        for (result, completion, names, final_calls, produced, consumed) in [
            (
                "",
                "Sink::take(Main::probe(prefix), forward(forward(first, Main::probe(11u16)), Main::probe(22u16)), Main::probe(33u16));",
                vec!["prefix", "first", "spare"],
                vec![vec![5, 33]],
                vec![700, 701],
                vec![700],
            ),
            (
                "-> u16",
                "let measured: u16 = Sink::take(Main::probe(prefix), forward(forward(first, Main::probe(11u16)), Main::probe(22u16)), Main::probe(33u16)); Host::finish(measured);",
                vec!["prefix", "first", "spare", "measured"],
                vec![vec![5, 33], vec![33]],
                vec![700, 701],
                vec![700],
            ),
            (
                "-> Token",
                "let replacement: Token = Sink::take(Main::probe(prefix), forward(forward(first, Main::probe(11u16)), Main::probe(22u16)), Main::probe(33u16)); Sink::consume(replacement, identity16(prefix));",
                vec!["prefix", "first", "spare", "replacement"],
                vec![vec![5, 33], vec![5]],
                vec![700, 701, 702],
                vec![700, 702],
            ),
        ] {
            let completion = format!("{completion} Host::finish(prefix);");
            let mut source = source(&completion).replace(
                "reaches Factory + Sink",
                "reaches Factory + Sink + Producer",
            );
            source.push_str(
                "machine Main::probe(value: u16) -> u16 reaches Producer { let result: u16 = Producer::choose(value, value, value); result }",
            );
            if declaration == "bodyless" {
                source = source
                    .replace("data Main {}", "pub data Main {}")
                    .replace("Sink::take(", "Main::take(");
                source.push_str(&format!(
                    "boundary machine Main::take(first: u16, token: Token, last: u16) {result} reaches Sink ensures true;"
                ));
            } else {
                source = source.replace(
                    "boundary trait Sink {",
                    &format!("boundary trait Sink {{ machine take(first: u16, token: Token, last: u16) {result} reaches Sink;"),
                );
                if declaration == "nominal" {
                    source = source
                        .replace("machine Main::main()", "machine Main::main<machine Take>() where machine Take satisfies Sink::take;")
                        .replace("Sink::take(", "Take(");
                }
            }
            let checked = checked(&source);
            let artifact = encoded_locals(&checked, &names);
            let published =
                terminal_production::produce_terminal_artifact(&checked, "Main::main").unwrap();
            let module = decode_module(&artifact.0).unwrap();
            assert_eq!(decode_module(published.semantic_bytes()).unwrap(), module);
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
            for block in &entry.blocks {
                if let Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } = &block.terminator
                {
                    assert_eq!(trivial_affine_discards, &[boundary_results[1]]);
                }
            }
            let expected = [
                vec![5, 7],
                vec![11, 13],
                vec![5, 5, 5],
                vec![11, 11, 11],
                vec![22, 22, 22],
                vec![33, 33, 33],
            ]
            .into_iter()
            .chain(final_calls)
            .chain([vec![5]])
            .map(|arguments| {
                arguments
                    .into_iter()
                    .map(|value| unsigned(16, value))
                    .collect::<Vec<_>>()
            })
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
                assert!(complete, "{declaration} {result}");
                assert_eq!(observer.results.calls, expected, "{declaration} {result}");
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
    }
}
