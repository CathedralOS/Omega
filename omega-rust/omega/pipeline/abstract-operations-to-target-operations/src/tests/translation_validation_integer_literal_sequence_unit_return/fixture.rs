use super::*;

pub(super) fn literals() -> [(u64, u64, IntegerType, IntegerValue); 3] {
    [
        (
            62_003,
            62_004,
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(-128),
        ),
        (
            62_005,
            62_006,
            IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
            IntegerValue::Unsigned(65_535),
        ),
        (
            62_007,
            62_008,
            IntegerType::new(IntegerSign::Signed, 64).unwrap(),
            IntegerValue::Signed(i64::MIN as i128),
        ),
    ]
}

pub(super) fn machine() -> MachineId {
    MachineId::new(62_001).unwrap()
}

pub(super) fn return_edge() -> EdgeId {
    EdgeId::new(62_009).unwrap()
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    let entry = BlockId::new(62_002).unwrap();
    let mut operations = literals()
        .into_iter()
        .map(
            |(operation, result, scalar_type, value)| AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(operation).unwrap(),
                result: ValueId::new(result).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
                value,
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
            program_fingerprint: SemanticFingerprint::from_bytes([0x62; 32]),
        },
        entry: machine(),
        structural_types: vec![StructuralTypeDeclaration {
            id: StructuralTypeId::new(62_010).unwrap(),
            identity: "test::integer_sequence_bool".into(),
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
            operations,
        }],
    }
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerLiteralSequenceUnitReturnTranslationError {
    let target_profile = NativeTarget::linux_x64();
    let mut source = base_plan();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_integer_literal_sequence_unit_return::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineIntegerLiteralSequenceUnitReturnTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerLiteralSequenceUnitReturn,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIntegerLiteralSequenceUnitReturn(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("integer-sequence corruption must fail in its independent family validator")
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
            value: ValueId::new(62_050).unwrap(),
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
        place: PlaceId::new(62_051).unwrap(),
        structural_type: StructuralTypeId::new(62_052).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        projected_qualifications: Vec::new(),
        shape,
        placement,
    }
}
