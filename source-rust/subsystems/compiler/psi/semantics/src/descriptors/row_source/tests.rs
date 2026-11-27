//! A builder must answer the same whether it reads a caller's bundle or the
//! bare module. Two twins differ only in machine 2's body: in the static
//! twin it returns at once, in the dynamic twin it selects an implementation
//! and calls it dynamically, so a closure that reaches it owns rows adapted
//! against that closure. Machine 4 takes a borrowed reference and calls
//! machine 2, so the borrowed-storage builders have a closure to build.

use super::*;
use crate::descriptors::descriptor_results::{
    DescriptorInvocationTarget, terminal_graph_closure, terminal_graph_rows,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, OperationId, PlaceId,
    ScalarQualificationSetId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineResult, BoundaryParameterKind, MachineContract, Operation, OperationKind,
    OperationResult, RegisterTargetApplicability, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable,
    TerminalIdentityIndex, TerminalMachine, TerminalMachineResult, Terminator, ValueDeclaration,
};

const ROOT: u64 = 1;
const SELECTING: u64 = 2;
const LEAF: u64 = 3;
const BORROWER: u64 = 4;
const REALIZATION: u64 = 5;
const CALLS_MISSING: u64 = 6;
const MISSING: u64 = 9;
const SELECTOR: u64 = 21;
const SCALAR: u64 = 1;
const REFERENCE: u64 = 2;

const KINDS: [DescriptorSlotKind; 4] = [
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

/// Takes a shared reference to a scalar and calls the selecting machine.
fn borrower() -> TerminalMachine {
    let mut body = machine(BORROWER, vec![call(10, SELECTING)]);
    let place = PlaceId::new(1).unwrap();
    body.contract.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        structural_type: StructuralTypeId::new(REFERENCE).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }];
    body.contract.parameter_order = vec![BoundaryParameterKind::Structural];
    body.structural_places = vec![StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter { position: 0 },
    }];
    body
}

fn types() -> Vec<StructuralTypeDeclaration> {
    let scalar = StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: StructuralTypeId::new(SCALAR).unwrap(),
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )),
        scalar_restriction: None,
    };
    let reference = StructuralTypeDeclaration {
        id: StructuralTypeId::new(REFERENCE).unwrap(),
        identity: TerminalIdentityIndex(2),
        shape: StructuralTypeShape::Reference {
            referent: scalar.id,
            access: StructuralAccess::SharedBorrow,
        },
        ..scalar.clone()
    };
    vec![scalar, reference]
}

/// Machines are declared in identity order, as the reference-end builder
/// requires. Machine 5 is the only realization of the dynamic slot.
fn module(dynamic: bool) -> TerminalModule {
    let mut module = TerminalModule::for_entry(id(ROOT));
    module.types.structural_types = types();
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
    module.machines = vec![
        machine(ROOT, vec![call(1, SELECTING), call(2, LEAF)]),
        machine(
            SELECTING,
            if dynamic { dynamic_call(3) } else { Vec::new() },
        ),
        machine(LEAF, Vec::new()),
        borrower(),
        machine(REALIZATION, Vec::new()),
    ];
    module
}

/// `module`, plus a machine whose call names an undeclared machine, so its
/// row fails, with the machines out of identity order.
fn module_with_failure(dynamic: bool) -> TerminalModule {
    let mut module = module(dynamic);
    module
        .machines
        .push(machine(CALLS_MISSING, vec![call(5, MISSING)]));
    module.machines.reverse();
    module
}

fn twins() -> Vec<(&'static str, TerminalModule)> {
    vec![
        ("static", module(false)),
        ("dynamic", module(true)),
        ("static with a failed row", module_with_failure(false)),
        ("dynamic with a failed row", module_with_failure(true)),
    ]
}

fn root_sets() -> Vec<Vec<MachineId>> {
    vec![
        vec![id(ROOT)],
        vec![id(LEAF)],
        vec![id(BORROWER)],
        vec![id(ROOT), id(BORROWER)],
        vec![id(CALLS_MISSING)],
        vec![id(MISSING)],
        Vec::new(),
    ]
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

fn closure_shapes(
    rows: Result<DescriptorRows<'_>, DescriptorGraphError>,
) -> Result<Vec<String>, DescriptorGraphError> {
    rows.map(|rows| shapes(rows.iter()))
}

#[test]
fn closures_from_a_bundle_or_the_bare_module_match_the_free_closures() {
    for (twin, module) in twins() {
        let analyses = ModuleAnalyses::new(&module).unwrap();
        let shared = DescriptorRowSource::of(&analyses);
        let alone = DescriptorRowSource::new(&module);
        for kind in KINDS {
            for roots in root_sets() {
                let expected = terminal_graph_closure(&module, roots.iter().copied(), kind, &[])
                    .map(|rows| shapes(&rows));
                assert_eq!(
                    closure_shapes(shared.closure(kind, roots.iter().copied())),
                    expected,
                    "{twin} {kind:?} {roots:?} from the caller's bundle"
                );
                assert_eq!(
                    closure_shapes(alone.closure(kind, roots.iter().copied())),
                    expected,
                    "{twin} {kind:?} {roots:?} from the bare module"
                );
            }
        }
    }
}

#[test]
fn successful_rows_and_module_rows_match_the_free_rows() {
    for (twin, module) in twins() {
        let analyses = ModuleAnalyses::new(&module).unwrap();
        let shared = DescriptorRowSource::of(&analyses);
        let alone = DescriptorRowSource::new(&module);
        for kind in KINDS {
            for roots in root_sets() {
                let expected = terminal_graph_rows(&module, roots.iter().copied(), kind, &[])
                    .into_iter()
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                let expected = shapes(&expected);
                for (source, rows) in [("bundle", &shared), ("module", &alone)] {
                    assert_eq!(
                        shapes(rows.successful_rows(kind, roots.iter().copied()).iter()),
                        expected,
                        "{twin} {kind:?} {roots:?} successful rows from the {source}"
                    );
                }
            }
            let expected = terminal_graph_for(&module, kind, &[]).map(|rows| shapes(&rows));
            for (source, rows) in [("bundle", &shared), ("module", &alone)] {
                assert_eq!(
                    rows.module_graph(kind).map(|rows| shapes(&rows)),
                    expected,
                    "{twin} {kind:?} module rows from the {source}"
                );
            }
        }
    }
}

#[test]
fn a_module_that_declares_one_machine_twice_is_refused_as_the_free_closures_refuse_it() {
    let mut module = module(false);
    module.machines.push(machine(LEAF, Vec::new()));
    assert!(ModuleAnalyses::new(&module).is_err());
    let alone = DescriptorRowSource::new(&module);
    for kind in KINDS {
        for roots in root_sets() {
            let expected = terminal_graph_closure(&module, roots.iter().copied(), kind, &[])
                .map(|rows| shapes(&rows));
            assert!(expected.is_err());
            assert_eq!(
                closure_shapes(alone.closure(kind, roots.iter().copied())),
                expected
            );
            assert!(
                alone
                    .successful_rows(kind, roots.iter().copied())
                    .is_empty()
            );
        }
        assert!(terminal_graph_for(&module, kind, &[]).is_none());
        assert!(alone.module_graph(kind).is_none());
    }
}

#[test]
fn closures_of_one_source_share_the_bundle_it_builds() {
    let module = module(false);
    let alone = DescriptorRowSource::new(&module);
    let kind = DescriptorSlotKind::BorrowedStorage;
    let first = alone.closure(kind, [id(ROOT)]).unwrap();
    let second = alone.closure(kind, [id(BORROWER)]).unwrap();
    let (
        DescriptorRows::Shared {
            family: first_family,
            ..
        },
        DescriptorRows::Shared {
            family: second_family,
            ..
        },
    ) = (&first, &second)
    else {
        panic!("closures without a dynamic call share their rows");
    };
    assert!(std::ptr::eq(*first_family, *second_family));

    let module = self::module(true);
    let alone = DescriptorRowSource::new(&module);
    assert!(matches!(
        alone.closure(kind, [id(BORROWER)]).unwrap(),
        DescriptorRows::Owned(_)
    ));
    assert!(matches!(
        alone.closure(kind, [id(LEAF)]).unwrap(),
        DescriptorRows::Shared { .. }
    ));
}

/// Candidates print but do not copy, so answers compare as text.
type CandidateAnswer = Result<Vec<(MachineId, String)>, (MachineId, Option<OperationId>, String)>;

fn candidate_answer(
    answer: Result<
        arena::IdTable<MachineId, crate::reference_ends::Candidates>,
        crate::reference_ends::Error,
    >,
) -> CandidateAnswer {
    answer
        .map(|table| {
            table
                .iter()
                .map(|(machine, candidates)| (machine, format!("{candidates:?}")))
                .collect()
        })
        .map_err(|error| (error.machine, error.operation, error.detail))
}

#[test]
fn builders_reading_a_bundle_answer_as_their_module_entry_points() {
    for (twin, module) in twins() {
        let analyses = ModuleAnalyses::new(&module).unwrap();
        assert_eq!(
            candidate_answer(crate::reference_ends::candidates_in(&analyses)),
            candidate_answer(crate::reference_ends::candidates(&module)),
            "{twin} reference ends"
        );
        assert_eq!(
            crate::structural_qualifications::validate_in(&analyses),
            crate::structural_qualifications::validate(&module),
            "{twin} structural qualifications"
        );
        assert_eq!(
            crate::calls::callable_refinement::validate_callable_result_refinements_in(&analyses),
            crate::calls::callable_refinement::validate_callable_result_refinements(&module),
            "{twin} callable result refinements"
        );
        assert_eq!(
            crate::references::validate_provider_reference_outputs_in(&analyses),
            crate::references::validate_provider_reference_outputs(&module),
            "{twin} provider reference outputs"
        );
        let value_types = std::collections::BTreeMap::<ValueId, ScalarType>::new();
        for body in &module.machines {
            assert_eq!(
                crate::target_scalar_ranges_in(&analyses, body, &value_types),
                crate::target_scalar_ranges(&module, body, &value_types),
                "{twin} target ranges of {:?}",
                body.id
            );
            assert_eq!(
                crate::layout_call_conditions_in(&analyses, body),
                crate::layout_call_conditions(&module, body),
                "{twin} layout call conditions of {:?}",
                body.id
            );
            assert_eq!(
                crate::array_extent_literal_conditions_in(&analyses, body),
                crate::array_extent_literal_conditions(&module, body),
                "{twin} array extent conditions of {:?}",
                body.id
            );
        }
    }
}

/// The reference-end builder reaches the borrowing machine's callees, so
/// its rows depend on whether machine 2 calls dynamically.
#[test]
fn reference_ends_read_the_borrowing_closure_from_the_bundle() {
    for dynamic in [false, true] {
        let module = module(dynamic);
        let analyses = ModuleAnalyses::new(&module).unwrap();
        let answer = candidate_answer(crate::reference_ends::candidates_in(&analyses));
        assert_eq!(
            answer,
            candidate_answer(crate::reference_ends::candidates(&module))
        );
        let candidates = answer.unwrap_or_else(|error| {
            panic!("dynamic {dynamic}: reference ends refused the fixture: {error:?}")
        });
        assert_eq!(candidates.len(), module.machines.len());
    }
}
