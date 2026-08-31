use super::*;

pub(super) fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("valid fixture type")
}

pub(super) fn literal_function(
    base: u64,
    scalar_type: IntegerType,
    value: IntegerValue,
) -> AbstractFunction {
    let machine = MachineId::new(base + 1).unwrap();
    let entry = BlockId::new(base + 2).unwrap();
    let constant_operation = OperationId::new(base + 3).unwrap();
    let constant_value = ValueId::new(base + 4).unwrap();
    let function_result = ValueId::new(base + 5).unwrap();
    let return_edge = EdgeId::new(base + 6).unwrap();
    let scalar_type = ScalarType::Integer(scalar_type);
    AbstractFunction {
        machine,
        attachment: None,
        entry,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: function_result,
            scalar_type,
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
                psi_operation: constant_operation,
                result: constant_value,
                scalar_type,
                value,
            },
            AbstractOperation::Return {
                psi_edge: return_edge,
                result: function_result,
                value: constant_value,
                scalar_type,
                cleanup_actions: Vec::new(),
            },
        ],
    }
}

pub(super) fn literal_plan(functions: Vec<AbstractFunction>) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x5a; 32]),
        },
        entry: functions[0].machine,
        structural_types: Vec::new(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions,
    }
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    literal_plan(vec![literal_function(
        100,
        integer_type(IntegerSign::Unsigned, 64),
        IntegerValue::Unsigned(37),
    )])
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerImmediateTranslationError {
    let mut source = base_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_integer_immediate::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> AbstractToTargetTranslationValidationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
}
