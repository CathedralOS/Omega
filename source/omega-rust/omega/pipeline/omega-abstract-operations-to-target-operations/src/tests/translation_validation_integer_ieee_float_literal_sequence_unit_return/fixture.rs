use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(63_001).unwrap()
}

pub(super) fn return_edge() -> EdgeId {
    EdgeId::new(63_009).unwrap()
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    let entry = BlockId::new(63_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x63; 32]),
        },
        entry: machine(),
        structural_types: vec![StructuralTypeDeclaration {
            id: StructuralTypeId::new(63_010).unwrap(),
            identity: "test::mixed_literal_sequence_bool".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: machine(),
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(63_003).unwrap(),
                    result: ValueId::new(63_004).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                    ),
                    value: IntegerValue::Signed(-128),
                },
                AbstractOperation::IeeeFloatConstant {
                    psi_operation: OperationId::new(63_005).unwrap(),
                    result: ValueId::new(63_006).unwrap(),
                    value: IeeeFloatValue::Binary32(0x7fc1_2345),
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(63_007).unwrap(),
                    result: ValueId::new(63_008).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                    ),
                    value: IntegerValue::Unsigned(65_535),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: return_edge(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError {
    let target_profile = NativeTarget::linux_x64();
    let mut source = base_plan();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_integer_ieee_float_literal_sequence_unit_return::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("mixed literal corruption must fail in its exact independent validator")
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
            value: ValueId::new(63_050).unwrap(),
            scalar_type,
            placement: call_plan.result.clone().unwrap(),
        },
        parameters: Vec::new(),
        call_plan,
    }
}

pub(super) fn target_structural_parameter() -> TargetStructuralParameter {
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
    TargetStructuralParameter {
        place: PlaceId::new(63_051).unwrap(),
        structural_type: StructuralTypeId::new(63_052).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        shape,
        placement,
    }
}
