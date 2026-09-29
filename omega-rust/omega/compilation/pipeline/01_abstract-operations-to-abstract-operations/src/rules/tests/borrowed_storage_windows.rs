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

/// A window pair in the member block of a self-looping machine. The Terminal
/// verifier admits it: the frontier replay compares every arrival's open
/// windows and owned places with the block's established entry, and the
/// unrestricted displaced value owes no disposal. The member block also holds
/// one dead constant leaf, the only work loop-invariant motion may relocate.
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
    machine.blocks[1]
        .operations
        .extend([extract(42, 12), repair(43, 12)]);
    cyclic
}

/// Admit `module` through the real optimizer boundary.
fn admitted_session(module: &TerminalModule) -> crate::VerifiedPsiOptimizationSession {
    let semantic =
        terminal_codec::encode_module(module).expect("the verifier admits the cyclic window");
    let proof =
        terminal_codec::encode_proof_section(module, &terminal_verifier::ProofBundle::default())
            .expect("empty proof section");
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .expect("optimizer admission");
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("verified optimization unit");
    crate::VerifiedPsiOptimizationSession::new(verified).expect("cyclic window session")
}

/// The block holding each window operation and its position there.
fn window_locations(session: &crate::VerifiedPsiOptimizationSession) -> Vec<(BlockId, usize)> {
    session
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| {
                    matches!(
                        node.operation,
                        abstract_operations::AbstractOperation::MoveStructuralField { .. }
                            | abstract_operations::AbstractOperation::StoreStructuralField { .. }
                    )
                })
                .map(move |(position, _)| (block.id, position))
        })
        .collect()
}

/// Loop-invariant motion sees the window in a member block and keeps it
/// there: the pair is structural custody, and a component holding it
/// preserves no place observation, so only the dead constant leaf may move.
/// The move still precedes the store in the member block afterwards.
#[test]
fn loop_invariant_motion_keeps_a_cyclic_window_in_its_member_block() {
    let session = admitted_session(&window_inside_cycle_module());
    let member = id::<BlockId>(73);
    let before = window_locations(&session);
    assert!(
        matches!(before.as_slice(), [(first, move_at), (second, store_at)]
            if *first == member && *second == member && move_at < store_at),
        "the pair enters the optimizer in its member block: {before:?}"
    );
    let candidates =
        crate::propose_loop_invariant_scalar_motion(&session, 8).expect("loop-invariant proposal");
    let [candidate] = candidates.as_slice() else {
        panic!("the one cyclic component yields one candidate: {candidates:?}")
    };
    let moved = candidate
        .relocations()
        .iter()
        .map(|relocation| relocation.node().psi_operation())
        .collect::<Vec<_>>();
    assert_eq!(
        moved,
        vec![id::<OperationId>(74)],
        "only the dead constant leaf relocates"
    );
    let validated = crate::validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let session = crate::apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application")
        .into_session();
    let after = window_locations(&session);
    assert!(
        matches!(after.as_slice(), [(first, move_at), (second, store_at)]
            if *first == member && *second == member && move_at < store_at),
        "the pair stays in its member block in order: {after:?}"
    );
}
