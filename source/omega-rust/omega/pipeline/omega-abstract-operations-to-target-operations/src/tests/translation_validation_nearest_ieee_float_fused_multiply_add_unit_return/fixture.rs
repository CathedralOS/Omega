use super::*;

pub(super) const FMA_OPERATION_RAW: u64 = 61_009;
const RETURN_EDGE_RAW: u64 = 61_011;

pub(super) fn machine() -> MachineId {
    MachineId::new(61_001).unwrap()
}

pub(super) fn fma_operation() -> OperationId {
    OperationId::new(FMA_OPERATION_RAW).unwrap()
}

pub(super) fn fma_result() -> ValueId {
    ValueId::new(61_010).unwrap()
}

pub(super) fn return_edge() -> EdgeId {
    EdgeId::new(RETURN_EDGE_RAW).unwrap()
}

pub(super) fn profile(target: NativeTarget) -> TargetProfile {
    if target == NativeTarget::linux_x64() {
        TargetProfile::LinuxX64
    } else if target == NativeTarget::windows_x64() {
        TargetProfile::WindowsX64
    } else if target == NativeTarget::uefi_x64() {
        TargetProfile::UefiX64
    } else {
        panic!("the FMA fixture only admits an exact x86 deployment profile")
    }
}

pub(super) fn slot(format: IeeeFloatFormat) -> X86ScalarFmaSlot {
    match format {
        IeeeFloatFormat::Binary32 => X86ScalarFmaSlot::Binary32,
        IeeeFloatFormat::Binary64 => X86ScalarFmaSlot::Binary64,
    }
}

pub(super) fn values(format: IeeeFloatFormat) -> [IeeeFloatValue; 3] {
    match format {
        IeeeFloatFormat::Binary32 => [
            IeeeFloatValue::Binary32(0x8000_0000),
            IeeeFloatValue::Binary32(0x7fc1_2345),
            IeeeFloatValue::Binary32(0x3f80_0001),
        ],
        IeeeFloatFormat::Binary64 => [
            IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
            IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
            IeeeFloatValue::Binary64(0x3ff0_0000_0000_0001),
        ],
    }
}

pub(super) fn literal_operation(position: usize) -> OperationId {
    OperationId::new(61_003 + (position as u64 * 2)).unwrap()
}

pub(super) fn literal_result(position: usize) -> ValueId {
    ValueId::new(61_004 + (position as u64 * 2)).unwrap()
}

pub(super) fn base_plan(format: IeeeFloatFormat) -> AbstractOperationPlan {
    let entry = BlockId::new(61_002).unwrap();
    let mut operations = values(format)
        .into_iter()
        .enumerate()
        .map(|(position, value)| AbstractOperation::IeeeFloatConstant {
            psi_operation: literal_operation(position),
            result: literal_result(position),
            value,
        })
        .collect::<Vec<_>>();
    operations.extend([
        AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation: fma_operation(),
            result: fma_result(),
            format,
            left: literal_result(0),
            right: literal_result(1),
            addend: literal_result(2),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: return_edge(),
            cleanup_actions: Vec::new(),
        },
    ]);
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x61; 32]),
        },
        entry: machine(),
        structural_types: vec![StructuralTypeDeclaration {
            id: StructuralTypeId::new(61_012).unwrap(),
            identity: "test::fma_bool".into(),
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

pub(super) fn provider_plan(target: NativeTarget, format: IeeeFloatFormat) -> ProviderPlan {
    let slot = slot(format);
    ProviderPlan {
        name: format!("test::nearest_fma::{format:?}"),
        provider_type: "test::CanonicalX86FmaProvider".into(),
        provider_type_package_identity: None,
        target: profile(target).target_name().into(),
        schema: ServiceSchema::default(),
        rows: vec![ProviderPlanRow {
            method: "fused_multiply_add".into(),
            requirement_identity: slot.selected_plan_requirement_identity().into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CompilerIntrinsic {
                machine: slot.realization_identity().into(),
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
}

pub(super) fn settlement<'a>(
    target: NativeTarget,
    format: IeeeFloatFormat,
    plan: &'a ProviderPlan,
) -> AdmittedIeeeFloatFmaSettlement<'a> {
    AdmittedIeeeFloatFmaSettlement {
        terminal_operation: fma_operation(),
        provider_plan: plan,
        format,
        slot: slot(format),
        provider: omega_target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
            profile(target),
            &X86_SCALAR_FMA_REQUIRED_FEATURES,
        )
        .unwrap(),
    }
}

pub(super) fn lowered(
    source: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedIeeeFloatFmaSettlement<'_>],
) -> omega_target_operations::TargetOperationPlan {
    lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma(
        source,
        target,
        &[],
        None,
        settlements,
    )
    .unwrap()
}

pub(super) fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError {
    let format = IeeeFloatFormat::Binary32;
    let target = NativeTarget::linux_x64();
    let plan = provider_plan(target, format);
    let admitted = settlement(target, format, &plan);
    let mut source = base_plan(format);
    let lowered = lowered(&source, target, &[admitted]);
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_nearest_ieee_float_fused_multiply_add_unit_return::validate(
        &source.functions[0],
        target,
        &lowered.functions[0],
        &[admitted],
    )
    .unwrap_err()
}

pub(super) fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError {
    let format = IeeeFloatFormat::Binary32;
    let target = NativeTarget::linux_x64();
    let plan = provider_plan(target, format);
    let admitted = settlement(target, format, &plan);
    let source = base_plan(format);
    let mut candidate = lowered(&source, target, &[admitted]);
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn,
        error: AbstractToTargetTranslationFamilyError::StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(error),
        ..
    } = validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
        &source,
        target,
        &candidate,
        &[admitted],
    )
    .unwrap_err()
    else {
        panic!("FMA corruption must fail in the independent family validator")
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
            value: ValueId::new(61_050).unwrap(),
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
        place: PlaceId::new(61_051).unwrap(),
        structural_type: StructuralTypeId::new(61_052).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        shape,
        placement,
    }
}
