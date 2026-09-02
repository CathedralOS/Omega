use omega_abstract_operations::AbstractOperation;
use omega_psi_to_abstract_operations::{ArtifactLoweringError, lower_artifact_sections};
use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, PsiSemanticId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::ProofBundle;

fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
    Identity::new(raw).expect("test identity is nonzero")
}

fn integer_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32 type"))
}

fn contract(raw: u64) -> MachineContract {
    MachineContract {
        id: id::<ContractId>(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn parameter_place(place: PlaceId) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    }
}

fn structural_scalar_field_module() -> TerminalModule {
    let caller = id::<MachineId>(1);
    let realization = id::<MachineId>(2);
    let owner_type = id::<StructuralTypeId>(1);
    let item_type = id::<StructuralTypeId>(2);
    let caller_self = id::<PlaceId>(1);
    let realization_self = id::<PlaceId>(2);
    let item_field = id::<StructuralFieldId>(1);
    let value_field = id::<StructuralFieldId>(1);
    let integer = integer_type();
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: owner_type,
                identity: "test::Owner".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: item_field,
                        identity: "item".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(item_type),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: item_type,
                identity: "test::Item".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: value_field,
                        identity: "value".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(integer),
                    }],
                },
            },
        ],
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
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: caller,
                attachment: Some(owner_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: caller_self,
                    position: 0,
                    is_self: true,
                    structural_type: owner_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![parameter_place(caller_self)],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id::<BlockId>(1),
                blocks: vec![Block {
                    id: id::<BlockId>(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: id::<OperationId>(1),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: id::<ValueId>(1),
                                scalar_type: integer,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Signed(99),
                            },
                        },
                        Operation {
                            id: id::<OperationId>(2),
                            result: OperationResult::Unit,
                            kind: OperationKind::StructuralScalarFieldStore {
                                destination: caller_self,
                                path: vec![StructuralPathSegment::Field("item".into())],
                                field: value_field,
                                value: id::<ValueId>(1),
                            },
                        },
                        Operation {
                            id: id::<OperationId>(3),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: id::<ValueId>(2),
                                scalar_type: integer,
                            }),
                            kind: OperationKind::CallStructuralScalar {
                                callee: realization,
                                arguments: Vec::new(),
                                structural_arguments: vec![StructuralArgument {
                                    place: caller_self,
                                    path: vec![StructuralPathSegment::Field("item".into())],
                                    access: StructuralAccess::SharedBorrow,
                                }],
                                claim_transfers: Vec::new(),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: id::<EdgeId>(1),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: contract(1),
            },
            TerminalMachine {
                id: realization,
                attachment: Some(item_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: realization_self,
                    position: 0,
                    is_self: true,
                    structural_type: item_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    id: id::<ValueId>(3),
                    scalar_type: integer,
                }),
                structural_places: vec![parameter_place(realization_self)],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id::<BlockId>(2),
                blocks: vec![Block {
                    id: id::<BlockId>(2),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: id::<OperationId>(4),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: id::<ValueId>(4),
                            scalar_type: integer,
                        }),
                        kind: OperationKind::IntegerStructuralField {
                            source: realization_self,
                            field: value_field,
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: id::<EdgeId>(2),
                        value: id::<ValueId>(4),
                    },
                }],
                contract: contract(2),
            },
        ],
    }
}

fn lower(
    module: &TerminalModule,
) -> Result<omega_abstract_operations::AbstractOperationPlan, ArtifactLoweringError> {
    let semantic = encode_module(module).expect("semantic module encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
}

#[test]
fn retains_exact_store_and_integer_field_read_custody() {
    let plan = lower(&structural_scalar_field_module())
        .expect("verified structural scalar operations lower exactly");
    let caller = &plan.functions[0];
    let store_index = caller
        .operations
        .iter()
        .position(|operation| {
            matches!(
                operation,
                AbstractOperation::StructuralScalarFieldStore { .. }
            )
        })
        .expect("caller retains field store");
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        value,
    } = &caller.operations[store_index]
    else {
        unreachable!()
    };
    assert_eq!(*psi_operation, id::<OperationId>(2));
    assert_eq!(destination, &caller.structural_parameters[0]);
    assert_eq!(destination.access, StructuralAccess::MutableBorrow);
    assert_eq!(path, &[StructuralPathSegment::Field("item".into())]);
    assert_eq!(*field, id::<StructuralFieldId>(1));
    assert_eq!(value.value, id::<ValueId>(1));
    assert_eq!(value.scalar_type, integer_type());
    assert!(matches!(
        caller.operations.get(store_index.wrapping_sub(1)),
        Some(AbstractOperation::IntegerConstant {
            result,
            scalar_type,
            value: IntegerValue::Signed(99),
            ..
        }) if *result == value.value && *scalar_type == value.scalar_type
    ));

    let realization = &plan.functions[1];
    let read = realization
        .operations
        .iter()
        .find(|operation| matches!(operation, AbstractOperation::IntegerStructuralField { .. }))
        .expect("realization retains integer field read");
    let AbstractOperation::IntegerStructuralField {
        psi_operation,
        result,
        source,
        field,
    } = read
    else {
        unreachable!()
    };
    assert_eq!(*psi_operation, id::<OperationId>(4));
    assert_eq!(result.value, id::<ValueId>(4));
    assert_eq!(result.scalar_type, integer_type());
    assert_eq!(source, &realization.structural_parameters[0]);
    assert_eq!(source.access, StructuralAccess::SharedBorrow);
    assert_eq!(*field, id::<StructuralFieldId>(1));
}

#[test]
fn rejects_store_path_field_value_and_integer_read_artifact_corruption() {
    let mut path = structural_scalar_field_module();
    let OperationKind::StructuralScalarFieldStore {
        path: store_path, ..
    } = &mut path.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    store_path.clear();
    assert!(encode_module(&path).is_err());

    let mut field = structural_scalar_field_module();
    let OperationKind::StructuralScalarFieldStore {
        field: selected_field,
        ..
    } = &mut field.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *selected_field = id::<StructuralFieldId>(2);
    assert!(encode_module(&field).is_err());

    let mut value = structural_scalar_field_module();
    let OperationKind::StructuralScalarFieldStore {
        value: stored_value,
        ..
    } = &mut value.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *stored_value = id::<ValueId>(99);
    assert!(encode_module(&value).is_err());

    let mut read = structural_scalar_field_module();
    read.machines[1].blocks[0].operations[0]
        .result
        .scalar_mut()
        .expect("integer read result")
        .scalar_type = ScalarType::Boolean;
    assert!(encode_module(&read).is_err());
}
