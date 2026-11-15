/*
  Target-lowering input settlements (`AdmittedBoundarySettlement`,
  `AdmittedBoundaryExecution`, `AdmittedNativeCallbackArgument`) and the closed
  `LoweringError` diagnostic vocabulary.
*/
use calling_conventions::PlanDiagnostic;
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId, ValueId,
};
use target::NativeTarget;

pub use provider_admission::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    FunctionContract {
        machine: MachineId,
        reason: &'static str,
    },
    DescriptorCallOrigins {
        machine: MachineId,
        result: PlaceId,
        reason: &'static str,
    },
    NativeStructuralLayout(calling_conventions::terminal_layout::TerminalLayoutError),
    ForeignLoweringFacts(&'static str),
    ForeignNativeCallbackCohort,
    ForeignInstalledInterruptCohort,
    ReferenceCarrierUnavailable {
        machine: MachineId,
        source: terminal_psi::StructuralArgument,
        carrier: StructuralTypeId,
        carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    },
    BorrowedSourceProjectionMismatch {
        machine: MachineId,
        source: terminal_psi::StructuralArgument,
        origin: terminal_psi::StructuralArgument,
        expected: optimization_unit_semantics::StructuralStorageProjection,
        actual: Option<optimization_unit_semantics::StructuralStorageProjection>,
        origin_carrier: Option<StructuralTypeId>,
    },
    InvalidAccessLifecycle {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        reason: &'static str,
    },
    DescriptorInputGuard {
        machine: MachineId,
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        reason: &'static str,
    },
    UnconsumedOptimizationPhase(optimization_core::OptimizationExecutionPhase),
    UnsupportedIeeeFloatFmaRealization(OperationId),
    AccessViewInput(AccessViewInputTranslationError),
    AccessCallCustody {
        caller: MachineId,
        callee: MachineId,
        invocation: semantic_vocabulary::InvocationSite,
        runtime_position: usize,
        reason: AccessCallCustodyError,
    },
    AccessReturnRequiresWholeView {
        machine: MachineId,
    },
    AccessDynamicCallRequiresBinding {
        caller: MachineId,
        operation: OperationId,
    },
    EntryFunctionMissing(MachineId),
    ProviderInstallationIdentityMismatch,
    DuplicateSelectedBoundaryCall {
        machine: MachineId,
        operation: semantic_vocabulary::InvocationSite,
        boundary: BoundaryMachineId,
    },
    UnknownSelectedBoundaryCall {
        machine: MachineId,
        operation: semantic_vocabulary::InvocationSite,
        boundary: BoundaryMachineId,
    },
    SelectedBoundaryCallEvidenceMismatch {
        machine: MachineId,
        operation: semantic_vocabulary::InvocationSite,
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
        invocation_site: semantic_vocabulary::InvocationSite,
        boundary: BoundaryMachineId,
    },
    DuplicateBoundarySettlement(BoundaryMachineId),
    UnknownBoundarySettlement(BoundaryMachineId),
    MissingBoundarySettlement(BoundaryMachineId),
    UnusedBoundarySettlement(BoundaryMachineId),
    BoundaryRealizationMismatch(BoundaryMachineId),
    BoundaryRealizationDetail {
        boundary: BoundaryMachineId,
        reason: String,
    },
    InvalidClaimCompletionOnlyShape {
        machine: MachineId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    ProviderExecutionBinding(String),
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
    StoredReferenceCall {
        caller: MachineId,
        callee: Option<MachineId>,
        operation: OperationId,
        reason: &'static str,
    },
    DuplicateNativeCallbackArgument(OperationId),
    MultipleNativeCallbackArguments,
    UnknownNativeCallbackArgument(OperationId),
    InvalidNativeCallbackArgument(OperationId),
    InvalidInstalledInterrupt {
        machine: MachineId,
        operation: OperationId,
        reason: &'static str,
    },
    MissingNativeCallbackArgument(OperationId),
    IeeeFloatFmaOperandMismatch(ValueId),
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
    StructuralStateBindingMismatch {
        machine: semantic_vocabulary::MachineId,
        reason: String,
    },
    DescriptorStateBindingMismatch {
        machine: MachineId,
        source: PlaceId,
        parameter: PlaceId,
        supplied_type: StructuralTypeId,
        required_type: StructuralTypeId,
        supplied_access: terminal_psi::StructuralAccess,
        required_access: terminal_psi::StructuralAccess,
        reason: &'static str,
    },
    NonDenseStructuralTypes(semantic_vocabulary::NonDenseIdentities),
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    EmptyStructuralType(StructuralTypeId),
    RelevantOpaqueStructuralField(StructuralTypeId),
    UnsupportedStructuralByteSequence(StructuralTypeId),
    UnsupportedStructuralSum(StructuralTypeId),
    UnsupportedStructuralArray(StructuralTypeId),
    InvalidOwnedStructuralArgument {
        source: PlaceId,
        parameter: PlaceId,
        structural_type: StructuralTypeId,
        reason: &'static str,
    },
    UnsupportedStructuralReference(StructuralTypeId),
    UnclosedTargetLayout(StructuralTypeId),
    UnclosedArrayExtent(StructuralTypeId),
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
pub enum AccessCallCustodyError {
    FormalDemandMismatch,
    CapturedInterpretationMissing,
    CapturedInterpretationMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessViewInputTranslationError {
    UnsupportedInputCount(usize),
    SelectionCountMismatch { expected: usize, actual: usize },
    InputIsNotDirectEntry,
    UnsupportedEntryFunctionShape(MachineId),
    SelectionRowMismatch,
    AccessPlanIdentityMismatch,
    AccessPlanHasNoConcreteSize,
    TargetPointerShapeUnsupported,
    AbiPlan(PlanDiagnostic),
    CandidatePlanMismatch,
    CandidateEntryCallPlanMismatch,
    CandidateInputRosterMismatch,
}

impl From<AccessViewInputTranslationError> for LoweringError {
    fn from(error: AccessViewInputTranslationError) -> Self {
        Self::AccessViewInput(error)
    }
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BoundaryRealizationDetail { reason, .. } => formatter.write_str(reason),
            other => write!(formatter, "{other:?}"),
        }
    }
}

impl std::error::Error for LoweringError {}

impl LoweringError {
    pub(crate) fn boundary_realization(
        boundary: BoundaryMachineId,
        reason: impl Into<String>,
    ) -> Self {
        Self::BoundaryRealizationDetail {
            boundary,
            reason: reason.into(),
        }
    }

    pub(crate) fn with_boundary_context(
        self,
        plan: &abstract_operations::AbstractOperationPlan,
    ) -> Self {
        let (boundary, reason) = match self {
            Self::BoundaryRealizationDetail { boundary, reason } => (boundary, reason),
            other => return other,
        };
        let name = plan
            .boundary_machine(boundary)
            .and_then(|declaration| plan.identities.text(declaration.identity))
            .map(language_semantics::type_identity::callable_display_path)
            .unwrap_or_else(|| format!("boundary {}", boundary.get()).into());
        Self::boundary_realization(boundary, format!("boundary `{name}`: {reason}"))
    }

    #[track_caller]
    pub(crate) fn unsupported_control_flow(machine: MachineId) -> Self {
        Self::UnsupportedControlFlow {
            machine,
            site: optimization_core::CompilerCheckSite::caller(),
        }
    }
}
