//! Borrowed-storage restoration windows under optimization.
//!
//! `MoveStructuralField`/`StoreStructuralField` arrive verified: the Terminal
//! verifier already proved the restoration debt, its overlap exclusions, and
//! its closure on every non-crash exit. Optimization may not touch that
//! contract — the pair is structural-state custody, never a scalar
//! computation: no rule may scalar-substitute, dead-eliminate,
//! common-subexpression, fold, reorder, or hoist either side. These tests pin
//! that conservatism end to end: every selected Psi pass declines the
//! window-bearing unit, publication replays to a byte-identical unit, and the
//! standalone ranked/specialization boundaries keep the pair in place and in
//! order while relocating only genuinely admissible scalar work around it.

use abstract_operations::AbstractOperation;
use optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, PsiSemanticId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;
use terminal_verifier::ProofBundle;

use crate::{
    VerifiedPsiOptimizationSession, apply_loop_invariant_scalar_motion,
    optimize_abstract_operations, propose_case_membership_specializations,
    propose_countdown_invariant_constant_relocations, propose_loop_invariant_scalar_motion,
    publish_optimization_run, run_psi_pipeline, validate_loop_invariant_scalar_motion,
};

fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
    Identity::new(raw).expect("test identity is nonzero")
}

fn cell() -> StructuralTypeId {
    id::<StructuralTypeId>(1)
}

fn envelope() -> StructuralTypeId {
    id::<StructuralTypeId>(2)
}

fn flag() -> StructuralFieldId {
    id::<StructuralFieldId>(1)
}

fn left() -> StructuralFieldId {
    id::<StructuralFieldId>(1)
}

fn right() -> StructuralFieldId {
    id::<StructuralFieldId>(2)
}

fn structural_types() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: cell(),
            identity: "Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: flag(),
                    identity: "flag".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: envelope(),
            identity: "Envelope".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: left(),
                        identity: "left".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                    StructuralFieldDeclaration {
                        id: right(),
                        identity: "right".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                ],
            },
        },
    ]
}

/// `MoveStructuralField` opening the `left` window on the borrowed envelope
/// parameter, producing the moved `Cell` at `result`.
fn extract(operation: u64, result: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id::<OperationId>(operation),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: id::<PlaceId>(result),
            structural_type: cell(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::MoveStructuralField {
            source: id::<PlaceId>(11),
            path: Vec::new(),
            field: left(),
        },
    }
}

/// `StoreStructuralField` reseating the `left` window with `value`.
fn repair(operation: u64, value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id::<OperationId>(operation),
        result: OperationResult::Unit,
        kind: OperationKind::StoreStructuralField {
            destination: id::<PlaceId>(11),
            path: Vec::new(),
            field: left(),
            value: StructuralArgument {
                place: id::<PlaceId>(value),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        },
    }
}

fn window_machine(blocks: Vec<Block>) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id::<MachineId>(10),
        attachment: None,
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: id::<ValueId>(50),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
        }],
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id::<PlaceId>(11),
            position: 0,
            is_self: false,
            structural_type: envelope(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: id::<PlaceId>(11),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: id::<PlaceId>(12),
                kind: StructuralPlaceKind::OperationResult {
                    producer: id::<OperationId>(42),
                    structural_type: cell(),
                },
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id::<BlockId>(20),
        blocks,
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: id::<ContractId>(60),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn module(machine: TerminalMachine) -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id::<MachineId>(10),
        structural_types: structural_types(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    }
}

/// The minimal closed window: one block extracts `left`, immediately reseats
/// it with the same subtree, and returns. No scalar work exists for any pass
/// to commit on, so a correct run leaves the unit byte-identical.
fn window_module() -> TerminalModule {
    module(window_machine(vec![Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id::<BlockId>(20),
        parameters: Vec::new(),
        operations: vec![extract(42, 12), repair(43, 12)],
        terminator: Terminator::ReturnUnit {
            edge: id::<EdgeId>(30),
            trivial_affine_discards: Vec::new(),
        },
    }]))
}

/// A self-looping scalar machine with one dead constant leaf — the shape the
/// unranked-cycle fixtures use to give loop-invariant motion something real
/// to relocate.
fn cycle_machine() -> TerminalMachine {
    let successor = |edge, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: id::<EdgeId>(edge),
        target: id::<BlockId>(target),
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id::<MachineId>(70),
        attachment: None,
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: id::<ValueId>(80),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
        }],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id::<BlockId>(71),
        blocks: vec![
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: id::<BlockId>(71),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    edge: id::<EdgeId>(72),
                    target: id::<BlockId>(73),
                    arguments: vec![id::<ValueId>(80)],
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: id::<BlockId>(73),
                parameters: vec![ValueDeclaration {
                    qualifications: Default::default(),
                    id: id::<ValueId>(81),
                    scalar_type: semantic_vocabulary::ScalarType::Boolean,
                }],
                operations: vec![Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: id::<OperationId>(74),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: id::<ValueId>(82),
                        scalar_type: semantic_vocabulary::ScalarType::Integer(unsigned_64),
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                }],
                terminator: Terminator::Conditional {
                    condition: id::<ValueId>(81),
                    when_true: successor(75, 73, vec![id::<ValueId>(81)]),
                    when_false: successor(76, 77, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: id::<BlockId>(77),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: id::<EdgeId>(78),
                    trivial_affine_discards: Vec::new(),
                },
            },
        ],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: id::<ContractId>(79),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

/// One module holding the acyclic window machine beside the cyclic scalar
/// machine: the loop boundary has real work to do while the window rides
/// along in the same unit.
fn window_cycle_module() -> TerminalModule {
    let mut combined = module(window_machine(vec![Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id::<BlockId>(20),
        parameters: Vec::new(),
        operations: vec![extract(42, 12), repair(43, 12)],
        terminator: Terminator::ReturnUnit {
            edge: id::<EdgeId>(30),
            trivial_affine_discards: Vec::new(),
        },
    }]));
    combined.machines.push(cycle_machine());
    combined
}

/// The window operations cannot be placed inside a cyclic machine at all:
/// the Terminal verifier's cycle-eligibility fence admits only scalar-result
/// and explicitly whitelisted custody shapes, so the borrowed-parameter
/// window is unrepresentable in a loop member by construction. Encoding must
/// fail before the optimizer ever sees the shape — this module documents
/// that fence.
fn window_inside_cycle_module() -> TerminalModule {
    let mut cyclic = module(cycle_machine());
    cyclic.entry = id::<MachineId>(70);
    let machine = &mut cyclic.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: id::<PlaceId>(11),
        position: 0,
        is_self: false,
        structural_type: envelope(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id::<PlaceId>(11),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id::<PlaceId>(12),
            kind: StructuralPlaceKind::OperationResult {
                producer: id::<OperationId>(42),
                structural_type: cell(),
            },
        },
    ];
    machine.blocks[1].operations = vec![extract(42, 12), repair(43, 12)];
    cyclic
}

/// Encode, admit, and build the verified optimization unit through the real
/// artifact boundary — the same route production input takes.
fn verified_unit(module: &TerminalModule) -> VerifiedPsiOptimizationUnit {
    let semantic = terminal_codec::encode_module(module).expect("semantic module encodes");
    let proof = terminal_codec::encode_proof_section(module, &ProofBundle::default())
        .expect("proof encodes");
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .expect("window module admits into optimizer input");
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("window module builds a verified unit")
}

/// The window operations of `unit` as `(operation, node)` rows in visitation
/// order — the value every optimization boundary must preserve.
fn window_operations(
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Vec<(semantic_vocabulary::OperationId, &AbstractOperation)> {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter_map(|node| {
            let operation = match node.provenance.first() {
                Some(optimization_unit::PsiProvenance::Operation(operation)) => *operation,
                _ => return None,
            };
            matches!(
                node.operation,
                AbstractOperation::MoveStructuralField { .. }
                    | AbstractOperation::StoreStructuralField { .. }
            )
            .then_some((operation, &node.operation))
        })
        .collect()
}

/// The pinned contract: exactly one `MoveStructuralField` immediately followed
/// by its `StoreStructuralField`, both byte-identical to the expected pair and
/// still in the same block.
fn assert_window_pair(
    unit: &optimization_unit::PsiOptimizationUnit,
    expected_move: &AbstractOperation,
    expected_store: &AbstractOperation,
) {
    let windows = window_operations(unit);
    let [(_, moved), (_, stored)] = windows.as_slice() else {
        panic!(
            "expected exactly one move/store pair, found {}",
            windows.len()
        )
    };
    assert_eq!(*moved, expected_move, "the move must stay byte-exact");
    assert_eq!(*stored, expected_store, "the store must stay byte-exact");
    let positions = unit
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .map(move |(index, node)| (block.id, index, node))
        })
        .filter_map(|(block, index, node)| {
            matches!(
                node.operation,
                AbstractOperation::MoveStructuralField { .. }
                    | AbstractOperation::StoreStructuralField { .. }
            )
            .then_some((block, index))
        })
        .collect::<Vec<_>>();
    let [(move_block, move_index), (store_block, store_index)] = positions.as_slice() else {
        unreachable!("window_operations found exactly two")
    };
    assert_eq!(move_block, store_block, "the pair must stay in one block");
    assert_eq!(
        *move_index + 1,
        *store_index,
        "the store must still immediately follow its move"
    );
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(96, 64, 64, 64, 8).unwrap()
}

/// Every Psi selection the pass catalog owns. `PSI_PASS_CATALOG` order is the
/// canonical schedule order.
const PSI_SELECTIONS: [Optimization; 8] = [
    Optimization::SparseConditionalConstantPropagation,
    Optimization::ControlFlowCleanup,
    Optimization::CopyPropagation,
    Optimization::GlobalValueNumbering,
    Optimization::ProofCheckElision,
    Optimization::DeadPureScalarElimination,
    Optimization::StateSpecialization,
    Optimization::RepresentationSpecialization,
];

#[test]
fn every_selected_psi_pass_leaves_the_window_pair_untouched() {
    let unit = verified_unit(&window_module());
    let windows = window_operations(unit.unit());
    let [(_, expected_move), (_, expected_store)] = windows.as_slice() else {
        panic!("the fixture carries exactly one window pair")
    };
    let (expected_move, expected_store) = ((*expected_move).clone(), (*expected_store).clone());
    for selection in PSI_SELECTIONS {
        let selections = OptimizationSelections::new([selection]).unwrap();
        let run = run_psi_pipeline(unit.clone(), &selections, budget()).unwrap();
        assert!(
            run.commits().is_empty(),
            "{:?} proposed work on a window-only machine",
            selection
        );
        let plan = publish_optimization_run(run).unwrap();
        assert_eq!(
            plan.unit().identity,
            unit.unit().identity,
            "{:?} changed the unit identity",
            selection
        );
        assert_window_pair(plan.unit(), &expected_move, &expected_store);
    }
}

#[test]
fn the_complete_psi_schedule_preserves_the_window_pair() {
    let unit = verified_unit(&window_module());
    let windows = window_operations(unit.unit());
    let [(_, expected_move), (_, expected_store)] = windows.as_slice() else {
        panic!("the fixture carries exactly one window pair")
    };
    let (expected_move, expected_store) = ((*expected_move).clone(), (*expected_store).clone());
    let selections = OptimizationSelections::new(PSI_SELECTIONS).unwrap();
    let plan = optimize_abstract_operations(
        unit.input().clone(),
        &selections,
        &selections.project_psi(),
        budget(),
    )
    .unwrap();
    assert!(
        plan.commits().is_empty(),
        "no scalar optimization may fire on a window-only machine"
    );
    assert_window_pair(plan.unit(), &expected_move, &expected_store);
}

/// A window inside a cyclic machine is unrepresentable upstream: the
/// verifier's cycle-eligibility fence admits only scalar-result operations
/// and explicitly whitelisted custody shapes, so the borrowed-parameter pair
/// is rejected at encoding. This is the fence that keeps loop-invariant
/// motion from ever seeing a window operation in a member block.
#[test]
fn a_window_inside_a_cycle_is_not_representable() {
    let module = window_inside_cycle_module();
    assert!(
        matches!(
            terminal_codec::encode_module(&module),
            Err(terminal_codec::CodecError::InvalidModule(
                terminal_verifier::ModuleError::ControlCycle(block)
            )) if block == id::<BlockId>(73)
        ),
        "the cyclic window machine must fail cycle eligibility"
    );
}

/// Loop-invariant scalar motion is a standalone session boundary, not a pass:
/// its admission whitelist cannot name a window operation, and retained nodes
/// compare byte-exact. Relocating the cyclic machine's admissible leaf leaves
/// the sibling window machine's pair in place and in order.
#[test]
fn loop_invariant_scalar_motion_preserves_the_window_pair() {
    let unit = verified_unit(&window_cycle_module());
    let session = VerifiedPsiOptimizationSession::new(unit).expect("window cycle session");
    let (component_machine, member) = {
        let [component] = session.cycle_components().components() else {
            panic!("one validated Terminal SCC for the scalar loop")
        };
        (component.id.machine, component.members[0])
    };
    assert_eq!(component_machine, id::<MachineId>(70));
    let candidates = propose_loop_invariant_scalar_motion(&session, 8).unwrap();
    let [candidate] = candidates.as_slice() else {
        panic!("one candidate for the single-entry component")
    };
    let window = [id::<OperationId>(42), id::<OperationId>(43)];
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| !window.contains(&relocation.node().psi_operation())),
        "a restoration-window node must never appear in a relocation plan"
    );
    let leaf = id::<OperationId>(74);
    let [relocation] = candidate.relocations() else {
        panic!("the dead integer constant is the one admissible leaf")
    };
    assert_eq!(relocation.node().psi_operation(), leaf);

    let expected = window_operations(session.unit())
        .into_iter()
        .map(|(_, operation)| operation.clone())
        .collect::<Vec<_>>();
    let validated = validate_loop_invariant_scalar_motion(&session, candidate).unwrap();
    let applied = apply_loop_invariant_scalar_motion(session, validated).unwrap();
    let output = applied.session().unit();
    let [expected_move, expected_store] = expected.as_slice() else {
        unreachable!()
    };
    assert_window_pair(output, expected_move, expected_store);
    // The leaf landed in the component's preheader while the window machine's
    // function was never in the component at all.
    let cyclic = output
        .functions
        .iter()
        .find(|function| function.machine == id::<MachineId>(70))
        .expect("the cyclic machine survives");
    assert!(
        cyclic
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("the member block survives")
            .nodes
            .iter()
            .all(|node| node.provenance.first()
                != Some(&optimization_unit::PsiProvenance::Operation(leaf))),
        "the leaf left the member block"
    );
    assert!(
        cyclic
            .blocks
            .iter()
            .find(|block| block.id != member)
            .is_some_and(|block| {
                block.nodes.iter().any(|node| {
                    node.provenance.first()
                        == Some(&optimization_unit::PsiProvenance::Operation(leaf))
                })
            }),
        "the leaf relocated into the component's preheader"
    );
}

/// The standalone specialization boundaries see only closed rosters and
/// certified countdown components: a record-shaped window machine beside an
/// unranked cycle offers neither, so both decline outright.
#[test]
fn specialization_boundaries_decline_around_the_window() {
    let unit = verified_unit(&window_cycle_module());
    let session = VerifiedPsiOptimizationSession::new(unit).expect("window cycle session");
    assert!(
        propose_case_membership_specializations(&session, 8)
            .unwrap()
            .is_empty(),
        "no sole-case roster exists to fold"
    );
    assert!(
        propose_countdown_invariant_constant_relocations(&session, 8)
            .unwrap()
            .is_empty(),
        "the unranked component carries no countdown certificate"
    );
}
