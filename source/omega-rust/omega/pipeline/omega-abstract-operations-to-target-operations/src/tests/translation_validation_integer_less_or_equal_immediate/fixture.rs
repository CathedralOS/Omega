use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(72_001).unwrap()
}

pub(super) fn scalar_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

pub(super) fn base_plan(
    scalar_type: IntegerType,
    left_value: IntegerValue,
    right_value: IntegerValue,
) -> AbstractOperationPlan {
    let entry = BlockId::new(72_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x72; 32]),
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
                value: ValueId::new(72_010).unwrap(),
                scalar_type: ScalarType::Boolean,
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
                    psi_operation: OperationId::new(72_003).unwrap(),
                    result: ValueId::new(72_004).unwrap(),
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: left_value,
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(72_005).unwrap(),
                    result: ValueId::new(72_006).unwrap(),
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: right_value,
                },
                AbstractOperation::IntegerLessOrEqual {
                    psi_operation: OperationId::new(72_007).unwrap(),
                    result: ValueId::new(72_008).unwrap(),
                    left: ValueId::new(72_004).unwrap(),
                    right: ValueId::new(72_006).unwrap(),
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(72_009).unwrap(),
                    result: ValueId::new(72_010).unwrap(),
                    value: ValueId::new(72_008).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn default_plan() -> AbstractOperationPlan {
    base_plan(
        scalar_type(),
        IntegerValue::Unsigned(255),
        IntegerValue::Unsigned(256),
    )
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerLessOrEqualImmediateTranslationError {
    let mut source = default_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_integer_less_or_equal_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerLessOrEqualImmediateTranslationError {
    let source = default_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualImmediate,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIntegerLessOrEqualImmediate(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("constant integer-less-or-equal corruption must fail in its exact validator")
    };
    error
}
