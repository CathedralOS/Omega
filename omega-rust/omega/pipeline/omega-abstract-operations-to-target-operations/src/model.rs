use omega_calling_conventions::PlanDiagnostic;
use omega_target::NativeTarget;
use omega_target_operations::BoundarySettlementRealization;
use psi_core::{BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId, ValueId};

/// Exact sealed placement plan selected for one retained Terminal placed-view
/// input. Construction grants no backing or access authority; lowering rejoins
/// the complete plan identity and content commitment to the retained row.
#[derive(Debug, Clone, Copy)]
pub struct SelectedPlacedViewInputPlan<'plan> {
    pub terminal_input: &'plan psi_terminal::TerminalPlacedViewInput,
    pub placement_plan: &'plan psi_access_plans::ValidatedPlacementPlan,
}

/// Borrowed exact-plan and deployment inputs for one Terminal nearest-FMA
/// occurrence. Construction grants no authority: the Abstract-to-Target
/// coordinator independently rejoins every field before producing target IR.
#[derive(Debug, Clone, Copy)]
pub struct AdmittedIeeeFloatFmaSettlement<'plan> {
    pub terminal_operation: OperationId,
    pub provider_plan: &'plan omega_effects::provider_plan::ProviderPlan,
    pub format: psi_core::IeeeFloatFormat,
    pub slot: omega_target::X86ScalarFmaSlot,
    pub provider: omega_target::AdmittedX86ScalarFmaProvider,
}

/// Owned target-side input for one compiler-private callback argument.
///
/// The exact Terminal operation is the join to the unchanged abstract
/// `BoundaryCall`. The application and complete registrar plan/context remain
/// data until this stage independently validates their one-slot relation. The
/// application commitment is retained compiler provenance, not authority this
/// reduced tuple can recompute; callers must supply it from the exact retained
/// placement owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeCallbackArgument {
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: omega_function_identity::MachineFunctionIdentity,
    pub application: omega_calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: omega_calling_conventions::BoundaryEntryPlan,
    pub registrar_context: omega_calling_conventions::CallbackMaterializationContext,
    /// Nonempty compiler-origin application-v3 commitment projection.
    pub registrar_application_commitment: [u8; 32],
}

/// One boundary realization sourced from a validated, admitted provider
/// execution or the consuming lowerer's closed compiler-builtin catalog.
#[derive(Debug, Clone)]
pub struct AdmittedBoundarySettlement<'execution> {
    pub boundary: BoundaryMachineId,
    pub execution: AdmittedBoundaryExecution<'execution>,
    pub realization: BoundarySettlementRealization,
}

#[derive(Debug, Clone, Copy)]
pub enum AdmittedBoundaryExecution<'execution> {
    Provider(&'execution dyn omega_installation_evidence::ProviderExecutionEvidence),
    CompilerBuiltin(omega_target_operations::CompilerBuiltinExecution),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    TranslationValidation(crate::AbstractToTargetTranslationValidationError),
    /// Parameter-rooted path qualifications are preserved through the
    /// prephysical optimizer boundary but have no target-operation carrier yet.
    UnsupportedProjectedStructuralQualifications,
    PlacedViewInput(PlacedViewInputTranslationError),
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
    DuplicateIeeeFloatFmaSettlement(OperationId),
    UnknownIeeeFloatFmaSettlement(OperationId),
    MissingIeeeFloatFmaSettlement(OperationId),
    InvalidIeeeFloatFmaSettlement(OperationId),
    DuplicateNativeCallbackArgument(OperationId),
    MultipleNativeCallbackArguments,
    UnknownNativeCallbackArgument(OperationId),
    InvalidNativeCallbackArgument(OperationId),
    MissingNativeCallbackArgument(OperationId),
    UnusedNativeCallbackArgument(OperationId),
    IeeeFloatFmaOperandMismatch(ValueId),
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
    InvalidDynamicDispatch {
        machine: MachineId,
        operation: OperationId,
    },
    /// Target-neutral custody retains aggregate descriptor storage and reload,
    /// but target operations do not yet define its physical two-word local.
    UnsupportedStoredDynamicDescriptor {
        machine: MachineId,
        operation: OperationId,
    },
    /// Abstract custody admits result-less dynamic dispatch, but target
    /// operations do not yet define its descriptor ABI or indirect call form.
    UnsupportedDynamicUnitDispatch {
        machine: MachineId,
        operation: OperationId,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacedViewInputTranslationError {
    UnsupportedInputCount(usize),
    SelectionCountMismatch { expected: usize, actual: usize },
    InputIsNotDirectEntry,
    UnsupportedEntryFunctionShape(MachineId),
    SelectionRowMismatch,
    PlacementPlanIdentityMismatch,
    PlacementPlanHasNoConcreteSize,
    TargetPointerShapeUnsupported,
    AbiPlan(PlanDiagnostic),
    CandidatePlanMismatch,
    CandidateEntryCallPlanMismatch,
    CandidateInputRosterMismatch,
}

impl From<PlacedViewInputTranslationError> for LoweringError {
    fn from(error: PlacedViewInputTranslationError) -> Self {
        Self::PlacedViewInput(error)
    }
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
