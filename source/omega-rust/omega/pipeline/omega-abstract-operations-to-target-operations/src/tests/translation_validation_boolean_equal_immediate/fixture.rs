use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(69_001).unwrap()
}

pub(super) fn base_plan(left_value: bool, right_value: bool) -> AbstractOperationPlan {
    let entry = BlockId::new(69_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x69; 32]),
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
                value: ValueId::new(69_010).unwrap(),
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
                AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(69_003).unwrap(),
                    result: ValueId::new(69_004).unwrap(),
                    value: left_value,
                },
                AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(69_005).unwrap(),
                    result: ValueId::new(69_006).unwrap(),
                    value: right_value,
                },
                AbstractOperation::BooleanEqual {
                    psi_operation: OperationId::new(69_007).unwrap(),
                    result: ValueId::new(69_008).unwrap(),
                    left: ValueId::new(69_004).unwrap(),
                    right: ValueId::new(69_006).unwrap(),
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(69_009).unwrap(),
                    result: ValueId::new(69_010).unwrap(),
                    value: ValueId::new(69_008).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn default_plan() -> AbstractOperationPlan {
    base_plan(true, false)
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineBooleanEqualImmediateTranslationError {
    let mut source = default_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_boolean_equal_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineBooleanEqualImmediateTranslationError {
    let source = default_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineBooleanEqualImmediate,
        error: AbstractToTargetTranslationFamilyError::StraightLineBooleanEqualImmediate(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("constant Boolean-equality corruption must fail in its exact validator")
    };
    error
}
