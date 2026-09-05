use super::*;

pub(super) const LITERALS: &[(u64, u64, IeeeFloatValue)] = &[
    (60_003, 60_004, IeeeFloatValue::Binary32(0x8000_0000)),
    (60_005, 60_006, IeeeFloatValue::Binary32(0x7fc1_2345)),
    (
        60_007,
        60_008,
        IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
    ),
];

pub(super) fn machine() -> MachineId {
    MachineId::new(60_001).unwrap()
}

pub(super) fn return_edge() -> EdgeId {
    EdgeId::new(60_009).unwrap()
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    let entry = BlockId::new(60_002).unwrap();
    let mut operations = LITERALS
        .iter()
        .map(
            |(operation, result, value)| AbstractOperation::IeeeFloatConstant {
                psi_operation: OperationId::new(*operation).unwrap(),
                result: ValueId::new(*result).unwrap(),
                value: *value,
            },
        )
        .collect::<Vec<_>>();
    operations.push(AbstractOperation::ReturnUnit {
        psi_edge: return_edge(),
        cleanup_actions: Vec::new(),
    });
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x60; 32]),
        },
        entry: machine(),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: StructuralTypeId::new(60_011).unwrap(),
                identity: "test::sequence_i32".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                )),
            },
            StructuralTypeDeclaration {
                id: StructuralTypeId::new(60_010).unwrap(),
                identity: "test::sequence_bool".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            },
        ],
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
            operations,
        }],
    }
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError {
    let mut source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_ieee_float_literal_sequence_unit_return::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIeeeFloatLiteralSequenceUnitReturn,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIeeeFloatLiteralSequenceUnitReturn(
                error,
            ),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("IEEE sequence corruption must fail at its independent family validator")
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
            value: ValueId::new(60_050).unwrap(),
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
        place: PlaceId::new(60_051).unwrap(),
        structural_type: StructuralTypeId::new(60_052).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        projected_qualifications: Vec::new(),
        shape,
        placement,
    }
}
