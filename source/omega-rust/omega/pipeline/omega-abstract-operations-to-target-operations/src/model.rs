use omega_calling_conventions::PlanDiagnostic;
use omega_target::NativeTarget;
use omega_target_operations::BoundarySettlementRealization;
use psi_core::{BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId, ValueId};

/// One boundary realization sourced from a validated, admitted provider
/// execution. Callers supply the exact target mechanism but cannot substitute
/// a secondary provider-plan identity.
#[derive(Debug, Clone)]
pub struct AdmittedBoundarySettlement<'execution> {
    pub boundary: BoundaryMachineId,
    pub provider_execution: &'execution dyn omega_installation_evidence::ProviderExecutionEvidence,
    pub realization: BoundarySettlementRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    TranslationValidation(crate::AbstractToTargetTranslationValidationError),
    InvalidRankedCountdown(MachineId),
    EntryFunctionMissing(MachineId),
    ProviderInstallationIdentityMismatch,
    DuplicateInstalledProviderCall {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    UnknownInstalledProviderCall {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    InstalledProviderCallEvidenceMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    InstalledProviderCallShapeMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    InstalledProviderClaimTransferMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    BoundarySettlementOverlapsInstalledProvider(BoundaryMachineId),
    PartialInstalledProviderBoundary {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    DuplicateBoundarySettlement(BoundaryMachineId),
    UnknownBoundarySettlement(BoundaryMachineId),
    MissingBoundarySettlement(BoundaryMachineId),
    UnusedBoundarySettlement(BoundaryMachineId),
    BoundaryRealizationMismatch(BoundaryMachineId),
    InvalidClaimCompletionOnlyShape {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    ProviderExecutionBinding(String),
    ProviderExecutionRequirementMismatch {
        boundary: BoundaryMachineId,
        expected: String,
        actual: String,
    },
    OperationAfterReturn(MachineId),
    FunctionHasNoReturn(MachineId),
    FunctionResultMismatch(MachineId),
    FunctionResultKindMismatch(MachineId),
    FixedIntegerScalarAbiPlanMissingResult(MachineId),
    UnitFunctionHasScalarParameters(MachineId),
    UnitFunctionNotStraightLine(MachineId),
    UnitOperationInScalarFunction {
        machine: MachineId,
        operation: OperationId,
    },
    ResultBearingBoundarySettlementRequiresNativeRealization {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    ScalarBoundaryArgumentsRequireNativeRealization {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    LinuxExitGroupUnsupportedTarget {
        machine: MachineId,
        target: NativeTarget,
    },
    LinuxWriteLineUnsupportedOrInvalid {
        machine: MachineId,
        boundary: BoundaryMachineId,
        target: NativeTarget,
    },
    InvalidLinuxExitGroupShape(MachineId),
    UnsupportedOperationInScalarFunction(MachineId),
    UnsupportedOperationInUnitFunction(MachineId),
    /// Target-neutral custody retains this verified semantic write, but no
    /// target operation may realize it until parameter address, scalar width,
    /// and non-observing store authority are selected and replayable.
    UnsupportedWriteOnlyPrimitiveStore {
        machine: MachineId,
        operation: OperationId,
    },
    UnsupportedStructuralReturn(MachineId),
    UnsupportedStructuralReturnShape {
        machine: MachineId,
        byte_size: u16,
    },
    UnsupportedStructuralReturnPlacement(MachineId),
    UnitCallTargetKindMismatch(MachineId),
    UnitScalarCallRequiresAttachedMachine {
        machine: MachineId,
        operation: OperationId,
    },
    UnitScalarCallTargetShapeUnsupported(MachineId),
    UnitScalarCallTargetPublishesServices(MachineId),
    UnitScalarCallIntegerTypeUnsupported(ValueId),
    UnitScalarCallResultTypeMismatch {
        callee: MachineId,
        result: ValueId,
    },
    UnitScalarCallResultPlacementUnsupported {
        callee: MachineId,
        result: ValueId,
    },
    UnitScalarCallTargetAbiMismatch(MachineId),
    StructuralCallArgumentCountMismatch {
        callee: MachineId,
        expected: usize,
        actual: usize,
    },
    UnknownStructuralArgumentPlace {
        machine: MachineId,
        place: PlaceId,
    },
    StructuralCallArgumentTypeMismatch {
        callee: MachineId,
        place: PlaceId,
    },
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    EmptyStructuralType(StructuralTypeId),
    RelevantOpaqueStructuralField(StructuralTypeId),
    UnsupportedStructuralByteSequence(StructuralTypeId),
    UnsupportedStructuralPrimitiveScalar(StructuralTypeId),
    UnsupportedStructuralSum(StructuralTypeId),
    StructuralTypeTooLarge(StructuralTypeId),
    ConditionalControlFlowRequiresBlockLowering(MachineId),
    ConditionalConditionMustBeBoolean(ValueId),
    ConditionalArmBindingTypeMismatch(psi_core::EdgeId),
    DuplicateValue(ValueId),
    UnknownCallTarget(MachineId),
    CallArgumentCountMismatch {
        callee: MachineId,
        expected: usize,
        actual: usize,
    },
    CallArgumentTypeMismatch {
        callee: MachineId,
        argument: ValueId,
    },
    UnknownValue(ValueId),
    ValueTypeMismatch(ValueId),
    UnsupportedRuntimeBooleanCondition(ValueId),
    IntegerConstantHasNonIntegerType(ValueId),
    IntegerConstantOutsideType(ValueId),
    IntegerBitwiseOperandTypeMismatch(ValueId),
    IntegerWidenTypeMismatch(ValueId),
    IntegerExactCastTypeMismatch(ValueId),
    WrappingShiftOperandTypeMismatch(ValueId),
    ExactShiftOperandTypeMismatch(ValueId),
    WrappingAddOperandTypeMismatch(ValueId),
    SaturatingAddOperandTypeMismatch(ValueId),
    WrappingSubtractOperandTypeMismatch(ValueId),
    SaturatingSubtractOperandTypeMismatch(ValueId),
    WrappingMultiplyOperandTypeMismatch(ValueId),
    SaturatingMultiplyOperandTypeMismatch(ValueId),
    ExactDivideOperandTypeMismatch(ValueId),
    ExactRemainderOperandTypeMismatch(ValueId),
    WrappingDivideOperandTypeMismatch(ValueId),
    WrappingRemainderOperandTypeMismatch(ValueId),
    SaturatingDivideOperandTypeMismatch(ValueId),
    SaturatingRemainderOperandTypeMismatch(ValueId),
    ParameterWidthNotNativelySupported {
        value: ValueId,
        bits: u16,
    },
    UnsupportedScalarParameterPlacement(ValueId),
    AbiPlan(PlanDiagnostic),
    AbiParameterCountMismatch {
        expected: usize,
        actual: usize,
    },
}

impl From<crate::AbstractToTargetTranslationValidationError> for LoweringError {
    fn from(error: crate::AbstractToTargetTranslationValidationError) -> Self {
        Self::TranslationValidation(error)
    }
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
