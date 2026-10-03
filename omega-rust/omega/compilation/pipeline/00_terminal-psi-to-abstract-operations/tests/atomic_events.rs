//! ATOMIC-MEMORY-MODEL: a verified Terminal `AtomicAccess` becomes one
//! normalized abstract atomic event at its exact location, with the
//! reads-from and modification-after edges the optimization unit rechecks.
//!
//! Sources run serial events through a borrowed receiver and through named
//! state parameters that forward two independent cells. The controls substitute a retained location, ordering,
//! operand or edge and check that plan replay or the independent unit
//! validation refuses it, and that a load after a join of distinct writes
//! refuses rather than naming one of them.

use abstract_operations::{
    AbstractAtomicEvent, AbstractAtomicReadModifyWrite, AbstractOperation, AbstractOperationPlan,
    AtomicModificationAfter, AtomicReadsFrom,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::OperationId;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi::{MemoryOrdering, OperationKind, StructuralTypeShape, TerminalModule};
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, ArtifactSections, LoweringError, ProviderInstallationError,
    admit_provider_installation, build_verified_psi_optimization_unit, lower_artifact,
};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check")
}

/// The canonical sections of `Cell::run` over one `AtomicU32` field.
fn artifact(body: &str) -> (TerminalModule, Vec<u8>, Vec<u8>) {
    let source = format!(
        "boundary trait Report {{ machine value(v: u32) reaches Report; }}
        data Cell {{ counter: AtomicU32; }}
        machine Cell::run(cell: &Cell) reaches Report {{
            {body}
        }}"
    );
    source_artifact(&source)
}

fn source_artifact(source: &str) -> (TerminalModule, Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked(source),
        TerminalMachineSelection::Name("Cell::run"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantic = encode_module(&lowered.semantic_module).expect("canonical semantics");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("canonical proof");
    (lowered.semantic_module, semantic, proof)
}

fn lower(semantic: &[u8], proof: &[u8]) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
    lower_artifact(
        ArtifactSections {
            semantic_bytes: semantic,
            proof_bytes: proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
}

/// Every retained atomic event, in operation order.
fn events(
    plan: &AbstractOperationPlan,
) -> Vec<(
    OperationId,
    AbstractAtomicEvent,
    Option<AtomicReadsFrom>,
    Option<AtomicModificationAfter>,
)> {
    plan.functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            AbstractOperation::AtomicEvent {
                psi_operation,
                event,
                reads_from,
                modification_after,
            } => Some((
                *psi_operation,
                event.clone(),
                *reads_from,
                *modification_after,
            )),
            _ => None,
        })
        .collect()
}

fn event_mut(operation: &mut AbstractOperation) -> &mut AbstractAtomicEvent {
    let AbstractOperation::AtomicEvent { event, .. } = operation else {
        unreachable!("positions name atomic events")
    };
    event
}

/// The verified optimization unit the artifact admits.
fn unit(semantic: &[u8], proof: &[u8]) -> optimization_unit::PsiOptimizationUnit {
    let input = lower_artifact(
        ArtifactSections {
            semantic_bytes: semantic,
            proof_bytes: proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .expect("optimizer admission retains atomic events");
    build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("reconstruct the optimizer unit from canonical input")
    .unit()
    .clone()
}

/// `unit` with the `ordinal`-th atomic node changed by `change` and its
/// identity recomputed, so only the substitution can fail validation.
fn substitute(
    unit: &optimization_unit::PsiOptimizationUnit,
    ordinal: usize,
    change: impl FnOnce(&mut AbstractOperation),
) -> optimization_unit::PsiOptimizationUnit {
    let mut changed = unit.clone();
    let operation = changed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .map(|node| &mut node.operation)
        .filter(|operation| matches!(operation, AbstractOperation::AtomicEvent { .. }))
        .nth(ordinal)
        .expect("atomic node");
    change(operation);
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    changed
}

const SEQUENCE: &str = "
    cell.counter.store(10, Publish);
    let observed: u32 = cell.counter.load(Receive);
    let added: u32 = cell.counter.fetch_add(5, ReceivePublish);
    let displaced: u32 = cell.counter.swap(20, GlobalOrder);
    let prior: u32 = cell.counter.compare_exchange(20, 30, GlobalOrder, Receive);
    Report::value(observed);
    Report::value(added);
    Report::value(displaced);
    Report::value(prior);
";

#[test]
fn every_serial_event_keeps_its_location_orderings_and_coherence_edges() {
    let (module, semantic, proof) = artifact(SEQUENCE);
    let machine = &module.machines[0];
    let receiver = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.position == 0)
        .expect("receiver");
    let StructuralTypeShape::Record { fields } = &module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == receiver.structural_type)
        .expect("receiver type")
        .shape
    else {
        panic!("Cell is a record");
    };
    let counter = fields
        .iter()
        .find(|field| field.identity == "counter")
        .expect("counter")
        .id;
    let terminal = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::AtomicAccess { .. }))
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    let plan = lower(&semantic, &proof).expect("atomic events lower");
    let events = events(&plan);
    assert_eq!(
        events.iter().map(|(id, ..)| *id).collect::<Vec<_>>(),
        terminal,
        "one abstract event per Terminal event, under its identity"
    );
    for (_, event, ..) in &events {
        let location = event.location().expect("every access names a location");
        assert_eq!(location.root, receiver.place);
        assert!(location.path.is_empty());
        assert_eq!(location.field, counter);
    }
    let [store, load, fetch, swap, exchange] = events.as_slice() else {
        panic!("five events: {events:#?}");
    };
    assert!(matches!(
        store,
        (
            _,
            AbstractAtomicEvent::Store {
                ordering: MemoryOrdering::Publish,
                ..
            },
            None,
            Some(AtomicModificationAfter::InitialResidency)
        )
    ));
    assert!(matches!(
        load,
        (_, AbstractAtomicEvent::Load { ordering: MemoryOrdering::Receive, .. },
            Some(AtomicReadsFrom::Write { operation }), None) if *operation == store.0
    ));
    assert!(matches!(
        fetch,
        (_, AbstractAtomicEvent::ReadModifyWrite {
            operation: AbstractAtomicReadModifyWrite::FetchAdd,
            ordering: MemoryOrdering::ReceivePublish, ..
        }, Some(AtomicReadsFrom::Write { operation: read }),
            Some(AtomicModificationAfter::Write { operation: after }))
            if *read == store.0 && *after == store.0
    ));
    assert!(matches!(
        swap,
        (_, AbstractAtomicEvent::Swap { ordering: MemoryOrdering::GlobalOrder, .. },
            Some(AtomicReadsFrom::Write { operation: read }),
            Some(AtomicModificationAfter::Write { operation: after }))
            if *read == fetch.0 && *after == fetch.0
    ));
    assert!(matches!(
        exchange,
        (_, AbstractAtomicEvent::CompareExchange {
            success: MemoryOrdering::GlobalOrder,
            failure: MemoryOrdering::Receive, ..
        }, Some(AtomicReadsFrom::Write { operation: read }),
            Some(AtomicModificationAfter::Write { operation: after }))
            if *read == swap.0 && *after == swap.0
    ));
    optimization_unit_semantics::validate_psi_optimization_unit(&unit(&semantic, &proof))
        .expect("the unit rechecks every retained edge");
    admit_provider_installation(&plan, &semantic, &proof, &AdmissionProfile::default(), &[])
        .expect("the plan replays from its artifact");
}

#[test]
fn a_substituted_location_ordering_operand_or_edge_is_refused() {
    let (_, semantic, proof) = artifact(SEQUENCE);
    let plan = lower(&semantic, &proof).expect("atomic events lower");
    let positions = plan
        .functions
        .iter()
        .enumerate()
        .flat_map(|(function, body)| {
            body.operations
                .iter()
                .enumerate()
                .filter(|(_, operation)| matches!(operation, AbstractOperation::AtomicEvent { .. }))
                .map(move |(index, _)| (function, index))
        })
        .collect::<Vec<_>>();
    let edit = |position: usize, change: &dyn Fn(&mut AbstractOperation)| {
        let (function, index) = positions[position];
        let mut drifted = plan.clone();
        change(&mut drifted.functions[function].operations[index]);
        drifted
    };
    // Location, ordering and operand substitutions are refused by replay
    // against the verified artifact.
    let replay_refusals = [
        edit(0, &|operation| {
            let event = event_mut(operation);
            let AbstractAtomicEvent::Store { location, .. } = event else {
                unreachable!()
            };
            location.field = semantic_vocabulary::StructuralFieldId::new(99).unwrap();
        }),
        edit(1, &|operation| {
            let event = event_mut(operation);
            let AbstractAtomicEvent::Load { ordering, .. } = event else {
                unreachable!()
            };
            *ordering = MemoryOrdering::GlobalOrder;
        }),
        edit(4, &|operation| {
            let event = event_mut(operation);
            let AbstractAtomicEvent::CompareExchange { failure, .. } = event else {
                unreachable!()
            };
            *failure = MemoryOrdering::NoOrdering;
        }),
        edit(2, &|operation| {
            let event = event_mut(operation);
            let AbstractAtomicEvent::ReadModifyWrite { operation, .. } = event else {
                unreachable!()
            };
            *operation = AbstractAtomicReadModifyWrite::FetchSub;
        }),
    ];
    for drifted in &replay_refusals {
        assert!(matches!(
            admit_provider_installation(
                drifted,
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[]
            ),
            Err(ProviderInstallationError::PlanReplayMismatch)
        ));
    }
    // A substituted edge is refused by the unit's own happens-before replay.
    let verified = unit(&semantic, &proof);
    let edge_refusals = [
        substitute(&verified, 1, |operation| {
            let AbstractOperation::AtomicEvent { reads_from, .. } = operation else {
                unreachable!()
            };
            *reads_from = Some(AtomicReadsFrom::InitialResidency);
        }),
        substitute(&verified, 3, |operation| {
            let AbstractOperation::AtomicEvent {
                modification_after, ..
            } = operation
            else {
                unreachable!()
            };
            *modification_after = Some(AtomicModificationAfter::InitialResidency);
        }),
        substitute(&verified, 4, |operation| {
            let AbstractOperation::AtomicEvent { reads_from, .. } = operation else {
                unreachable!()
            };
            *reads_from = Some(AtomicReadsFrom::Write {
                operation: OperationId::new(1).expect("nonzero"),
            });
        }),
    ];
    for drifted in &edge_refusals {
        let refusal = optimization_unit_semantics::validate_psi_optimization_unit(drifted);
        assert!(
            matches!(
                refusal,
                Err(optimization_unit_semantics::OptimizationUnitValidationError::AtomicEventCoherenceMismatch { .. })
            ),
            "{refusal:?}"
        );
    }
}

#[test]
fn shared_callee_mutation_does_not_leave_a_known_atomic_predecessor() {
    let (_, semantic, proof) = source_artifact(
        "data Cell { counter: AtomicU32; }
        machine Cell::write(cell: &Cell) { cell.counter.store(2, NoOrdering); }
        machine Cell::run(cell: &Cell) -> u32 {
            cell.counter.store(1, NoOrdering);
            Cell::write(cell);
            let observed: u32 = cell.counter.load(NoOrdering);
            (observed)
        }",
    );
    assert!(matches!(
        lower(&semantic, &proof),
        Err(ArtifactLoweringError::Lowering(
            LoweringError::UnsupportedAtomicCoherence(_)
        ))
    ));
}

#[test]
fn different_shared_roots_do_not_prove_an_atomic_predecessor_survives() {
    for mutation in [
        "second.counter.store(2, NoOrdering);",
        "Cell::write(second);",
    ] {
        let (_, semantic, proof) = source_artifact(&format!(
            "data Cell {{ counter: AtomicU32; }}
            machine Cell::write(cell: &Cell) {{ cell.counter.store(2, NoOrdering); }}
            machine Cell::run(first: &Cell, second: &Cell) -> u32 {{
                first.counter.store(1, NoOrdering);
                {mutation}
                let observed: u32 = first.counter.load(NoOrdering);
                (observed)
            }}"
        ));
        assert!(
            matches!(
                lower(&semantic, &proof),
                Err(ArtifactLoweringError::Lowering(
                    LoweringError::UnsupportedAtomicCoherence(_)
                ))
            ),
            "{mutation}"
        );
    }
}

#[test]
fn a_join_of_distinct_writes_names_no_single_predecessor() {
    // Both arms write the cell; the load after the join has two candidate
    // writes and no retained edge could name the one it observes.
    let (_, semantic, proof) = artifact(
        "
        cell.counter.store(1, NoOrdering);
        let branch: u32 = cell.counter.load(NoOrdering);
        transition branch == 1 {
            true -> first(cell)
            false -> second(cell)
        }
        state first(cell: &Cell) {
            cell.counter.store(2, NoOrdering);
            transition { _ -> observe(cell) }
        }
        state second(cell: &Cell) {
            cell.counter.store(3, NoOrdering);
            transition { _ -> observe(cell) }
        }
        state observe(cell: &Cell) {
            let seen: u32 = cell.counter.load(NoOrdering);
            Report::value(seen);
        }
        ",
    );
    assert!(matches!(
        lower(&semantic, &proof),
        Err(ArtifactLoweringError::Lowering(
            LoweringError::UnsupportedAtomicCoherence(_)
        ))
    ));
}

const NAMED_SEQUENCE: &str = "
    boundary trait Report { machine value(v: u32) reaches Report; }
    data Cell { counter: AtomicU32; }
    machine Cell::run(first: &mut Cell, second: &mut Cell) reaches Report {
        first.counter.store(10, Publish);
        second.counter.store(40, Publish);
        transition { _ -> add(second, first) }
        state add(other: &mut Cell, current: &mut Cell) {
            let prior: u32 = current.counter.fetch_add(5, ReceivePublish);
            Report::value(prior);
            transition { _ -> observe(current, other) }
        }
        state observe(current: &mut Cell, other: &mut Cell) {
            let observed: u32 = current.counter.load(Receive);
            let prior: u32 = other.counter.compare_exchange(40, 60, GlobalOrder, Receive);
            Report::value(observed);
            Report::value(prior);
        }
    }
";

#[test]
fn named_state_parameters_preserve_each_atomic_backing_and_event_root() {
    let (module, semantic, proof) = source_artifact(NAMED_SEQUENCE);
    let plan = lower(&semantic, &proof).expect("named atomic state parameters lower");
    let events = events(&plan);
    let [first_store, second_store, fetch, load, exchange] = events.as_slice() else {
        panic!("five events: {events:#?}");
    };
    assert_eq!(
        fetch.2,
        Some(AtomicReadsFrom::Write {
            operation: first_store.0
        })
    );
    assert_eq!(
        fetch.3,
        Some(AtomicModificationAfter::Write {
            operation: first_store.0
        })
    );
    assert_eq!(load.2, Some(AtomicReadsFrom::Write { operation: fetch.0 }));
    assert_eq!(
        exchange.2,
        Some(AtomicReadsFrom::Write {
            operation: second_store.0
        })
    );
    assert_eq!(
        exchange.3,
        Some(AtomicModificationAfter::Write {
            operation: second_store.0
        })
    );
    for (operation, event, ..) in &events {
        let source = module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|candidate| candidate.id == *operation)
            .expect("source operation");
        let OperationKind::AtomicAccess { place, .. } = &source.kind else {
            panic!("source atomic event");
        };
        assert_eq!(event.location().expect("atomic location").root, *place);
    }
    optimization_unit_semantics::validate_psi_optimization_unit(&unit(&semantic, &proof))
        .expect("the unit follows actual incoming storage bindings");
}

#[test]
fn changed_event_root_or_initial_residency_is_refused() {
    let (_, semantic, proof) = source_artifact(NAMED_SEQUENCE);
    let verified = unit(&semantic, &proof);
    let plan = lower(&semantic, &proof).expect("named atomic state parameters lower");
    let events = events(&plan);
    let other = events[4]
        .1
        .location()
        .expect("compare exchange location")
        .root;
    // These unconditional source aliases canonicalize to their original roots.
    // Retained binding substitutions are exercised by optimization-unit
    // semantics' `atomic_coherence_rejects_stale_initial_and_wrong_forwarded_backing`.
    let substitutions = [
        (
            "other context at the same observer",
            substitute(&verified, 3, |operation| {
                let AbstractAtomicEvent::Load { location, .. } = event_mut(operation) else {
                    unreachable!("load ordinal");
                };
                location.root = other;
            }),
        ),
        (
            "initial read after a transferred write",
            substitute(&verified, 3, |operation| {
                let AbstractOperation::AtomicEvent { reads_from, .. } = operation else {
                    unreachable!("atomic ordinal");
                };
                *reads_from = Some(AtomicReadsFrom::InitialResidency);
            }),
        ),
        (
            "initial modification after a transferred write",
            substitute(&verified, 2, |operation| {
                let AbstractOperation::AtomicEvent {
                    modification_after, ..
                } = operation
                else {
                    unreachable!("atomic ordinal");
                };
                *modification_after = Some(AtomicModificationAfter::InitialResidency);
            }),
        ),
    ];
    for (reason, changed) in substitutions {
        let refusal = optimization_unit_semantics::validate_psi_optimization_unit(&changed);
        assert!(matches!(refusal,
            Err(optimization_unit_semantics::OptimizationUnitValidationError::AtomicEventCoherenceMismatch { .. })),
            "{reason}: {refusal:?}");
    }
}
