use super::*;

pub(super) fn caller() -> MachineId {
    MachineId::new(55_001).unwrap()
}

pub(super) fn call_operation() -> OperationId {
    OperationId::new(55_003).unwrap()
}

pub(super) fn caller_return_edge() -> EdgeId {
    EdgeId::new(55_004).unwrap()
}

pub(super) fn callee() -> MachineId {
    MachineId::new(55_005).unwrap()
}

pub(super) fn requirement() -> ObligationId {
    ObligationId::new(55_008).unwrap()
}

pub(super) fn crash_continuation() -> CrashRouteBucket {
    CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    let caller_entry = BlockId::new(55_002).unwrap();
    let callee_entry = BlockId::new(55_006).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x55; 32]),
        },
        entry: caller(),
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller(),
                attachment: None,
                entry: caller_entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::CallUnit {
                        psi_operation: call_operation(),
                        callee: callee(),
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: vec![requirement()],
                        crash_continuations: vec![crash_continuation()],
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: caller_return_edge(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee(),
                attachment: None,
                entry: callee_entry,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(55_007).unwrap(),
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineUnitCallReturnTranslationError {
    let mut source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_unit_call_return::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineUnitCallReturnTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineUnitCallReturn,
        error: AbstractToTargetTranslationFamilyError::StraightLineUnitCallReturn(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("Unit-call corruption must fail at its independent validator")
    };
    error
}

pub(super) fn fixed_integer_scalar_abi(target: NativeTarget) -> FixedIntegerScalarFunctionAbi {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .unwrap();
    FixedIntegerScalarFunctionAbi {
        result: FixedIntegerScalarAbiValue {
            value: ValueId::new(55_050).unwrap(),
            scalar_type,
            placement: call_plan.result.clone().unwrap(),
        },
        parameters: Vec::new(),
        call_plan,
    }
}

pub(super) fn target_structural_argument() -> TargetStructuralArgument {
    let shape = ValueShape::integer(4, 4);
    let placement = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap()
    .parameters
    .remove(0);
    TargetStructuralArgument {
        place: PlaceId::new(55_051).unwrap(),
        access: StructuralAccess::Owned,
        path: Vec::new(),
        root_structural_type: StructuralTypeId::new(55_052).unwrap(),
        structural_type: StructuralTypeId::new(55_052).unwrap(),
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: placement.clone(),
        destination: placement,
    }
}
