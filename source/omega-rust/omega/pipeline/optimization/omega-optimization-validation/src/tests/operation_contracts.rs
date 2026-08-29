//! Scalar, structural-call, claim, and ownership contract tests.

use super::*;

#[test]
fn rejects_self_consistent_scalar_operation_contract_corruption() {
    let mut arithmetic = exact_add_unit();
    let (psi_operation, result) = match &arithmetic.functions[0].blocks[0].nodes[1].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("fixture right operand is an integer constant"),
    };
    arithmetic.functions[0].blocks[0].nodes[1].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut arithmetic, 0, 0, 1);
    assert_eq!(
        validate_psi_optimization_unit(&arithmetic),
        Err(
            OptimizationUnitValidationError::ScalarOperationContractMismatch {
                machine: id(201, MachineId::new),
                block: id(202, BlockId::new),
                node: 2,
            }
        )
    );

    let mut out_of_range = unit();
    let AbstractOperation::IntegerConstant { value, .. } =
        &mut out_of_range.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with an integer constant")
    };
    *value = IntegerValue::Unsigned(256);
    refresh_node_derivatives(&mut out_of_range, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&out_of_range),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
    ));
}

#[test]
fn rejects_self_consistent_control_and_return_type_corruption() {
    let mut conditional = redundant_parameter_region_fixture().0;
    conditional.functions[0].parameters[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("valid integer"));
    refresh_identity(&mut conditional);
    assert!(matches!(
        validate_psi_optimization_unit(&conditional),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
    ));

    let mut scalar_return = unit();
    let (psi_operation, result) = match &scalar_return.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("fixture begins with an integer constant"),
    };
    scalar_return.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut scalar_return, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&scalar_return),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
    ));
}

#[test]
fn rejects_self_consistent_call_signature_corruption() {
    let mut call = scalar_call_unit();
    let (psi_operation, result) = match &call.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("caller begins with an integer constant"),
    };
    call.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut call, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&call),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
    ));

    let mut boundary = scalar_boundary_call_unit();
    let (psi_operation, result) = match &boundary.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("boundary caller begins with an integer constant"),
    };
    boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
    ));

    let mut duplicate_boundary = scalar_boundary_call_unit();
    duplicate_boundary
        .boundary_machines
        .push(duplicate_boundary.boundary_machines[0].clone());
    refresh_identity(&mut duplicate_boundary);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate_boundary),
        Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(_))
    ));
}

#[test]
fn rejects_structural_call_argument_arity_and_access_corruption() {
    let baseline = structural_call_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("matching structural argument access should validate");

    let mut access = baseline.clone();
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut access.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments[0].access = psi_terminal::StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut access, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&access),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut arity = baseline;
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut arity.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments.clear();
    refresh_node_derivatives(&mut arity, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&arity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut boundary = structural_call_unit();
    let boundary_id = id(341, BoundaryMachineId::new);
    boundary
        .boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary_id,
            identity: "validation::structural-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![boundary.functions[1].structural_parameters[0].clone()],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    let (psi_operation, structural_arguments) =
        match &boundary.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::CallUnit {
                psi_operation,
                structural_arguments,
                ..
            } => (*psi_operation, structural_arguments.clone()),
            _ => panic!("fixture begins with a structural Unit call"),
        };
    boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary: boundary_id,
        arguments: Vec::new(),
        structural_arguments,
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    validate_psi_optimization_unit(&boundary)
        .expect("matching boundary structural access should validate");

    boundary.boundary_machines[0].structural_parameters[0].access =
        psi_terminal::StructuralAccess::SharedBorrow;
    refresh_identity(&mut boundary);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

#[test]
fn rejects_structural_call_path_type_multiplicity_and_qualification_corruption() {
    let baseline = structural_call_unit();

    let mut path = baseline.clone();
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut path.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments[0].path = vec![psi_terminal::StructuralPathSegment::FixedIndex(0)];
    refresh_node_derivatives(&mut path, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&path),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_type = baseline.clone();
    let alternate = id(342, psi_core::StructuralTypeId::new);
    wrong_type
        .structural_types
        .push(psi_terminal::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-structural-call-argument".into(),
            shape: psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        });
    wrong_type.functions[1].structural_parameters[0].structural_type = alternate;
    refresh_identity(&mut wrong_type);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_type),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut multiplicity = baseline.clone();
    multiplicity.functions[1].structural_parameters[0].multiplicity =
        psi_terminal::StructuralMultiplicity::Affine;
    refresh_identity(&mut multiplicity);
    assert!(matches!(
        validate_psi_optimization_unit(&multiplicity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut source_access = baseline.clone();
    source_access.functions[0].structural_parameters[0].access =
        psi_terminal::StructuralAccess::SharedBorrow;
    refresh_identity(&mut source_access);
    assert!(matches!(
        validate_psi_optimization_unit(&source_access),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut qualified = baseline;
    let domain = id(343, psi_core::StructuralDomainId::new);
    qualified.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: id(344, psi_core::DomainSemanticId::new),
        identity: "validation::structural-call-domain".into(),
        carrier: qualified.structural_types[0].id,
        content_projection: None,
    }]
    .into();
    qualified.functions[0].structural_parameters[0].qualifications = vec![domain];
    qualified.functions[1].structural_parameters[0].qualifications = vec![domain];
    refresh_identity(&mut qualified);
    validate_psi_optimization_unit(&qualified)
        .expect("an exact retained argument qualification should validate");

    qualified.functions[0].structural_parameters[0]
        .qualifications
        .clear();
    refresh_identity(&mut qualified);
    assert!(matches!(
        validate_psi_optimization_unit(&qualified),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

#[test]
fn rejects_self_consistent_internal_claim_transfer_and_boundary_completion_corruption() {
    let internal = affine_claim_transfer_unit();
    let claim = id(1, ClaimId::new);
    validate_psi_optimization_unit(&internal)
        .expect("exact ordinary claim correspondence should validate");

    let mut missing_transfer = internal.clone();
    let AbstractOperation::CallUnit {
        claim_transfers, ..
    } = &mut missing_transfer.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    claim_transfers.clear();
    refresh_node_derivatives(&mut missing_transfer, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_transfer),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut boundary = structural_call_unit();
    boundary.functions[0].structural_parameters[0].multiplicity =
        psi_terminal::StructuralMultiplicity::Affine;
    let entry = psi_terminal::EntryClaim {
        claim,
        input: boundary.functions[0].structural_parameters[0].place,
        path: Vec::new(),
    };
    boundary.functions[0]
        .entry_claim_declarations
        .push(entry.clone());
    boundary.functions[0].entry_claims.insert(claim);
    let boundary_id = id(345, BoundaryMachineId::new);
    let mut parameter = boundary.functions[1].structural_parameters[0].clone();
    parameter.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
    boundary
        .boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary_id,
            identity: "validation::claim-completing-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![parameter],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    let (psi_operation, structural_arguments) =
        match &boundary.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::CallUnit {
                psi_operation,
                structural_arguments,
                ..
            } => (*psi_operation, structural_arguments.clone()),
            _ => panic!("fixture begins with a structural Unit call"),
        };
    boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary: boundary_id,
        arguments: Vec::new(),
        structural_arguments,
        completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
            claim,
            entry: Some(entry),
            content: None,
        }],
        completion_receipts: vec![psi_terminal::CompletionReceipt {
            claim,
            argument_index: 0,
        }],
    };
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    validate_psi_optimization_unit(&boundary)
        .expect("exact boundary completion evidence should validate");

    let AbstractOperation::BoundaryCall {
        completion_claim_sources,
        completion_receipts,
        ..
    } = &mut boundary.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture now contains a boundary call")
    };
    completion_claim_sources.clear();
    completion_receipts.clear();
    refresh_node_derivatives(&mut boundary, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&boundary),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

#[test]
fn current_claim_replay_rejects_double_transfer_stale_crash_and_invalid_returns() {
    let claim = id(1, ClaimId::new);
    validate_psi_optimization_unit(&affine_claim_join_unit(true))
        .expect("equal current claim settlement on both arms joins exactly");
    assert!(matches!(
        validate_psi_optimization_unit(&affine_claim_join_unit(false)),
        Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch { .. })
    ));

    let baseline = affine_claim_transfer_unit();
    validate_psi_optimization_unit(&baseline).expect("one affine claim transfer is live");

    let mut double_transfer = baseline.clone();
    let mut repeated = double_transfer.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
        unreachable!("fixture starts with a Unit call")
    };
    *psi_operation = id(341, OperationId::new);
    double_transfer.functions[0].blocks[0]
        .nodes
        .insert(1, repeated);
    refresh_function_derivatives(&mut double_transfer, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&double_transfer),
        Err(OptimizationUnitValidationError::CurrentClaimNotLive {
            node: 1,
            claim: actual,
            ..
        }) if actual == claim
    ));

    let mut stale_crash = baseline;
    let return_node = stale_crash.functions[0].blocks[0].nodes.len() - 1;
    let psi_edge = match stale_crash.functions[0].blocks[0].nodes[return_node].operation {
        AbstractOperation::ReturnUnit { psi_edge, .. } => psi_edge,
        _ => unreachable!("fixture returns Unit"),
    };
    stale_crash.functions[0].blocks[0].nodes[return_node].operation = AbstractOperation::Crash {
        psi_edge,
        cause: psi_terminal::CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: vec![claim],
    };
    refresh_node_derivatives(&mut stale_crash, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&stale_crash),
        Err(OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch { .. })
    ));

    let baseline = structural_result_call_unit();
    let mut missing_return = baseline.clone();
    let return_node = missing_return.functions[0].blocks[0].nodes.len() - 1;
    let AbstractOperation::ReturnStructural {
        returned_claims, ..
    } = &mut missing_return.functions[0].blocks[0].nodes[return_node].operation
    else {
        unreachable!("fixture returns the structural call result")
    };
    returned_claims.clear();
    refresh_node_derivatives(&mut missing_return, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_return),
        Err(OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch { .. })
    ));

    let mut linear_unit_return = baseline;
    let result_place = linear_unit_return.functions[0]
        .result
        .structural()
        .expect("fixture has a structural result")
        .place;
    linear_unit_return.functions[0].result = AbstractFunctionResult::Unit;
    linear_unit_return.functions[0]
        .structural_places
        .retain(|place| place.id != result_place);
    linear_unit_return.functions[0]
        .declared_places
        .remove(&result_place);
    let return_node = linear_unit_return.functions[0].blocks[0].nodes.len() - 1;
    let psi_edge = match linear_unit_return.functions[0].blocks[0].nodes[return_node].operation {
        AbstractOperation::ReturnStructural { psi_edge, .. } => psi_edge,
        _ => unreachable!("fixture returns structurally"),
    };
    linear_unit_return.functions[0].blocks[0].nodes[return_node].operation =
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions: Vec::new(),
        };
    refresh_node_derivatives(&mut linear_unit_return, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&linear_unit_return),
        Err(OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
            claim: actual,
            ..
        }) if actual == claim
    ));
}

#[test]
fn current_owned_place_replay_rejects_double_moves_unequal_joins_and_bad_residuals() {
    let baseline = affine_place_transfer_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("one claim-free affine whole-root transfer is exact");

    let mut double_move = baseline;
    let mut repeated = double_move.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
        unreachable!("fixture begins with a Unit call")
    };
    *psi_operation = id(4_862, OperationId::new);
    double_move.functions[0].blocks[0].nodes.insert(1, repeated);
    refresh_function_derivatives(&mut double_move, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&double_move),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { node: 1, .. })
    ));

    validate_psi_optimization_unit(&affine_place_join_unit(true))
        .expect("equal whole-root settlement on both arms joins exactly");
    assert!(matches!(
        validate_psi_optimization_unit(&affine_place_join_unit(false)),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceJoinMismatch { .. })
    ));

    let baseline = partial_affine_place_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("one projected move and its exact residual cleanup validate");

    let mut overlap = baseline.clone();
    let mut repeated = overlap.functions[0].blocks[0].nodes[0].clone();
    let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
        unreachable!("fixture begins with a projected Unit call")
    };
    *psi_operation = id(4_863, OperationId::new);
    overlap.functions[0].blocks[0].nodes.insert(1, repeated);
    refresh_function_derivatives(&mut overlap, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&overlap),
        Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { node: 1, .. })
    ));

    let mutate_residual =
        |unit: &mut PsiOptimizationUnit,
         mutate: &dyn Fn(&mut psi_terminal::StructuralAffineDiscard)| {
            let return_node = unit.functions[0].blocks[0].nodes.len() - 1;
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = &mut unit.functions[0].blocks[0].nodes[return_node].operation
            else {
                unreachable!("fixture returns Unit")
            };
            let [psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)] =
                cleanup_actions.as_mut_slice()
            else {
                unreachable!("fixture has one residual cleanup")
            };
            mutate(residual);
            refresh_node_derivatives(unit, 0, 0, return_node);
        };

    let mut wrong_path = baseline.clone();
    mutate_residual(&mut wrong_path, &|residual| {
        residual.path = vec![psi_terminal::StructuralPathSegment::Field("right".into())];
    });
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_path),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut wrong_type = baseline.clone();
    let pair_type = wrong_type.functions[0].structural_parameters[0].structural_type;
    mutate_residual(&mut wrong_type, &|residual| {
        residual.structural_type = pair_type;
    });
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_type),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut missing = baseline;
    let return_node = missing.functions[0].blocks[0].nodes.len() - 1;
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut missing.functions[0].blocks[0].nodes[return_node].operation
    else {
        unreachable!("fixture returns Unit")
    };
    cleanup_actions.clear();
    refresh_node_derivatives(&mut missing, 0, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&missing),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let nominal = boolean_structural_field_unit();
    let mut missing_target = nominal.clone();
    missing_target.functions.pop();
    refresh_identity(&mut missing_target);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_target),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut wrong_attachment = nominal.clone();
    wrong_attachment.functions[1].attachment = None;
    refresh_identity(&mut wrong_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_attachment),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut unnormalized = nominal;
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut unnormalized.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!("nominal fixture returns a scalar")
    };
    let [psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup)] =
        cleanup_actions.as_mut_slice()
    else {
        unreachable!("nominal fixture has one cleanup")
    };
    cleanup.cleanup_receiver = Some(id(4_864, PlaceId::new));
    refresh_node_derivatives(&mut unnormalized, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&unnormalized),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn accepts_content_only_internal_claim_transfer_and_rejects_interface_corruption() {
    let mut baseline = structural_call_unit();
    install_content_owner(&mut baseline);
    let claim = id(1, ClaimId::new);
    for function in &mut baseline.functions {
        let root = function.structural_parameters[0].place;
        function
            .content_entry_claims
            .push(content_entry_claim(claim, root));
    }
    let AbstractOperation::CallUnit {
        claim_transfers, ..
    } = &mut baseline.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    claim_transfers.push(psi_terminal::ClaimTransfer {
        claim,
        argument_index: 0,
    });
    refresh_node_derivatives(&mut baseline, 0, 0, 0);
    validate_psi_optimization_unit(&baseline)
        .expect("content-only claims participate in the live transfer namespace");

    let mut missing_transfer = baseline.clone();
    let AbstractOperation::CallUnit {
        claim_transfers, ..
    } = &mut missing_transfer.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    claim_transfers.clear();
    refresh_node_derivatives(&mut missing_transfer, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_transfer),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut substituted_projection = baseline.clone();
    substituted_projection.functions[0].content_entry_claims[0].projections[0]
        .algebra
        .parameter = "validation::substituted-content".into();
    refresh_identity(&mut substituted_projection);
    assert!(matches!(
        validate_psi_optimization_unit(&substituted_projection),
        Err(OptimizationUnitValidationError::ContentProjectionOwnerMismatch(_))
    ));

    let mutate_projection = [
        |projection: &mut psi_terminal::ClaimContentProjection| {
            projection.projection.domain = id(99, psi_core::ContentDomainId::new);
        },
        |projection: &mut psi_terminal::ClaimContentProjection| {
            projection.projection.projection_fingerprint ^= 1;
        },
        |projection: &mut psi_terminal::ClaimContentProjection| {
            projection.algebra.kind = psi_core::ContentAlgebraKind::IntervalSet;
        },
    ];
    for mutate in mutate_projection {
        let mut candidate = baseline.clone();
        mutate(&mut candidate.functions[0].content_entry_claims[0].projections[0]);
        refresh_identity(&mut candidate);
        assert!(matches!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::ContentProjectionOwnerMismatch(_))
        ));
    }

    let mut mismatched_interface = baseline.clone();
    let semantic_domain = id(2, psi_core::DomainSemanticId::new);
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::alternate-content".into(),
    };
    let expression = psi_core::ContentProjectionExpression::CountedQuantity(
        psi_core::ContentProjectionScalar::Natural("2".into()),
    );
    let identity = psi_core::ContentProjectionIdentity {
        domain: id(semantic_domain.get(), psi_core::ContentDomainId::new),
        projection_fingerprint: psi_language_semantics::content::terminal_projection_fingerprint(
            &algebra,
            &expression,
        ),
    };
    let mut domains = mismatched_interface.structural_domains.to_vec();
    domains.push(psi_terminal::StructuralDomainDeclaration {
        id: id(2, StructuralDomainId::new),
        semantic_domain,
        identity: "validation::alternate-content-domain".into(),
        carrier: mismatched_interface.structural_types[0].id,
        content_projection: Some(psi_terminal::StructuralContentProjection {
            identity,
            algebra: algebra.clone(),
            expression,
        }),
    });
    mismatched_interface.structural_domains = domains.into();
    let callee_projection =
        &mut mismatched_interface.functions[1].content_entry_claims[0].projections[0];
    callee_projection.projection = identity;
    callee_projection.algebra = algebra;
    refresh_identity(&mut mismatched_interface);
    assert!(matches!(
        validate_psi_optimization_unit(&mismatched_interface),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

#[test]
fn accepts_content_only_boundary_completion_and_rejects_correspondence_corruption() {
    let mut baseline = structural_call_unit();
    install_content_owner(&mut baseline);
    let claim = id(1, ClaimId::new);
    let caller_root = baseline.functions[0].structural_parameters[0].place;
    let content = content_entry_claim(claim, caller_root);
    baseline.functions[0]
        .content_entry_claims
        .push(content.clone());
    let boundary_id = id(346, BoundaryMachineId::new);
    baseline
        .boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary_id,
            identity: "validation::content-only-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![baseline.functions[1].structural_parameters[0].clone()],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    let (psi_operation, structural_arguments) =
        match &baseline.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::CallUnit {
                psi_operation,
                structural_arguments,
                ..
            } => (*psi_operation, structural_arguments.clone()),
            _ => panic!("fixture begins with a structural Unit call"),
        };
    baseline.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary: boundary_id,
        arguments: Vec::new(),
        structural_arguments,
        completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
            claim,
            entry: None,
            content: Some(content),
        }],
        completion_receipts: vec![psi_terminal::CompletionReceipt {
            claim,
            argument_index: 0,
        }],
    };
    refresh_node_derivatives(&mut baseline, 0, 0, 0);
    validate_psi_optimization_unit(&baseline)
        .expect("content-only claims participate in the live completion namespace");

    let mut narrowed = baseline.clone();
    let AbstractOperation::BoundaryCall {
        completion_claim_sources,
        completion_receipts,
        ..
    } = &mut narrowed.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture contains a boundary call")
    };
    completion_claim_sources.clear();
    completion_receipts.clear();
    refresh_node_derivatives(&mut narrowed, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&narrowed),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_claim = baseline;
    let AbstractOperation::BoundaryCall {
        completion_receipts,
        ..
    } = &mut wrong_claim.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture contains a boundary call")
    };
    completion_receipts[0].claim = id(2, ClaimId::new);
    refresh_node_derivatives(&mut wrong_claim, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_claim),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

#[test]
fn rejects_structural_call_result_signature_and_claim_interface_corruption() {
    let baseline = structural_result_call_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("exact linear structural result should validate");

    let mut wrong_type = baseline.clone();
    let alternate = id(360, psi_core::StructuralTypeId::new);
    wrong_type
        .structural_types
        .push(psi_terminal::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-call-result".into(),
            shape: psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        });
    let AbstractOperation::CallStructural { result, .. } =
        &mut wrong_type.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural-result call")
    };
    let result_place = result.place;
    result.structural_type = alternate;
    let StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = &mut wrong_type.functions[0]
        .structural_places
        .iter_mut()
        .find(|place| place.id == result_place)
        .expect("caller retains its operation-result place")
        .kind
    else {
        unreachable!("call result has its operation-result root kind")
    };
    *structural_type = alternate;
    let AbstractFunctionResult::Structural(result) = &mut wrong_type.functions[0].result else {
        unreachable!("fixture has a structural result")
    };
    result.structural_type = alternate;
    refresh_node_derivatives(&mut wrong_type, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_type),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_multiplicity = baseline.clone();
    let AbstractOperation::CallStructural { result, .. } =
        &mut wrong_multiplicity.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural-result call")
    };
    result.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
    let AbstractFunctionResult::Structural(result) = &mut wrong_multiplicity.functions[0].result
    else {
        unreachable!("fixture has a structural result")
    };
    result.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut wrong_multiplicity, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_multiplicity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut invented_claim = baseline;
    let AbstractOperation::CallStructural { result, .. } =
        &mut invented_claim.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural-result call")
    };
    result
        .claims
        .push(psi_terminal::StructuralResultClaimBinding {
            claim: id(1, ClaimId::new),
            path: Vec::new(),
        });
    refresh_node_derivatives(&mut invented_claim, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&invented_claim),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}
