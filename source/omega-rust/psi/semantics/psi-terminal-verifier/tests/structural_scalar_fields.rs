use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, PsiSemanticId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use psi_terminal::{
    BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{ModuleError, validate_module};

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

fn structural_place(place: PlaceId) -> StructuralPlaceDeclaration {
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
                        id: id::<StructuralFieldId>(1),
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
                        id: id::<StructuralFieldId>(1),
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
                structural_places: vec![structural_place(caller_self)],
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
                                field: id::<StructuralFieldId>(1),
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
                                arguments: vec![id::<ValueId>(1)],
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
                parameters: vec![ValueDeclaration {
                    id: id::<ValueId>(5),
                    scalar_type: integer,
                }],
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
                structural_places: vec![structural_place(realization_self)],
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
                            field: id::<StructuralFieldId>(1),
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

#[test]
fn admits_scalar_store_observed_through_projected_structural_call() {
    validate_module(&structural_scalar_field_module())
        .expect("mixed scalar/structural call module verifies");
}

#[test]
fn rejects_mixed_structural_call_scalar_argument_corruption() {
    let mut missing = structural_scalar_field_module();
    let OperationKind::CallStructuralScalar { arguments, .. } =
        &mut missing.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    arguments.clear();
    assert_eq!(
        validate_module(&missing).unwrap_err(),
        ModuleError::CallArgumentArityMismatch {
            operation: id::<OperationId>(3),
            expected: 1,
            actual: 0,
        }
    );

    let mut wrong_type = structural_scalar_field_module();
    wrong_type.machines[0].blocks[0].operations.insert(
        2,
        Operation {
            id: id::<OperationId>(5),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id::<ValueId>(6),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        },
    );
    let OperationKind::CallStructuralScalar { arguments, .. } =
        &mut wrong_type.machines[0].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    arguments[0] = id::<ValueId>(6);
    assert_eq!(
        validate_module(&wrong_type).unwrap_err(),
        ModuleError::CallArgumentTypeMismatch {
            operation: id::<OperationId>(3),
            argument: id::<ValueId>(6),
            expected: integer_type(),
            actual: ScalarType::Boolean,
        }
    );

    let mut use_before_definition = structural_scalar_field_module();
    let OperationKind::CallStructuralScalar { arguments, .. } =
        &mut use_before_definition.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    arguments[0] = id::<ValueId>(2);
    assert_eq!(
        validate_module(&use_before_definition).unwrap_err(),
        ModuleError::ValueUsedBeforeDefinition(id::<ValueId>(2))
    );
}

#[test]
fn rejects_store_path_field_type_and_authority_corruption() {
    let mut path = structural_scalar_field_module();
    let OperationKind::StructuralScalarFieldStore {
        path: store_path, ..
    } = &mut path.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    store_path.clear();
    assert!(matches!(
        validate_module(&path),
        Err(ModuleError::InvalidStructuralScalarFieldStore { operation, .. })
            if operation == id::<OperationId>(2)
    ));

    let mut field = structural_scalar_field_module();
    let OperationKind::StructuralScalarFieldStore {
        field: selected, ..
    } = &mut field.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *selected = id::<StructuralFieldId>(2);
    assert!(matches!(
        validate_module(&field),
        Err(ModuleError::InvalidStructuralScalarFieldStore { operation, .. })
            if operation == id::<OperationId>(2)
    ));

    let mut value_type = structural_scalar_field_module();
    value_type.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .expect("constant result")
        .scalar_type = ScalarType::Boolean;
    value_type.machines[0].blocks[0].operations[0].kind =
        OperationKind::BooleanConstant { value: true };
    assert!(matches!(
        validate_module(&value_type),
        Err(ModuleError::StructuralScalarFieldStoreValueTypeMismatch {
            operation,
            expected,
            actual: ScalarType::Boolean,
        }) if operation == id::<OperationId>(2) && expected == integer_type()
    ));

    let mut authority = structural_scalar_field_module();
    authority.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        validate_module(&authority),
        Err(ModuleError::InvalidStructuralScalarFieldStore { operation, .. })
            if operation == id::<OperationId>(2)
    ));
}

#[test]
fn rejects_integer_field_result_and_read_authority_corruption() {
    let mut result = structural_scalar_field_module();
    result.machines[1].blocks[0].operations[0]
        .result
        .scalar_mut()
        .expect("integer field result")
        .scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&result),
        Err(ModuleError::IntegerStructuralFieldRequiresIntegerResult(operation))
            if operation == id::<OperationId>(4)
    ));

    let mut authority = structural_scalar_field_module();
    authority.entry = id::<MachineId>(2);
    authority.machines.remove(0);
    authority.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let error = validate_module(&authority).expect_err("mutable field read rejects");
    assert!(
        matches!(
            error,
            ModuleError::InvalidIntegerStructuralField { operation, .. }
                if operation == id::<OperationId>(4)
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn admits_affine_shared_integer_field_observation_but_rejects_linear_custody() {
    let mut affine = structural_scalar_field_module();
    affine.entry = id::<MachineId>(2);
    affine.machines.remove(0);
    affine.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    validate_module(&affine).expect("an affine shared parameter licenses scalar observation");

    let mut linear = affine;
    linear.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
    let error = validate_module(&linear).expect_err("linear field observation rejects");
    assert!(
        matches!(
            error,
            ModuleError::LinearParameterHasNoEntryClaim { machine, place }
                if machine == id::<MachineId>(2) && place == id::<PlaceId>(2)
        ),
        "unexpected error: {error:?}"
    );
}
