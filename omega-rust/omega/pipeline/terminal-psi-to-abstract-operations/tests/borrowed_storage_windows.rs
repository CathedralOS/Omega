//! Borrowed-storage restoration windows consumed into abstract custody.
//!
//! `MoveStructuralField`/`StoreStructuralField` (Terminal codec tags 77/78)
//! arrive already verified: the Terminal verifier proved the restoration
//! debt, its overlap exclusions, and its closure on every non-crash exit.
//! These fixtures show the lowering transports that contract intact — the
//! complete borrowed parameter row, the spelled path and field, and the
//! exact result/value custody — into `AbstractOperation` variants, and that
//! the optimization-unit construction and semantic validation accept them
//! as structural state events rather than scalar or pure operations.

use abstract_operations::AbstractOperation;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    PsiSemanticId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_codec::{CodecError, encode_module, encode_proof_section};
use terminal_psi::{
    BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    VocabularyMarker,
};
use terminal_psi_to_abstract_operations::{ArtifactLoweringError, lower_artifact};
use terminal_verifier::{ModuleError, ProofBundle};

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
                    relevance: BindingRelevance::Relevant,
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
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                    StructuralFieldDeclaration {
                        id: right(),
                        identity: "right".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                ],
            },
        },
    ]
}

/// `MoveStructuralField` opening the `left` window on the borrowed envelope
/// parameter, producing the moved `Cell` at `result`.
fn extract(operation: OperationId, result: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
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
            source: id::<PlaceId>(1),
            path: Vec::new(),
            field: left(),
        },
    }
}

/// `StoreStructuralField` reseating the `left` window with `value`.
fn repair(value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id::<OperationId>(2),
        result: OperationResult::Unit,
        kind: OperationKind::StoreStructuralField {
            destination: id::<PlaceId>(1),
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

fn window_machine(access: StructuralAccess) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id::<MachineId>(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id::<PlaceId>(1),
            position: 0,
            is_self: false,
            structural_type: envelope(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: id::<PlaceId>(1),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: id::<PlaceId>(2),
                kind: StructuralPlaceKind::OperationResult {
                    producer: id::<OperationId>(1),
                    structural_type: cell(),
                },
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id::<BlockId>(1),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![extract(id::<OperationId>(1), 2), repair(2)],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(1),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: id::<ContractId>(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn window_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id::<MachineId>(1),
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
        machines: vec![window_machine(StructuralAccess::MutableBorrow)],
    }
}

fn lower(
    module: &TerminalModule,
) -> Result<abstract_operations::AbstractOperationPlan, ArtifactLoweringError> {
    let semantic = encode_module(module).expect("semantic module encodes");
    let proof = encode_proof_section(module, &ProofBundle::default()).expect("empty proof encodes");
    lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_plan())
}

#[test]
fn verified_window_pair_lowers_to_exact_abstract_spellings() {
    let plan = lower(&window_module()).expect("verified window pair lowers");
    let operations = &plan.functions[0].operations;
    // The pair plus the ReturnUnit terminator the abstract plan spells as an
    // operation.
    assert_eq!(operations.len(), 3);
    let moved = match &operations[0] {
        AbstractOperation::MoveStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => {
            assert_eq!(*psi_operation, id::<OperationId>(1));
            // The moved subtree keeps its fresh exact-once custody row.
            assert_eq!(result.place, id::<PlaceId>(2));
            assert_eq!(result.structural_type, cell());
            assert_eq!(result.multiplicity, StructuralMultiplicity::Unrestricted);
            assert!(result.claims.is_empty());
            // The root retains the complete borrowed parameter row, not a
            // bare place reconstructed from ABI shape.
            assert_eq!(source.place, id::<PlaceId>(1));
            assert_eq!(source.structural_type, envelope());
            assert_eq!(source.access, StructuralAccess::MutableBorrow);
            assert_eq!(source.multiplicity, StructuralMultiplicity::Unrestricted);
            assert!(path.is_empty());
            assert_eq!(*field, left());
            result.place
        }
        other => panic!("expected MoveStructuralField, got {other:?}"),
    };
    match &operations[1] {
        AbstractOperation::StoreStructuralField {
            psi_operation,
            destination,
            path,
            field,
            value,
        } => {
            assert_eq!(*psi_operation, id::<OperationId>(2));
            assert_eq!(destination.place, id::<PlaceId>(1));
            assert_eq!(destination.access, StructuralAccess::MutableBorrow);
            assert!(path.is_empty());
            assert_eq!(*field, left());
            // The repair consumes the exact moved place, owned, whole.
            assert_eq!(value.place, moved);
            assert_eq!(value.access, StructuralAccess::Owned);
            assert!(value.path.is_empty());
        }
        other => panic!("expected StoreStructuralField, got {other:?}"),
    }
    assert!(matches!(
        operations[2],
        AbstractOperation::ReturnUnit { .. }
    ));
}

#[test]
fn window_pair_builds_and_validates_its_optimization_unit() {
    let module = window_module();
    let semantic = encode_module(&module).expect("semantic module encodes");
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .expect("window pair projects into optimizer input");
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("window pair constructs a verified unit");
    let unit = verified.unit();
    optimization_unit_semantics::validate_psi_optimization_unit(unit)
        .expect("window unit validates");
    // Provenance keeps the Terminal operation identities through unit
    // construction for both sides of the window.
    let nodes = &unit.functions[0].blocks[0].nodes;
    for (node, expected) in nodes.iter().zip([1u64, 2]) {
        assert!(
            node.provenance.iter().any(|row| matches!(
                row,
                optimization_unit::PsiProvenance::Operation(operation)
                    if *operation == id::<OperationId>(expected)
            )),
            "node lost its Terminal operation provenance"
        );
    }
    // The canonical identity binds the window: renaming the field changes
    // the recomputed unit identity, so the hole cannot silently move.
    let mut changed = unit.clone();
    match &mut changed.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::MoveStructuralField { field, .. } => *field = right(),
        _ => unreachable!(),
    }
    assert_ne!(
        optimization_unit::recompute_psi_optimization_unit_identity(&changed),
        unit.identity,
        "field rename must change the canonical unit identity"
    );
}

#[test]
fn a_replacement_subtree_repairs_and_lowers() {
    // The debt is keyed on the hole, not the moved place: a freshly
    // established Cell of the exact declared type reseats it just as well.
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id::<PlaceId>(3),
        kind: StructuralPlaceKind::OperationResult {
            producer: id::<OperationId>(3),
            structural_type: cell(),
        },
    });
    let operations = &mut machine.blocks[0].operations;
    operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id::<OperationId>(5),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
                id: id::<ValueId>(1),
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: false },
        },
    );
    operations.insert(
        2,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id::<OperationId>(3),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: id::<PlaceId>(3),
                structural_type: cell(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishRecord {
                fields: vec![terminal_psi::RecordFieldInitializer {
                    field: flag(),
                    value: RecordFieldValue::Scalar {
                        value: id::<ValueId>(1),
                        range_obligation: None,
                    },
                }],
            },
        },
    );
    let OperationKind::StoreStructuralField { value, .. } = &mut operations[3].kind else {
        unreachable!()
    };
    value.place = id::<PlaceId>(3);
    let plan = lower(&module).expect("replacement repair lowers");
    let operations = &plan.functions[0].operations;
    let store = operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::StoreStructuralField { value, .. } => Some(value),
            _ => None,
        })
        .expect("store survives lowering");
    assert_eq!(store.place, id::<PlaceId>(3));
    assert_eq!(store.access, StructuralAccess::Owned);
}

#[test]
fn a_sibling_observation_inside_the_window_lowers() {
    // While `left` is absent the disjoint `right` subtree stays observable;
    // the window leg must transport both operations without merging them.
    let mut module = window_module();
    module.machines[0].blocks[0].operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id::<OperationId>(4),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
                id: id::<ValueId>(1),
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: id::<PlaceId>(1),
                path: vec![CanonicalStructuralPathSegment::Field(right())],
                field: flag(),
            },
        },
    );
    let plan = lower(&module).expect("sibling read inside the window lowers");
    let operations = &plan.functions[0].operations;
    assert!(matches!(
        operations[0],
        AbstractOperation::MoveStructuralField { .. }
    ));
    assert!(matches!(
        operations[1],
        AbstractOperation::BooleanStructuralField { .. }
    ));
    assert!(matches!(
        operations[2],
        AbstractOperation::StoreStructuralField { .. }
    ));
}

/// Malformed windows fail before the abstract leg ever sees them: the
/// codec's structural-foundation check and the verifier's debt replay both
/// run inside `lower_artifact` admission. Assert the exact rejection so a
/// silently widened contract cannot hide behind a generic error.
#[test]
fn an_unrepaired_window_never_reaches_abstract_form() {
    let mut module = window_module();
    module.machines[0].blocks[0].operations.pop();
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            ModuleError::BorrowedStorageRestorationPending { .. }
        ))
    ));
}

#[test]
fn a_shared_borrow_root_never_opens_a_window() {
    let mut module = window_module();
    module.machines[0] = window_machine(StructuralAccess::SharedBorrow);
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(message))
            if message.contains("mutable-borrowed")
    ));
}

#[test]
fn a_borrowed_value_cannot_repair_the_window() {
    let mut module = window_module();
    let OperationKind::StoreStructuralField { value, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    value.access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(message))
            if message.contains("owned")
    ));
}

#[test]
fn a_store_into_a_live_field_rejects() {
    // The store names the hole exactly: reseating `right` — which was never
    // vacated — is not a window repair.
    let mut module = window_module();
    let OperationKind::StoreStructuralField { field, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *field = right();
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            ModuleError::BorrowedStorageRepairMismatch { .. }
        ))
    ));
}
