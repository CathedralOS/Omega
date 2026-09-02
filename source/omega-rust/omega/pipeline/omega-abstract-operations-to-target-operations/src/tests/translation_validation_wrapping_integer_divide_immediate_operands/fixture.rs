use super::*;

pub(super) fn machine() -> MachineId {
    MachineId::new(84_001).unwrap()
}

pub(super) fn scalar_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 16).unwrap()
}

pub(super) fn base_plan(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> AbstractOperationPlan {
    let entry = BlockId::new(84_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x84; 32]),
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
                value: ValueId::new(84_010).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
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
                    psi_operation: OperationId::new(84_003).unwrap(),
                    result: ValueId::new(84_004).unwrap(),
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: left,
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(84_005).unwrap(),
                    result: ValueId::new(84_006).unwrap(),
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: right,
                },
                AbstractOperation::WrappingIntegerDivide {
                    psi_operation: OperationId::new(84_007).unwrap(),
                    obligation: ObligationId::new(84_011).unwrap(),
                    result: ValueId::new(84_008).unwrap(),
                    scalar_type,
                    left: ValueId::new(84_004).unwrap(),
                    right: ValueId::new(84_006).unwrap(),
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(84_009).unwrap(),
                    result: ValueId::new(84_010).unwrap(),
                    value: ValueId::new(84_008).unwrap(),
                    scalar_type: ScalarType::Integer(scalar_type),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

pub(super) fn default_plan() -> AbstractOperationPlan {
    base_plan(
        scalar_type(),
        IntegerValue::Signed(-32_768),
        IntegerValue::Signed(-1),
    )
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineWrappingIntegerDivideImmediateOperandsTranslationError {
    let mut source = default_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_wrapping_integer_divide_immediate_operands::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineWrappingIntegerDivideImmediateOperandsTranslationError {
    let source = default_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineWrappingIntegerDivideImmediateOperands,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerDivideImmediateOperands(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("constant-operand wrapping divide corruption must fail in its exact validator")
    };
    error
}
