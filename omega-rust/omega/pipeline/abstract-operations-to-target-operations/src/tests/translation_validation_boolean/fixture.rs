use super::*;

pub(super) fn boolean_plan(value: bool) -> AbstractOperationPlan {
    let machine = MachineId::new(1_001).unwrap();
    let entry = BlockId::new(1_002).unwrap();
    let constant_operation = OperationId::new(1_003).unwrap();
    let constant_value = ValueId::new(1_004).unwrap();
    let function_result = ValueId::new(1_005).unwrap();
    let return_edge = EdgeId::new(1_006).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0xb0; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: function_result,
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
                    psi_operation: constant_operation,
                    result: constant_value,
                    value,
                },
                AbstractOperation::Return {
                    psi_edge: return_edge,
                    result: function_result,
                    value: constant_value,
                    scalar_type: ScalarType::Boolean,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineBooleanImmediateTranslationError {
    let mut source = boolean_plan(true);
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_boolean_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLineBooleanImmediateTranslationError {
    let source = boolean_plan(true);
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
        error: AbstractToTargetTranslationFamilyError::StraightLineBooleanImmediate(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("Boolean-family corruption must fail at its independent validator")
    };
    error
}
