use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(83_001).unwrap()
}

pub(super) fn value_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

pub(super) fn count_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
}

pub(super) fn base_plan(
    value_type: IntegerType,
    count_type: IntegerType,
    value: IntegerValue,
    count: IntegerValue,
) -> AbstractOperationPlan {
    let entry = BlockId::new(83_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x83; 32]),
        },
        entry: machine(),
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: machine(),
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(83_010).unwrap(),
                scalar_type: ScalarType::Integer(value_type),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(83_003).unwrap(),
                    result: ValueId::new(83_004).unwrap(),
                    scalar_type: ScalarType::Integer(value_type),
                    value,
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(83_005).unwrap(),
                    result: ValueId::new(83_006).unwrap(),
                    scalar_type: ScalarType::Integer(count_type),
                    value: count,
                },
                AbstractOperation::WrappingIntegerShiftLeft {
                    psi_operation: OperationId::new(83_007).unwrap(),
                    result: ValueId::new(83_008).unwrap(),
                    value_type,
                    count_type,
                    value: ValueId::new(83_004).unwrap(),
                    count: ValueId::new(83_006).unwrap(),
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(83_009).unwrap(),
                    result: ValueId::new(83_010).unwrap(),
                    value: ValueId::new(83_008).unwrap(),
                    scalar_type: ScalarType::Integer(value_type),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn default_plan() -> AbstractOperationPlan {
    base_plan(
        value_type(),
        count_type(),
        IntegerValue::Unsigned(65_535),
        IntegerValue::Unsigned(16),
    )
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineWrappingIntegerShiftLeftImmediateTranslationError {
    let mut source = default_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_wrapping_integer_shift_left_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineWrappingIntegerShiftLeftImmediateTranslationError {
    let source = default_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftLeftImmediate,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerShiftLeftImmediate(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("constant wrapping integer shift-left corruption must fail in its exact validator")
    };
    error
}
