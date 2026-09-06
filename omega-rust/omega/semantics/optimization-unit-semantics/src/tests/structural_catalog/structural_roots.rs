//! Function structural-root uniqueness and operation replay tests.

use super::super::*;

#[test]
fn logical_structural_roots_are_unique_beyond_place_identity() {
    let mut duplicate = structural_result_call_unit();
    let first_call = duplicate.functions[0].blocks[0].nodes[0].clone();
    let (psi_operation, result_type) = match &first_call.operation {
        O::CallStructural {
            psi_operation,
            result,
            ..
        } => (*psi_operation, result.structural_type),
        _ => panic!("fixture begins with one structural call"),
    };
    let duplicate_place = id(4_712, PlaceId::new);
    let mut duplicate_call = first_call;
    let O::CallStructural {
        result: duplicate_result,
        ..
    } = &mut duplicate_call.operation
    else {
        unreachable!()
    };
    duplicate_result.place = duplicate_place;
    duplicate.functions[0].blocks[0]
        .nodes
        .insert(1, duplicate_call);
    duplicate.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: duplicate_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: psi_operation,
                structural_type: result_type,
            },
        });
    refresh_function_derivatives(&mut duplicate, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate),
        Err(
            OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                machine: _,
                kind: StructuralPlaceKind::OperationResult { .. },
            }
        )
    ));
}

#[test]
fn boolean_structural_field_replays_terminal_root_and_cleanup_contract() {
    let baseline = boolean_structural_field_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("exact affine readable Boolean observation validates");
    let invalid = |mut candidate: PsiOptimizationUnit| {
        refresh_identity(&mut candidate);
        assert!(matches!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidBooleanStructuralField { .. })
        ));
    };

    let mut non_entry = baseline.clone();
    non_entry.entry = id(4_799, MachineId::new);
    invalid(non_entry);

    let mut unrestricted = baseline.clone();
    unrestricted.functions[0].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Unrestricted;
    invalid(unrestricted);

    let mut write_only = baseline.clone();
    write_only.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::WriteOnlyBorrow;
    invalid(write_only);

    let mut qualified = baseline.clone();
    let domain = id(1, StructuralDomainId::new);
    qualified.structural_domains =
        vec![structural_domain(1, 1, qualified.structural_types[0].id)].into();
    qualified.functions[0].structural_parameters[0]
        .qualifications
        .push(domain);
    invalid(qualified);

    let mut claimed = baseline.clone();
    let claim = id(1, ClaimId::new);
    let source = claimed.functions[0].structural_parameters[0].place;
    claimed.functions[0]
        .entry_claim_declarations
        .push(terminal_psi::EntryClaim {
            claim,
            input: source,
            path: Vec::new(),
        });
    claimed.functions[0].entry_claims.insert(claim);
    invalid(claimed);

    let mut content_claimed = baseline.clone();
    install_content_owner(&mut content_claimed);
    content_claimed.functions[0]
        .content_entry_claims
        .push(content_entry_claim(claim, source));
    invalid(content_claimed);

    let mut no_boolean_parameter = baseline.clone();
    no_boolean_parameter.functions[0].parameters.clear();
    invalid(no_boolean_parameter);

    let mut missing_cleanup = baseline.clone();
    let O::Return {
        cleanup_actions, ..
    } = &mut missing_cleanup.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("fixture ends in a scalar return")
    };
    cleanup_actions.clear();
    refresh_node_derivatives(&mut missing_cleanup, 0, 0, 1);
    invalid(missing_cleanup);

    let mut wrong_field = baseline.clone();
    let O::BooleanStructuralField { field, .. } =
        &mut wrong_field.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with its observation")
    };
    *field = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
    invalid(wrong_field);

    let mut non_boolean_field = baseline.clone();
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut non_boolean_field.structural_types[0].shape
    else {
        unreachable!()
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
    ));
    invalid(non_boolean_field);

    let mut differing_observation = baseline;
    let mut second = differing_observation.functions[0].blocks[0].nodes[0].clone();
    let second_field = id(4_713, semantic_vocabulary::StructuralFieldId::new);
    let O::BooleanStructuralField {
        psi_operation,
        result,
        field,
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = id(4_714, OperationId::new);
    *result = id(4_715, ValueId::new);
    *field = second_field;
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut differing_observation.structural_types[0].shape
    else {
        unreachable!()
    };
    fields.push(terminal_psi::StructuralFieldDeclaration {
        id: second_field,
        identity: "validation::other-ready".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    differing_observation.functions[0].blocks[0]
        .nodes
        .insert(1, second);
    refresh_function_derivatives(&mut differing_observation, 0);
    invalid(differing_observation);
}

fn assert_invalid_direct_realization_observation(mut candidate: PsiOptimizationUnit) {
    refresh_identity(&mut candidate);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidBooleanStructuralField { .. })
    ));
}

#[test]
fn unrestricted_shared_boolean_structural_field_direct_realization_validates() {
    validate_psi_optimization_unit(&direct_realization_boolean_structural_field_unit())
        .expect("an unqualified unrestricted shared direct realization validates");
}

#[test]
fn unrestricted_shared_integer_structural_field_direct_realization_validates() {
    validate_psi_optimization_unit(&direct_realization_integer_structural_field_unit())
        .expect("an unqualified unrestricted shared integer field read validates");
}

#[test]
fn direct_integer_structural_field_rejects_access_type_and_field_corruption() {
    let mut access = direct_realization_integer_structural_field_unit();
    access.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::MutableBorrow;
    refresh_identity(&mut access);
    assert!(matches!(
        validate_psi_optimization_unit(&access),
        Err(OptimizationUnitValidationError::InvalidIntegerStructuralField { .. })
    ));

    let mut result_type = direct_realization_integer_structural_field_unit();
    let O::IntegerStructuralField { result, .. } =
        &mut result_type.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Boolean;
    refresh_node_derivatives(&mut result_type, 0, 0, 0);
    refresh_identity(&mut result_type);
    assert!(validate_psi_optimization_unit(&result_type).is_err());

    let mut field = direct_realization_integer_structural_field_unit();
    let O::IntegerStructuralField {
        field: selected_field,
        ..
    } = &mut field.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *selected_field = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut field, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&field),
        Err(OptimizationUnitValidationError::InvalidIntegerStructuralField { .. })
    ));
}

#[test]
fn projected_structural_scalar_field_store_validates_and_rejects_corruption() {
    validate_psi_optimization_unit(&structural_scalar_field_store_unit())
        .expect("an exact mutable projected scalar store validates");

    let mut affine = structural_scalar_field_store_unit();
    affine.functions[0].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Affine;
    let O::StructuralScalarFieldStore { destination, .. } =
        &mut affine.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    destination.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut affine, 0, 0, 1);
    refresh_identity(&mut affine);
    validate_psi_optimization_unit(&affine)
        .expect("an exact affine mutable loan retains the same store authority");

    let mut access = structural_scalar_field_store_unit();
    access.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::SharedBorrow;
    let O::StructuralScalarFieldStore { destination, .. } =
        &mut access.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    destination.access = terminal_psi::StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut access, 0, 0, 1);
    refresh_identity(&mut access);
    assert!(matches!(
        validate_psi_optimization_unit(&access),
        Err(OptimizationUnitValidationError::InvalidStructuralScalarFieldStore { .. })
    ));

    let mut path = structural_scalar_field_store_unit();
    let O::StructuralScalarFieldStore { path: selected, .. } =
        &mut path.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    selected.clear();
    refresh_node_derivatives(&mut path, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&path),
        Err(OptimizationUnitValidationError::InvalidStructuralScalarFieldStore { .. })
    ));

    let mut field = structural_scalar_field_store_unit();
    let O::StructuralScalarFieldStore {
        field: selected, ..
    } = &mut field.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    *selected = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut field, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&field),
        Err(OptimizationUnitValidationError::InvalidStructuralScalarFieldStore { .. })
    ));
}

#[test]
fn direct_realization_boolean_structural_field_rejects_access_corruption() {
    let mut readable_but_exclusive = direct_realization_boolean_structural_field_unit();
    readable_but_exclusive.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::MutableBorrow;
    assert_invalid_direct_realization_observation(readable_but_exclusive);

    let mut write_only = direct_realization_boolean_structural_field_unit();
    write_only.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::WriteOnlyBorrow;
    assert_invalid_direct_realization_observation(write_only);
}

#[test]
fn direct_realization_boolean_structural_field_rejects_multiplicity_corruption() {
    let mut affine = direct_realization_boolean_structural_field_unit();
    affine.functions[0].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Affine;
    assert_invalid_direct_realization_observation(affine);
}

#[test]
fn direct_realization_boolean_structural_field_rejects_type_corruption() {
    let mut non_boolean = direct_realization_boolean_structural_field_unit();
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut non_boolean.structural_types[0].shape
    else {
        panic!("direct realization carrier is a record")
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
    ));
    assert_invalid_direct_realization_observation(non_boolean);
}

#[test]
fn direct_realization_boolean_structural_field_rejects_field_corruption() {
    let mut wrong_field = direct_realization_boolean_structural_field_unit();
    let O::BooleanStructuralField { field, .. } =
        &mut wrong_field.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("direct realization begins with a Boolean field observation")
    };
    *field = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
    assert_invalid_direct_realization_observation(wrong_field);
}

#[test]
fn structural_returns_reject_non_source_roots_and_signature_drift() {
    let mut result_root = structural_result_call_unit();
    let result_place = result_root.functions[1]
        .result
        .structural()
        .expect("structural result")
        .place;
    let return_node = result_root.functions[1].blocks[0].nodes.len() - 1;
    let O::ReturnStructural { source, .. } =
        &mut result_root.functions[1].blocks[0].nodes[return_node].operation
    else {
        panic!("fixture returns structurally")
    };
    *source = result_place;
    refresh_node_derivatives(&mut result_root, 1, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&result_root),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));

    let mut literal_root = structural_result_call_unit();
    let literal_type = terminal_psi::StructuralTypeDeclaration {
        id: id(4_716, StructuralTypeId::new),
        identity: "validation::return-source-literal".into(),
        shape: terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView,
        ),
    };
    let literal = terminal_psi::StructuralPlaceDeclaration {
        id: id(4_717, PlaceId::new),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: literal_type.id,
        },
    };
    literal_root.structural_types.push(literal_type.clone());
    literal_root.functions[1].structural_places.push(literal);
    let establishment_node = literal_root.functions[1].blocks[0].nodes[0].clone();
    literal_root.functions[1].blocks[0]
        .nodes
        .insert(0, establishment_node);
    literal_root.functions[1].blocks[0].nodes[0].operation = O::EstablishByteSequenceLiteral {
        psi_operation: id(4_718, OperationId::new),
        place: literal,
        structural_type: literal_type,
        bytes: b"return-source".to_vec(),
    };
    let O::ReturnStructural { source, .. } =
        &mut literal_root.functions[1].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    *source = literal.id;
    refresh_function_derivatives(&mut literal_root, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&literal_root),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));

    let mut wrong_signature =
        operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
    let O::CallStructural { result, .. } =
        &mut wrong_signature.functions[0].blocks[3].nodes[0].operation
    else {
        panic!("non-topological fixture stores its call in the entry block")
    };
    result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut wrong_signature, 0, 3, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_signature),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));
}
