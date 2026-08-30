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
        .push(psi_terminal::StructuralPlaceDeclaration {
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
        psi_terminal::StructuralMultiplicity::Unrestricted;
    invalid(unrestricted);

    let mut write_only = baseline.clone();
    write_only.functions[0].structural_parameters[0].access =
        psi_terminal::StructuralAccess::WriteOnlyBorrow;
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
        .push(psi_terminal::EntryClaim {
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
    *field = id(4_799, psi_core::StructuralFieldId::new);
    refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
    invalid(wrong_field);

    let mut non_boolean_field = baseline.clone();
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &mut non_boolean_field.structural_types[0].shape
    else {
        unreachable!()
    };
    fields[0].field_type = psi_terminal::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
    ));
    invalid(non_boolean_field);

    let mut differing_observation = baseline;
    let mut second = differing_observation.functions[0].blocks[0].nodes[0].clone();
    let second_field = id(4_713, psi_core::StructuralFieldId::new);
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
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &mut differing_observation.structural_types[0].shape
    else {
        unreachable!()
    };
    fields.push(psi_terminal::StructuralFieldDeclaration {
        id: second_field,
        identity: "validation::other-ready".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    differing_observation.functions[0].blocks[0]
        .nodes
        .insert(1, second);
    refresh_function_derivatives(&mut differing_observation, 0);
    invalid(differing_observation);
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
    let literal_type = psi_terminal::StructuralTypeDeclaration {
        id: id(4_716, StructuralTypeId::new),
        identity: "validation::return-source-literal".into(),
        shape: psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    };
    let literal = psi_terminal::StructuralPlaceDeclaration {
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
    result.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut wrong_signature, 0, 3, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_signature),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));
}
