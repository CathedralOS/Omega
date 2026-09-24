//! Read-only leaf copies out of borrowed roots consumed into abstract custody.
//!
//! `StructuralLeafCopy` (Terminal codec tag 84) arrives already verified: the
//! Terminal verifier proved the source is a readable root, the spelled path
//! resolves to the result type, and the copy observes contents where a move
//! would vacate borrowed storage. These fixtures show the lowering transports
//! that contract intact into the `AbstractOperation` variant and that the
//! optimization-unit construction and semantic validation accept it as a
//! structural state event rather than a scalar or pure operation.

use abstract_operations::AbstractOperation;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, PsiSemanticId, StructuralFieldId,
    StructuralPlaceKind, StructuralTypeId,
};
use terminal_codec::{CodecError, encode_module, encode_proof_section};
use terminal_psi::{
    BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
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

/// `StructuralLeafCopy` projecting `left` out of the borrowed envelope
/// parameter, producing an owned `Cell` at `result`.
fn copy_leaf(operation: OperationId, result: u64) -> Operation {
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
        kind: OperationKind::StructuralLeafCopy {
            source: id::<PlaceId>(1),
            path: vec![StructuralPathSegment::Field("left".into())],
        },
    }
}

fn copy_machine(access: StructuralAccess) -> TerminalMachine {
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
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: id::<PlaceId>(3),
            structural_type: cell(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
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
            StructuralPlaceDeclaration {
                id: id::<PlaceId>(3),
                kind: StructuralPlaceKind::Result,
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
            operations: vec![copy_leaf(id::<OperationId>(1), 2)],
            terminator: Terminator::ReturnStructural {
                edge: id::<EdgeId>(1),
                source: id::<PlaceId>(2),
                returned_claims: Vec::new(),
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

fn copy_module(access: StructuralAccess) -> TerminalModule {
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
        machines: vec![copy_machine(access)],
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
    .map(|admitted| admitted.into_plan())
}

#[test]
fn verified_leaf_copy_lowers_to_exact_abstract_spelling() {
    let plan =
        lower(&copy_module(StructuralAccess::SharedBorrow)).expect("verified leaf copy lowers");
    let operations = &plan.functions[0].operations;
    // The copy plus the ReturnStructural terminator the abstract plan spells
    // as an operation.
    assert_eq!(operations.len(), 2);
    match &operations[0] {
        AbstractOperation::StructuralLeafCopy {
            psi_operation,
            result,
            source,
            path,
        } => {
            assert_eq!(*psi_operation, id::<OperationId>(1));
            // The copied leaf keeps its fresh unrestricted custody row.
            assert_eq!(result.place, id::<PlaceId>(2));
            assert_eq!(result.structural_type, cell());
            assert_eq!(result.multiplicity, StructuralMultiplicity::Unrestricted);
            assert!(result.claims.is_empty());
            // The source stays the borrowed root place itself, not a spelled
            // window: the copy reads through it without vacating anything.
            assert_eq!(*source, id::<PlaceId>(1));
            assert_eq!(path, &[StructuralPathSegment::Field("left".into())]);
        }
        other => panic!("expected StructuralLeafCopy, got {other:?}"),
    }
    assert!(matches!(
        operations[1],
        AbstractOperation::ReturnStructural { .. }
    ));
}

#[test]
fn leaf_copy_builds_and_validates_its_optimization_unit() {
    let module = copy_module(StructuralAccess::SharedBorrow);
    let semantic = encode_module(&module).expect("semantic module encodes");
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .expect("leaf copy projects into optimizer input");
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("leaf copy constructs a verified unit");
    let unit = verified.unit();
    optimization_unit_semantics::validate_psi_optimization_unit(unit)
        .expect("leaf copy unit validates");
    // Provenance keeps the Terminal operation identity through unit
    // construction for the copy.
    let nodes = &unit.functions[0].blocks[0].nodes;
    assert!(
        nodes[0].provenance.iter().any(|row| matches!(
            row,
            optimization_unit::PsiProvenance::Operation(operation)
                if *operation == id::<OperationId>(1)
        )),
        "node lost its Terminal operation provenance"
    );
    // The canonical identity binds the spelled path: renaming the field
    // changes the recomputed unit identity, so the projected leaf cannot
    // silently move.
    let mut changed = unit.clone();
    match &mut changed.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::StructuralLeafCopy { path, .. } => {
            *path = vec![StructuralPathSegment::Field("right".into())];
        }
        _ => unreachable!(),
    }
    assert_ne!(
        optimization_unit::recompute_psi_optimization_unit_identity(&changed),
        unit.identity,
        "path rename must change the canonical unit identity"
    );
}

#[test]
fn a_write_only_borrow_root_never_copies_a_leaf() {
    assert!(matches!(
        encode_module(&copy_module(StructuralAccess::WriteOnlyBorrow)),
        Err(CodecError::InvalidModule(
            ModuleError::StructuralObservationRequiresReadableAccess { .. }
        ))
    ));
}

#[test]
fn a_path_missing_its_leaf_rejects() {
    let mut module = copy_module(StructuralAccess::SharedBorrow);
    let OperationKind::StructuralLeafCopy { path, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *path = vec![StructuralPathSegment::Field("missing".into())];
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            ModuleError::InvalidStructuralLeafCopy { .. }
        ))
    ));
}
