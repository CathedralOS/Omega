//! The bundle must give every closure the rows the free closure functions
//! give. Two twins differ only in machine 2's body: in the static twin it
//! returns at once, in the dynamic twin it selects an implementation and
//! calls it dynamically, so every row in a closure that reaches it must be
//! adapted against that closure's rows.

use super::*;
use crate::descriptors::descriptor_results::{
    terminal_graph_closure, terminal_graph_for, terminal_graph_rows,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, OperationId, ScalarQualificationSetId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineResult, MachineContract, Operation, OperationResult,
    RegisterTargetApplicability, TerminalDynamicInterface, TerminalDynamicSlot,
    TerminalDynamicTable, TerminalIdentityIndex, TerminalMachineResult, Terminator,
    ValueDeclaration,
};

const ROOT: u64 = 1;
const SELECTING: u64 = 2;
const LEAF: u64 = 3;
const CALLS_MISSING: u64 = 4;
const REALIZATION: u64 = 5;
const MISSING: u64 = 9;
const SELECTOR: u64 = 21;

const KINDS: [DescriptorSlotKind; SLOT_KINDS] = [
    DescriptorSlotKind::Views,
    DescriptorSlotKind::References,
    DescriptorSlotKind::BorrowedStorage,
    DescriptorSlotKind::AccessOrigins,
];

fn id(number: u64) -> MachineId {
    MachineId::new(number).unwrap()
}

fn machine(identity: u64, operations: Vec<Operation>) -> TerminalMachine {
    let entry = BlockId::new(1).unwrap();
    let mut block = Block::plain(
        entry,
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    block.operations = operations;
    TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: id(identity),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        block_entry_claims: Vec::new(),
        call_formations: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![block],
        contract: MachineContract {
            result: TerminalMachineResult::Unit,
            ..MachineContract::empty(ContractId::new(1).unwrap())
        },
    }
}

fn call(operation: u64, callee: u64) -> Operation {
    Operation::unsuspended(
        OperationId::new(operation).unwrap(),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: id(callee),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    )
}

fn dynamic_call(first_operation: u64) -> Vec<Operation> {
    let selector = ValueId::new(SELECTOR).unwrap();
    vec![
        Operation::unsuspended(
            OperationId::new(first_operation).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: selector,
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::SelectImplementation { table: 0 },
        ),
        Operation::unsuspended(
            OperationId::new(first_operation + 1).unwrap(),
            OperationResult::Unit,
            OperationKind::CallDynamic {
                selector,
                interface: 0,
                slot: 0,
                arguments: Vec::new(),
                family_actuals: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ),
    ]
}

/// The root calls machines 2 and 3. Machine 4 calls a machine the module does
/// not declare, so its row fails. Machine 5 is the only realization of the
/// dynamic slot.
fn module(dynamic: bool) -> TerminalModule {
    let mut module = TerminalModule::for_entry(id(ROOT));
    module
        .execution
        .dynamic_dispatch
        .interfaces
        .push(TerminalDynamicInterface {
            id: 0,
            identity: TerminalIdentityIndex(7),
            slots: vec![TerminalDynamicSlot {
                slot: 0,
                declaring_trait_identity: TerminalIdentityIndex(7),
                requirement_identity: TerminalIdentityIndex(8),
                family_tuple: Vec::new(),
                scalar_parameters: Vec::new(),
                structural_parameters: Vec::new(),
                entry_claim_paths: Vec::new(),
                result: BoundaryMachineResult::Unit,
                result_claim_paths: Vec::new(),
                result_reference_regions: Vec::new(),
            }],
        });
    module
        .execution
        .dynamic_dispatch
        .tables
        .push(TerminalDynamicTable {
            id: 0,
            interface: 0,
            implementation_identity: TerminalIdentityIndex(9),
            rows: vec![Some(id(REALIZATION))],
        });
    // Declaration order differs from identity order, so module order and
    // sorted lookup are both exercised.
    module.machines = vec![
        machine(LEAF, Vec::new()),
        machine(ROOT, vec![call(1, SELECTING), call(2, LEAF)]),
        machine(REALIZATION, Vec::new()),
        machine(
            SELECTING,
            if dynamic { dynamic_call(3) } else { Vec::new() },
        ),
        machine(CALLS_MISSING, vec![call(5, MISSING)]),
    ];
    module
}

/// Every row field that can be printed, plus each call's targets.
fn shape(row: &DescriptorMachine) -> String {
    let calls = row
        .calls
        .iter()
        .map(|(position, calls)| {
            let targets = calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|call| match &call.target {
                        DescriptorInvocationTarget::Machine(callee) => {
                            format!("machine {callee:?}")
                        }
                        DescriptorInvocationTarget::Public {
                            effects_machine,
                            realizations,
                            invalidates_contents,
                            ..
                        } => format!(
                            "public {effects_machine:?} {realizations:?} {invalidates_contents}"
                        ),
                        DescriptorInvocationTarget::Provider {
                            invalidates_contents,
                            ..
                        } => format!("provider {invalidates_contents}"),
                    })
                    .collect::<Vec<_>>()
            });
            format!("{position:?}: {targets:?}")
        })
        .collect::<Vec<_>>();
    format!(
        "{:?} returns {:?} places {:?} types {:?} entry {:?} flow {:?} cases {:?} {:?} \
         origins {:?} projections {:?} queries {:?} producers {:?} literals {:?} \
         derivations {:?} unresolved {:?} exits {:?} calls {:?}",
        row.id,
        row.returns,
        row.places
            .iter()
            .map(|place| place.place)
            .collect::<Vec<_>>(),
        row.place_types,
        row.entry,
        row.flow.keys().collect::<Vec<_>>(),
        row.case_constructors,
        row.case_successors,
        row.originations,
        row.projections,
        row.contents_queries,
        row.payload_producers,
        row.unsigned_index_literals,
        row.storage_derivations,
        row.unresolved_contents_writes,
        row.normal_exits,
        calls,
    )
}

fn shapes<'a>(rows: impl IntoIterator<Item = &'a DescriptorMachine>) -> Vec<String> {
    rows.into_iter().map(shape).collect()
}

fn result_shapes<'a>(
    rows: impl IntoIterator<Item = Result<&'a DescriptorMachine, DescriptorGraphError>>,
) -> Vec<Result<String, DescriptorGraphError>> {
    rows.into_iter().map(|row| row.map(shape)).collect()
}

#[test]
fn a_closure_without_dynamic_calls_shares_the_rows_the_free_closure_builds() {
    let module = module(false);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    for kind in KINDS {
        let expected = terminal_graph_closure(&module, [id(ROOT)], kind, &[]).unwrap();
        assert_eq!(expected.len(), 3, "{kind:?}");
        // A second request reads the rows the first one adapted.
        for _ in 0..2 {
            let rows = analyses.descriptor_closure(kind, [id(ROOT)]).unwrap();
            assert!(matches!(rows, DescriptorRows::Shared { .. }), "{kind:?}");
            assert_eq!(shapes(rows.iter()), shapes(&expected), "{kind:?}");
            for row in &expected {
                assert_eq!(
                    rows.machine(row.id).map(shape),
                    Some(shape(row)),
                    "{kind:?}"
                );
            }
            assert!(rows.machine(id(REALIZATION)).is_none());
        }
    }
}

#[test]
fn a_closure_with_a_dynamic_call_owns_rows_adapted_against_that_closure() {
    let module = module(true);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    assert!(analyses.calls_dynamically(id(SELECTING)));
    assert!(!analyses.calls_dynamically(id(ROOT)));
    for kind in KINDS {
        let expected = terminal_graph_closure(&module, [id(ROOT)], kind, &[]).unwrap();
        // The dynamic call reaches its realization.
        assert_eq!(expected.len(), 4, "{kind:?}");
        let rows = analyses.descriptor_closure(kind, [id(ROOT)]).unwrap();
        assert!(matches!(rows, DescriptorRows::Owned(_)), "{kind:?}");
        assert_eq!(shapes(rows.iter()), shapes(&expected), "{kind:?}");
        // A closure that does not reach the dynamic call still shares.
        let leaf = analyses.descriptor_closure(kind, [id(LEAF)]).unwrap();
        assert!(matches!(leaf, DescriptorRows::Shared { .. }), "{kind:?}");
        assert_eq!(
            shapes(leaf.iter()),
            shapes(&terminal_graph_closure(&module, [id(LEAF)], kind, &[]).unwrap()),
            "{kind:?}"
        );
    }
}

/// The closure walk as it stood before the bundle: it sorts the machines,
/// adapts each reached body, and adapts every row again against the closure's
/// rows when a reached body calls dynamically. Kept as an independent oracle.
fn rows_before_the_bundle(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = MachineId>,
    kind: DescriptorSlotKind,
) -> Vec<RowResult> {
    let mut machines = module.machines.iter().collect::<Vec<_>>();
    machines.sort_unstable_by_key(|machine| machine.id);
    if let Some(pair) = machines.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return vec![Err(DescriptorGraphError::MissingOrAmbiguousMachine(
            pair[1].id,
        ))];
    }
    let find = |identity: MachineId| {
        machines
            .binary_search_by_key(&identity, |machine| machine.id)
            .ok()
            .map(|position| machines[position])
    };
    let zeroed_cases = ZeroedCases::new(&module.types.structural_types);
    let adapt = |body: &TerminalMachine, closure: Option<&[DescriptorMachine]>| {
        terminal_flow::adapt(
            module,
            &machines,
            body,
            kind,
            &[],
            closure,
            GenericCallInputs::default(),
            &zeroed_cases,
        )
    };
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    // Each reached identity once, with its row or the failure to find it.
    let mut reached: Vec<(MachineId, RowResult)> = Vec::new();
    while let Some(identity) = pending.pop() {
        if reached.iter().any(|(seen, _)| *seen == identity) {
            continue;
        }
        let Some(body) = find(identity) else {
            reached.push((
                identity,
                Err(DescriptorGraphError::MissingOrAmbiguousMachine(identity)),
            ));
            continue;
        };
        let row = adapt(body, None);
        if let Ok(row) = &row {
            push_callees(row, &mut pending);
        }
        reached.push((identity, row));
    }
    // Declared machines in module order, then undeclared ones by identity.
    let mut rows = Vec::new();
    for body in &module.machines {
        if let Some(index) = reached.iter().position(|(seen, _)| *seen == body.id) {
            rows.push(reached.swap_remove(index).1);
        }
    }
    reached.sort_unstable_by_key(|(identity, _)| *identity);
    rows.extend(reached.into_iter().map(|(_, row)| row));
    if !rows
        .iter()
        .filter_map(|row| row.as_ref().ok())
        .any(|row| find(row.id).is_some_and(body_calls_dynamically))
    {
        return rows;
    }
    let (initial, failures): (Vec<_>, Vec<_>) = rows.into_iter().partition(Result::is_ok);
    let initial = initial.into_iter().map(Result::unwrap).collect::<Vec<_>>();
    let mut rows = initial
        .iter()
        .map(|row| adapt(find(row.id).unwrap(), Some(&initial)))
        .collect::<Vec<_>>();
    rows.extend(failures);
    rows
}

#[test]
fn closures_give_the_rows_the_walk_before_the_bundle_gave() {
    for dynamic in [false, true] {
        let module = module(dynamic);
        let analyses = ModuleAnalyses::new(&module).unwrap();
        for kind in KINDS {
            for roots in [
                vec![id(ROOT)],
                vec![id(LEAF)],
                vec![id(CALLS_MISSING), id(ROOT)],
                vec![id(MISSING), id(ROOT)],
                module.machines.iter().map(|machine| machine.id).collect(),
            ] {
                let expected = row_results(&rows_before_the_bundle(&module, roots.clone(), kind));
                let context = format!("dynamic {dynamic}, {kind:?}, {roots:?}");
                assert_eq!(
                    row_results(&analyses.closure_rows(
                        kind,
                        roots.clone(),
                        &[],
                        GenericCallInputs::default()
                    )),
                    expected,
                    "{context}"
                );
                let complete = expected.iter().cloned().collect::<Result<Vec<_>, _>>();
                assert_eq!(
                    analyses
                        .descriptor_closure(kind, roots)
                        .map(|rows| shapes(rows.iter())),
                    complete,
                    "{context}"
                );
            }
        }
    }
}

fn row_results(
    rows: &[Result<DescriptorMachine, DescriptorGraphError>],
) -> Vec<Result<String, DescriptorGraphError>> {
    rows.iter()
        .map(|row| row.as_ref().map(shape).map_err(|error| *error))
        .collect()
}

#[test]
fn whole_module_rows_match_the_free_module_rows() {
    for dynamic in [false, true] {
        let mut module = module(dynamic);
        let analyses = ModuleAnalyses::new(&module).unwrap();
        let every_machine = module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>();
        for kind in KINDS {
            // Machine 4's row fails and keeps its place in the list.
            let expected = terminal_graph_rows(&module, every_machine.clone(), kind, &[]);
            assert!(expected.iter().any(Result::is_err));
            assert_eq!(
                result_shapes(analyses.descriptor_rows(kind)),
                row_results(&expected),
                "dynamic {dynamic}, {kind:?}"
            );
        }
        drop(analyses);
        // Without the failing machine every row succeeds, as the free
        // whole-module rows require.
        module
            .machines
            .retain(|machine| machine.id != id(CALLS_MISSING));
        let analyses = ModuleAnalyses::new(&module).unwrap();
        for kind in KINDS {
            let expected = terminal_graph_for(&module, kind, &[]).unwrap();
            assert_eq!(expected.len(), module.machines.len());
            assert_eq!(
                result_shapes(analyses.descriptor_rows(kind)),
                result_shapes(expected.iter().map(Ok)),
                "dynamic {dynamic}, {kind:?}"
            );
        }
    }
}

#[test]
fn a_closure_reaching_a_failed_or_undeclared_machine_is_refused_as_the_free_closure_refuses_it() {
    for dynamic in [false, true] {
        let module = module(dynamic);
        let analyses = ModuleAnalyses::new(&module).unwrap();
        for kind in KINDS {
            for roots in [[id(CALLS_MISSING), id(ROOT)], [id(MISSING), id(ROOT)]] {
                let expected = terminal_graph_closure(&module, roots, kind, &[]).err();
                assert!(expected.is_some(), "dynamic {dynamic}, {kind:?}, {roots:?}");
                assert_eq!(
                    analyses.descriptor_closure(kind, roots).err(),
                    expected,
                    "dynamic {dynamic}, {kind:?}, {roots:?}"
                );
                // The per-row list keeps each failure beside the rows that succeeded.
                let rows = analyses.closure_rows(kind, roots, &[], GenericCallInputs::default());
                assert!(rows.iter().any(Result::is_ok));
                assert_eq!(
                    row_results(&rows),
                    row_results(&terminal_graph_rows(&module, roots, kind, &[])),
                    "dynamic {dynamic}, {kind:?}, {roots:?}"
                );
            }
            assert_eq!(
                analyses
                    .descriptor_closure(kind, [id(MISSING), id(ROOT)])
                    .err(),
                Some(DescriptorGraphError::MissingOrAmbiguousMachine(id(MISSING)))
            );
        }
    }
}

#[test]
fn a_module_that_declares_one_machine_twice_is_refused_as_the_free_closure_refuses_it() {
    let mut module = module(false);
    module.machines.push(machine(LEAF, Vec::new()));
    let expected = terminal_graph_closure(&module, [id(ROOT)], DescriptorSlotKind::Views, &[]);
    assert_eq!(
        expected.err(),
        Some(DescriptorGraphError::MissingOrAmbiguousMachine(id(LEAF)))
    );
    assert_eq!(
        ModuleAnalyses::new(&module).err(),
        Some(DescriptorGraphError::MissingOrAmbiguousMachine(id(LEAF)))
    );
}

#[test]
fn analyses_over_shared_rows_find_each_member_and_recognize_their_own_rows() {
    let module = module(false);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    let rows = analyses
        .descriptor_closure(DescriptorSlotKind::BorrowedStorage, [id(ROOT)])
        .unwrap();
    let other = analyses
        .descriptor_closure(DescriptorSlotKind::BorrowedStorage, [id(ROOT)])
        .unwrap();
    let types = &module.types.structural_types;
    let analysis =
        crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(types, &rows);
    assert!(analysis.uses_source_graph(types, &rows));
    // Equal rows held by another closure value are not these rows.
    assert!(!analysis.uses_source_graph(types, &other));
    assert_eq!(
        analysis
            .compatibility_machines()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![id(LEAF), id(ROOT), id(SELECTING)]
    );
}

#[test]
fn the_bundle_can_be_shared_between_threads() {
    fn shared<T: Sync>(_: &T) {}
    let module = module(true);
    let analyses = ModuleAnalyses::new(&module).unwrap();
    shared(&analyses);
    std::thread::scope(|scope| {
        for kind in KINDS {
            let analyses = &analyses;
            scope.spawn(move || {
                assert!(analyses.descriptor_closure(kind, [id(LEAF)]).is_ok());
            });
        }
    });
}
