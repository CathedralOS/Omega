use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, PsiSemanticId, ScalarTerm, ScalarType, StructuralFieldId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{ModuleError, reconstruct_operation_obligations, validate_module};

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
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
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

fn indexed_scalar_field_store_module(index: u64) -> TerminalModule {
    let mut module = structural_scalar_field_module();
    module.machines.truncate(1);
    module.machines[0].blocks[0].operations.truncate(2);
    module.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::Structural(id::<StructuralTypeId>(3));
    module.structural_types.push(StructuralTypeDeclaration {
        id: id::<StructuralTypeId>(3),
        identity: "test::Items".into(),
        shape: StructuralTypeShape::FixedArray {
            element: id::<StructuralTypeId>(2),
            length: 3,
        },
    });
    scalar_store_path(&mut module).push(StructuralPathSegment::FixedIndex(index));
    module
}

fn scalar_store_path(module: &mut TerminalModule) -> &mut Vec<StructuralPathSegment> {
    let OperationKind::StructuralScalarFieldStore { path, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    path
}

fn assert_invalid_scalar_store(module: &TerminalModule) {
    let error = validate_module(module).expect_err("invalid scalar store rejects");
    assert!(
        matches!(
            error,
            ModuleError::InvalidStructuralScalarFieldStore { operation, .. }
                if operation == id::<OperationId>(2)
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn admits_indexed_scalar_stores_at_both_array_boundaries() {
    for index in [0, 2] {
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut module = indexed_scalar_field_store_module(index);
            module.machines[0].structural_parameters[0].access = access;
            validate_module(&module).expect("in-bounds indexed scalar store verifies");
        }
    }
}

#[test]
fn rejects_indexed_scalar_stores_outside_array_bounds() {
    for index in [3, u64::MAX] {
        assert_invalid_scalar_store(&indexed_scalar_field_store_module(index));
    }
}

#[test]
fn rejects_indexed_scalar_stores_with_malformed_carrier_paths() {
    let mut empty_field = indexed_scalar_field_store_module(0);
    scalar_store_path(&mut empty_field)[0] = StructuralPathSegment::Field(String::new());
    assert_invalid_scalar_store(&empty_field);

    let mut root_index = indexed_scalar_field_store_module(0);
    root_index.machines[0].attachment = Some(id::<StructuralTypeId>(3));
    root_index.machines[0].structural_parameters[0].structural_type = id::<StructuralTypeId>(3);
    scalar_store_path(&mut root_index).remove(0);
    assert_invalid_scalar_store(&root_index);

    // These paths are well typed and in bounds; only the bounded store grammar
    // excludes a second array index or a field after the first index.
    let mut repeated_index = indexed_scalar_field_store_module(0);
    repeated_index.structural_types[2].shape = StructuralTypeShape::FixedArray {
        element: id::<StructuralTypeId>(4),
        length: 3,
    };
    repeated_index
        .structural_types
        .push(StructuralTypeDeclaration {
            id: id::<StructuralTypeId>(4),
            identity: "test::InnerItems".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id::<StructuralTypeId>(2),
                length: 2,
            },
        });
    scalar_store_path(&mut repeated_index).push(StructuralPathSegment::FixedIndex(1));
    assert_invalid_scalar_store(&repeated_index);

    let mut field_after_index = indexed_scalar_field_store_module(0);
    field_after_index.structural_types[2].shape = StructuralTypeShape::FixedArray {
        element: id::<StructuralTypeId>(4),
        length: 3,
    };
    field_after_index
        .structural_types
        .push(StructuralTypeDeclaration {
            id: id::<StructuralTypeId>(4),
            identity: "test::WrappedItem".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id::<StructuralFieldId>(1),
                    identity: "nested".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(id::<StructuralTypeId>(2)),
                }],
            },
        });
    scalar_store_path(&mut field_after_index).push(StructuralPathSegment::Field("nested".into()));
    assert_invalid_scalar_store(&field_after_index);
}

#[test]
fn rejects_indexed_scalar_stores_through_erased_fields() {
    for structural_type_index in [0, 1] {
        let mut module = indexed_scalar_field_store_module(0);
        let StructuralTypeShape::Record { fields } =
            &mut module.structural_types[structural_type_index].shape
        else {
            unreachable!()
        };
        fields[0].relevance = BindingRelevance::Erased;
        fields[0].field_type = StructuralFieldType::Erased {
            type_identity: "test::ErasedContent".into(),
        };
        assert_invalid_scalar_store(&module);
    }
}

#[test]
fn rejects_indexed_scalar_store_value_type_mismatch() {
    let mut module = indexed_scalar_field_store_module(2);
    module.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .expect("constant result")
        .scalar_type = ScalarType::Boolean;
    module.machines[0].blocks[0].operations[0].kind =
        OperationKind::BooleanConstant { value: true };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::StructuralScalarFieldStoreValueTypeMismatch {
            operation: id::<OperationId>(2),
            expected: integer_type(),
            actual: ScalarType::Boolean,
        })
    );
}

#[test]
fn indexed_shared_subloans_preserve_source_access_and_multiplicity() {
    let mut module = indexed_scalar_field_store_module(2);
    let existing = structural_scalar_field_module();
    let mut call = existing.machines[0].blocks[0].operations[2].clone();
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = scalar_store_path(&mut module).clone();
    module.machines[0].blocks[0].operations[1] = call;
    module.machines.push(existing.machines[1].clone());
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    validate_module(&module).expect("unrestricted indexed shared subloan verifies");

    let mut write_only = module.clone();
    write_only.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    assert_eq!(
        validate_module(&write_only).map(|_| ()),
        Err(ModuleError::StructuralArgumentAccessExceedsSource {
            operation: id::<OperationId>(3),
            argument_index: 0,
            source: StructuralAccess::WriteOnlyBorrow,
            presented: StructuralAccess::SharedBorrow,
        })
    );

    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
    module.machines[0]
        .entry_claims
        .push(psi_terminal::EntryClaim {
            claim: id::<psi_core::ClaimId>(1),
            input: id::<PlaceId>(1),
            path: Vec::new(),
        });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::StructuralArgumentMultiplicityMismatch {
            operation: id::<OperationId>(3),
            argument_index: 0,
            expected: StructuralMultiplicity::Unrestricted,
            actual: StructuralMultiplicity::Linear,
        })
    );
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
fn mixed_structural_call_contracts_substitute_scalar_parameters() {
    let mut module = structural_scalar_field_module();
    let integer = integer_type();
    let OperationKind::CallStructuralScalar {
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    requirement_obligations.push(id::<ObligationId>(1));
    module.machines[1]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::value(id::<ValueId>(5), integer),
            ScalarTerm::integer(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                IntegerValue::Signed(99),
            )
            .unwrap(),
        ));
    let obligations =
        reconstruct_operation_obligations(&module).expect("mixed call obligation reconstructs");
    let [obligation] = obligations.as_slice() else {
        panic!("one mixed call requirement")
    };
    assert_eq!(
        obligation.obligation.proposition,
        Proposition::Equal(
            ScalarTerm::value(id::<ValueId>(1), integer),
            ScalarTerm::integer(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                IntegerValue::Signed(99),
            )
            .unwrap(),
        )
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
fn rejects_integer_field_result_and_write_only_authority_corruption() {
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
    validate_module(&authority).expect("mutable authority remains readable");
    authority.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    let error = validate_module(&authority).expect_err("write-only field read rejects");
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
