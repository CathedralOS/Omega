use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(64_001).unwrap()
}

pub(super) fn source_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

pub(super) fn target_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 64).unwrap()
}

pub(super) fn base_plan(
    source_type: IntegerType,
    target_type: IntegerType,
    value: IntegerValue,
) -> AbstractOperationPlan {
    let entry = BlockId::new(64_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x64; 32]),
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
                value: ValueId::new(64_008).unwrap(),
                scalar_type: ScalarType::Integer(target_type),
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
                    psi_operation: OperationId::new(64_003).unwrap(),
                    result: ValueId::new(64_004).unwrap(),
                    scalar_type: ScalarType::Integer(source_type),
                    value,
                },
                AbstractOperation::IntegerWiden {
                    psi_operation: OperationId::new(64_005).unwrap(),
                    result: ValueId::new(64_006).unwrap(),
                    source_type,
                    target_type,
                    operand: ValueId::new(64_004).unwrap(),
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(64_007).unwrap(),
                    result: ValueId::new(64_008).unwrap(),
                    value: ValueId::new(64_006).unwrap(),
                    scalar_type: ScalarType::Integer(target_type),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn default_plan() -> AbstractOperationPlan {
    base_plan(source_type(), target_type(), IntegerValue::Unsigned(65_535))
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerWidenImmediateTranslationError {
    let mut source = default_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_integer_widen_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerWidenImmediateTranslationError {
    let source = default_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerWidenImmediate,
        error: AbstractToTargetTranslationFamilyError::StraightLineIntegerWidenImmediate(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("constant widening corruption must fail in its exact validator")
    };
    error
}
