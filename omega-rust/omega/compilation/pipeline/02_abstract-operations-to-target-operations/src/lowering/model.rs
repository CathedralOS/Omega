use calling_conventions::PlanDiagnostic;
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId, ValueId,
};
use target::NativeTarget;

pub use terminal_psi_to_abstract_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    UnconsumedOptimizationPhase(optimization_core::OptimizationExecutionPhase),
    UnsupportedIeeeFloatFmaRealization(OperationId),
    TranslationValidation(crate::AbstractToTargetTranslationValidationError),
    PlacedViewInput(PlacedViewInputTranslationError),
    EntryFunctionMissing(MachineId),
    ProviderInstallationIdentityMismatch,
    DuplicateSelectedBoundaryCall {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    UnknownSelectedBoundaryCall {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    SelectedBoundaryCallEvidenceMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    SelectedBoundaryCallShapeMismatch {
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
        invocation_site: semantic_vocabulary::BoundaryInvocationSite,
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
    HostedExitProcessUnsupportedTarget {
        machine: MachineId,
        target: NativeTarget,
    },
    LinuxWriteLineUnsupportedOrInvalid {
        machine: MachineId,
        boundary: BoundaryMachineId,
        target: NativeTarget,
    },
    InvalidHostedExitProcessShape(MachineId),
    UnsupportedOperationInScalarFunction(MachineId),
    /// No lowering admits this control shape. `machine` is the authored
    /// coordinate the refusal is about. `site` is the check that refused
    /// (`LoweringError::unsupported_control_flow`), since dozens of checks
    /// share this spelling; it is withheld from an author's diagnostic
    /// because only a developer can act on which one fired.
    UnsupportedControlFlow {
        machine: MachineId,
        site: optimization_core::CompilerCheckSite,
    },
    UnsupportedOperationInUnitFunction(MachineId),
    InvalidStructuralValueInput {
        machine: MachineId,
        place: PlaceId,
        reason: &'static str,
    },
    InvalidReferenceResult {
        machine: MachineId,
        reason: &'static str,
    },
    DuplicateNativeCallbackArgument(OperationId),
    MultipleNativeCallbackArguments,
    UnknownNativeCallbackArgument(OperationId),
    InvalidNativeCallbackArgument(OperationId),
    MissingNativeCallbackArgument(OperationId),
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
    UnitScalarCallTargetShapeUnsupported(MachineId),
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
    UnsupportedStructuralSum(StructuralTypeId),
    UnsupportedStructuralArray(StructuralTypeId),
    UnsupportedStructuralReference(StructuralTypeId),
    UnclosedTargetLayout(StructuralTypeId),
    StructuralTypeTooLarge(StructuralTypeId),
    ConditionalControlFlowRequiresBlockLowering(MachineId),
    ConditionalConditionMustBeBoolean(ValueId),
    ConditionalArmBindingTypeMismatch(semantic_vocabulary::EdgeId),
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

diagnostics::debug_display!(LoweringError);

impl std::error::Error for LoweringError {}

impl LoweringError {
    /// An unsupported control shape in `machine`, carrying the caller's check
    /// site for a developer.
    #[track_caller]
    pub(crate) fn unsupported_control_flow(machine: MachineId) -> Self {
        Self::UnsupportedControlFlow {
            machine,
            site: optimization_core::CompilerCheckSite::caller(),
        }
    }
}
