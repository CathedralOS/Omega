use super::*;

pub(super) fn service() -> ServiceId {
    ServiceId::new(54_003).unwrap()
}

pub(super) fn port_operation() -> OperationId {
    OperationId::new(54_004).unwrap()
}

pub(super) fn return_edge() -> EdgeId {
    EdgeId::new(54_005).unwrap()
}

pub(super) fn base_plan() -> AbstractOperationPlan {
    let machine = MachineId::new(54_001).unwrap();
    let entry = BlockId::new(54_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x54; 32]),
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
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: vec![service()],
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::PortWrite {
                    psi_operation: port_operation(),
                    service: service(),
                    port: 0x03f8,
                    value: 0x41,
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
) -> StraightLinePortWriteUnitReturnTranslationError {
    let mut source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_port_write_unit_return::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut target_operations::TargetOperationPlan),
) -> StraightLinePortWriteUnitReturnTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLinePortWriteUnitReturn,
        error: AbstractToTargetTranslationFamilyError::StraightLinePortWriteUnitReturn(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("Port-write Unit-return corruption must fail at its independent validator")
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
            value: ValueId::new(54_050).unwrap(),
            scalar_type,
            placement: call_plan.result.clone().unwrap(),
        },
        parameters: Vec::new(),
        call_plan,
    }
}
