use super::boundary_result_moves::{ObserveMoves, source};
use super::later_results::encoded_locals;
use super::{
    AdmissionProfile, TerminalExecutionResult, decode_module, invoking, main_machine, unsigned,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use std::collections::BTreeSet;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, OperationResult, Terminator};

fn observed_source(completion: &str) -> String {
    let mut source = source(completion).replace(
        "reaches Factory + Sink",
        "reaches Factory + Sink + Producer",
    );
    source.push_str("machine Main::probe(value: u16) -> u16 reaches Producer { let result: u16 = Producer::choose(value, value, value); result }");
    source
}

fn start(artifact: &(Vec<u8>, Vec<u8>)) -> TerminalExecution {
    TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
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
            match execution.resume(&mut fuel, &mut observer).unwrap() {
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
        let checked = crate::front_end::checked_program(&source);
        let artifact = encoded_locals(&checked, &names);
        let published = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("Main::main"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap()
        .into_artifact();
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
            source = invoking(&source, "Factory");
        }
        let checked = crate::front_end::checked_program(&source);
        let artifact = encoded_locals(&checked, &["prefix", "first", "spare", "measured"]);
        let published = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("Main::main"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap()
        .into_artifact();
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
fn open_nominal_boundary_temporaries_require_a_closed_selection() {
    // Nominal unit callbacks require closed execution selections: the open
    // generic entry does not publish, while the explicit invocation of the
    // same boundary runs the same temporary schedule.
    let completion = "Sink::consume(Factory::create(prefix, 19u16), prefix);";
    let open = source(completion)
        .replace(
            "machine Main::main()",
            "machine Main::main<machine Create>() where machine Create satisfies Factory::create;",
        )
        .replace("Factory::create(", "Create(");
    assert!(matches!(
        checked_trees_to_lowered_psi::lower_machine(&crate::front_end::checked_program(&open), TerminalMachineSelection::Name("Main::main")),
        Err(checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan { machine, .. })
            if machine == "Main::main"
    ));
    let explicit = crate::front_end::checked_program(&invoking(&source(completion), "Factory"));
    let artifact = encoded_locals(&explicit, &["prefix", "first", "spare"]);
    assert_completion(
        &artifact,
        &[vec![5, 7], vec![11, 13], vec![5, 19], vec![5]],
        &[700, 701, 702],
        &[702],
    );
    let machine = main_machine(&explicit);
    let (handle, occurrence) = explicit
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, occurrence)| occurrence.statement_index == 3 && occurrence.call_ordinal == 1)
        .unwrap();
    let requirement = occurrence.target_symbol;
    let mut changed = explicit.clone();
    changed
        .facts
        .flow
        .control
        .calls
        .get_mut(handle)
        .target_symbol = machine.symbol;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("Main::main")
        )
        .is_err(),
        "the checked call occurrence keeps its authored boundary target {requirement:?}"
    );
}
