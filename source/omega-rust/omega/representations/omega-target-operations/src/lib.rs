#![forbid(unsafe_code)]

//! Target-selected operations derived from source-independent terminal Omega
//! requirements.

pub use omega_abstract_operations::{CompletionClaimSource, RankedU32CountdownCustody};
use omega_calling_conventions::{BoundaryEntryPlan, CallPlan, ValuePlacement, ValueShape};
use omega_target::NativeTarget;
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashPredicateTerm, ProviderCandidateConformance,
    StructuralArgument, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
    TerminalPsiIdentity,
};

pub use omega_calling_conventions::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TargetFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFunction {
    pub machine: MachineId,
    /// Exact terminal attachment retained for artifact-side nominal cleanup validation.
    pub attachment: Option<StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub operation: TargetOperation,
}

/// Ordered terminal-Psi sources refined into one target function.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalPsiProvenance {
    pub operations: Vec<OperationId>,
    pub edges: Vec<EdgeId>,
}

/// Semantic owner of one in-module native call site.
///
/// Ordinary calls are owned by terminal-Psi operations. An executable nominal
/// cleanup is different: invoking the attached cleanup machine is work of one
/// exact ordered action on the selected ownership edge, so it must retain both
/// identities rather than fabricating an [`OperationId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallSiteOwner {
    Operation(OperationId),
    CleanupAction { edge: EdgeId, action_ordinal: u32 },
}

impl CallSiteOwner {
    pub const fn operation(self) -> Option<OperationId> {
        match self {
            Self::Operation(operation) => Some(operation),
            Self::CleanupAction { .. } => None,
        }
    }

    pub const fn edge(self) -> Option<EdgeId> {
        match self {
            Self::Operation(_) => None,
            Self::CleanupAction { edge, .. } => Some(edge),
        }
    }

    pub const fn cleanup_action_ordinal(self) -> Option<u32> {
        match self {
            Self::Operation(_) => None,
            Self::CleanupAction { action_ordinal, .. } => Some(action_ordinal),
        }
    }
}

/// Non-authoritative report identity for the installation-selected provider
/// plan of one bodyless boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderPlanReportIdentity(u64);

impl ProviderPlanReportIdentity {
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Non-authoritative target-operation report projection of the exact admitted
/// provider execution selected for this terminal realization.
///
/// The ledger-owned `ProviderExecutionEvidence` borrowed by lowering is the
/// authority carrier. These compact coordinates support deterministic reports
/// and serialization only and cannot recreate admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderExecutionBinding {
    provider_plan_report_identity: ProviderPlanReportIdentity,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
}

impl ProviderExecutionBinding {
    /// Non-authoritative data projection. Production lowering obtains these
    /// fields from `omega_external_roots::ProviderExecution`; constructing a
    /// record does not grant root admission or executable authority.
    pub fn from_execution_record(
        provider_plan_report_identity: ProviderPlanReportIdentity,
        provider_execution_report_identity: u64,
        provider_execution_report_fingerprint: u64,
        normalized_root_report_identity: u64,
        boundary_contract_report_fingerprint: u64,
    ) -> Option<Self> {
        [
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        ]
        .iter()
        .all(|identity| *identity != 0)
        .then_some(Self {
            provider_plan_report_identity,
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        })
    }

    pub const fn provider_plan_report_identity(self) -> ProviderPlanReportIdentity {
        self.provider_plan_report_identity
    }

    pub const fn provider_execution_report_identity(self) -> u64 {
        self.provider_execution_report_identity
    }

    pub const fn provider_execution_report_fingerprint(self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    pub const fn normalized_root_report_identity(self) -> u64 {
        self.normalized_root_report_identity
    }

    pub const fn boundary_contract_report_fingerprint(self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataOnlyPortRealization {
    pub effect_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectPortReadU8Realization {
    pub service: ServiceId,
    pub port: u16,
}

/// Import-free Linux process termination through the kernel's `exit_group`
/// syscall. The syscall number and register assignment are target facts, not
/// producer-selected metadata, so this realization carries no configurable
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxExitGroupI32Realization;

/// Import-free Linux standard-output realization through the kernel's
/// `write(2)` ABI. The emitted loop consumes the complete immutable payload
/// and one trailing newline or traps; no hosted import is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxWriteLineRealization;

/// A provider execution whose complete native effect is the successful
/// completion of the boundary call's retained ownership claims.
///
/// This realization has no scalar input, result, byte-sequence payload, or
/// target instruction. The boundary occurrence, admitted provider execution,
/// structural arguments, and completion receipts remain explicit in the
/// surrounding [`TargetUnitOperation::BoundarySettlement`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClaimCompletionOnlyRealization;

/// Exact source-free custody for one evaluated normalized import leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignCallBinding {
    pub locator: omega_target::NormalizedForeignLocator,
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub same_stack_contribution: omega_task_plans::AdmittedSameStackContribution,
}

/// Closed native settlement choice. Keeping evaluated imports disjoint from
/// built-in realizations prevents locator custody from being stripped into a
/// no-code boundary settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundarySettlementRealization {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(NormalizedForeignCallBinding),
}

impl From<BoundaryRealization> for BoundarySettlementRealization {
    fn from(realization: BoundaryRealization) -> Self {
        Self::Builtin(realization)
    }
}

macro_rules! builtin_settlement_conversion {
    ($realization:ty) => {
        impl From<$realization> for BoundarySettlementRealization {
            fn from(realization: $realization) -> Self {
                Self::Builtin(realization.into())
            }
        }
    };
}

builtin_settlement_conversion!(MetadataOnlyPortRealization);
builtin_settlement_conversion!(DirectPortReadU8Realization);
builtin_settlement_conversion!(LinuxWriteLineRealization);
builtin_settlement_conversion!(LinuxExitGroupI32Realization);
builtin_settlement_conversion!(ClaimCompletionOnlyRealization);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryRealization {
    MetadataOnlyPort(MetadataOnlyPortRealization),
    DirectPortReadU8(DirectPortReadU8Realization),
    LinuxWriteLine(LinuxWriteLineRealization),
    LinuxExitGroupI32(LinuxExitGroupI32Realization),
    ClaimCompletionOnly(ClaimCompletionOnlyRealization),
}

impl From<MetadataOnlyPortRealization> for BoundaryRealization {
    fn from(realization: MetadataOnlyPortRealization) -> Self {
        Self::MetadataOnlyPort(realization)
    }
}

impl From<DirectPortReadU8Realization> for BoundaryRealization {
    fn from(realization: DirectPortReadU8Realization) -> Self {
        Self::DirectPortReadU8(realization)
    }
}

impl From<LinuxExitGroupI32Realization> for BoundaryRealization {
    fn from(realization: LinuxExitGroupI32Realization) -> Self {
        Self::LinuxExitGroupI32(realization)
    }
}

impl From<LinuxWriteLineRealization> for BoundaryRealization {
    fn from(realization: LinuxWriteLineRealization) -> Self {
        Self::LinuxWriteLine(realization)
    }
}

impl From<ClaimCompletionOnlyRealization> for BoundaryRealization {
    fn from(realization: ClaimCompletionOnlyRealization) -> Self {
        Self::ClaimCompletionOnly(realization)
    }
}

/// Exact scalar value consumed by a native boundary realization.
///
/// The value identity and type bind this row back to terminal Psi; the
/// immediate and destination register make the emitted provider interval
/// independently replayable. This is deliberately separate from structural
/// settlement custody and from a machine result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryScalarArgument {
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub immediate: IntegerValue,
    pub destination: MachineRegister,
}

/// Exact structural source consumed by one native byte-sequence boundary.
/// The literal operation and declaration bind the payload back to terminal
/// Psi independently of target byte placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryByteSequenceArgument {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
}

/// The first boundary realization is metadata-only: an exact selected
/// provider execution settles the claim, while the preceding semantic effect
/// (for example `PortWrite`) performs the hardware operation. No code is
/// silently erased; this row must survive installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySettlementBinding {
    pub boundary: BoundaryMachineId,
    pub provider_execution: ProviderExecutionBinding,
    pub realization: BoundarySettlementRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitBody {
    /// Canonical verifier-owned structural declaration closure used to replay
    /// projected-layout and partial-cleanup partitions at artifact boundaries.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    pub parameters: Vec<TargetStructuralParameter>,
    pub operations: Vec<TargetUnitOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralParameter {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub access: psi_terminal::StructuralAccess,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralArgument {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
    /// Exact source-relative semantic projection retained through native and
    /// installed-artifact custody.
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    /// Checked byte offset of this projected value within `source`.
    pub source_byte_offset: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUnitOperation {
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
    },
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    Call {
        psi_operation: OperationId,
        callee: MachineId,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
    },
    /// One bodyless boundary occurrence projected through an opaque admitted
    /// installation into an exact checked Unit provider call. The original
    /// receipt evidence remains alongside its call-transfer interpretation so
    /// later legalization can replay the join without treating it as a
    /// source-authored `CallUnit`.
    InstalledProviderCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        source_arguments: Vec<StructuralArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// One zero-argument Unit-returning evaluated import leaf. Native
    /// settlement rejoins this exact carrier; lowering never accepts locator
    /// or calling-plan strings from the call site.
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: NormalizedForeignCallBinding,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    BoundarySettlement {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        realization: BoundaryRealization,
        scalar_arguments: Vec<BoundaryScalarArgument>,
        arguments: Vec<StructuralArgument>,
        byte_sequence_arguments: Vec<BoundaryByteSequenceArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOperation {
    /// Exact admitted structural-Unit `u32` countdown. This carrier preserves
    /// the verifier/fuel custody and cyclic graph identity directly; it is not
    /// an acyclic conditional-control tree.
    RankedU32Countdown(TargetRankedU32Countdown),
    /// Ordered straight-line Unit/effect body. Scalar expression trees remain
    /// a separate representation so value-less execution cannot fabricate a
    /// pseudo-result.
    UnitBody(TargetUnitBody),
    /// One bounded whole-root structural call whose scalar ABI result is
    /// returned directly. Keeping this carrier distinct from `UnitBody`
    /// prevents a result-bearing call from being erased into a value-less
    /// operation stream while still sharing the aggregate-copy ABI lane.
    ReturnStructuralScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
    },
    /// One exact whole-root structural call whose direct ABI result is returned
    /// unchanged by the caller.
    ReturnStructuralCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        operation_result: StructuralOperationResult,
        result: StructuralResultDeclaration,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        callee_call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        returned_claims: Vec<ClaimId>,
    },
    /// A scalar return plus the exact structural cleanup frontier that runs
    /// after result materialization and before native return teardown.
    ScalarReturnWithCleanup {
        scalar: Box<TargetOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
        psi_edge: EdgeId,
    },
    /// One direct x86 provider realization for a verified bodyless boundary
    /// returning an unsigned byte. Structural arguments and receipts are
    /// semantic custody metadata; the selected port read produces the ABI
    /// scalar result.
    ReturnBoundaryPortReadU8 {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        realization: DirectPortReadU8Realization,
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
    },
    /// One verified `exit_process(i32)` call realized directly by Linux
    /// `exit_group`. `nominal_return_edge` remains zero-byte provenance: if
    /// the nominally nonreturning syscall returns, the emitted code traps
    /// before that semantic tail can execute.
    ExitProcessI32 {
        constant_operation: OperationId,
        psi_operation: OperationId,
        nominal_return_edge: EdgeId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        realization: LinuxExitGroupI32Realization,
        argument: BoundaryScalarArgument,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// One finite short-circuit Boolean tree whose value-return leaves all
    /// execute the same complete structural cleanup stream. Each leaf retains
    /// its own terminal-Psi return edge in `control`; there is deliberately no
    /// synthetic shared cleanup edge.
    BooleanControlWithCleanup {
        control: TargetBooleanControl,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Return one exact whole-root structural parameter through the selected
    /// native ABI while retaining its zero-runtime custody metadata.
    ReturnStructuralParameter {
        call_plan: CallPlan,
        /// Complete ordered structural input signature used to derive the ABI
        /// plan. Cleanup-only parameters remain semantic inputs even though
        /// disposing them emits no instruction.
        parameters: Vec<StructuralParameterDeclaration>,
        source: StructuralParameterDeclaration,
        result: StructuralResultDeclaration,
        shape: ValueShape,
        source_placement: ValuePlacement,
        result_placement: ValuePlacement,
        psi_edge: EdgeId,
        returned_claims: Vec<ClaimId>,
        /// Typed no-ABI local declarations retained as zero-code metadata.
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        /// Exact verifier-owned reverse-declaration cleanup order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// End the execution domain at one verified terminal-Psi crash edge.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    /// Return one compile-time integer through the target's ordinary scalar
    /// function-result convention. Register and instruction encoding are
    /// chosen by machine emission.
    ReturnIntegerImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    /// Return a compile-time Boolean as the target ABI's canonical zero/one
    /// scalar result.
    ReturnBooleanImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    /// Return one caller-supplied integer from its selected native ABI
    /// location. The source value remains the terminal-Psi parameter identity.
    ReturnIntegerParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// Return one caller-supplied Boolean from its selected native ABI
    /// location.
    ReturnBooleanParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// Return the logical negation of one caller-supplied canonical Boolean.
    ReturnBooleanNotParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// One verified terminal-Psi Boolean convergence tree whose value leaves
    /// join one physical return/cleanup tail.
    ReturnBooleanSharedConvergence {
        psi_edge: EdgeId,
        control: TargetBooleanControl,
    },
    /// Return a runtime Boolean expression lowered from terminal-Psi logical
    /// operations. Every node produces a canonical zero/one Boolean.
    ReturnBooleanExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        expression: TargetBooleanExpression,
    },
    /// Return a runtime integer expression lowered from exact-width terminal
    /// Psi operations. Every node has the enclosing result's integer type.
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
    /// Execute an acyclic conditional-control tree whose leaves return integer
    /// expressions. Every structural and return edge remains explicit.
    ReturnIntegerConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        scalar_type: IntegerType,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    /// Execute integer-returning control whose root condition is a recursive
    /// runtime Boolean expression rather than one direct ABI parameter.
    ReturnIntegerExpressionConditionalControl {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        scalar_type: IntegerType,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    /// Execute an acyclic conditional-control tree whose leaves return
    /// canonical Boolean values.
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
    /// Execute Boolean control whose root condition is a recursive runtime
    /// Boolean expression rather than one direct ABI parameter.
    ReturnBooleanExpressionConditionalControl {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRankedU32Countdown {
    pub custody: RankedU32CountdownCustody,
    pub call_plan: CallPlan,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TargetCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    StructuralField {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        field: StructuralFieldId,
        source_placement: ValuePlacement,
        field_byte_offset: u32,
    },
    Not {
        psi_operation: OperationId,
        operand: Box<TargetBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<TargetBooleanExpression>,
        right: Box<TargetBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<TargetBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    ReturnImmediate {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    ReturnParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TargetBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<TargetIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TargetIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TargetCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    WrappingShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    ExactShiftLeft {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactAdd {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingAdd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingSubtract {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactSubtract {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingSubtract {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingMultiply {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactMultiply {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetScalarExpression {
    Boolean(TargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
}

impl TargetScalarExpression {
    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCallArgument {
    pub scalar_type: ScalarType,
    pub location: ScalarParameterLocation,
    pub expression: TargetScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarParameterLocation {
    Register(MachineRegister),
    /// Byte offset in the ABI's incoming stack-argument area, excluding an
    /// architecture-specific return-address bias.
    IncomingStack {
        byte_offset: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::CallSiteOwner;
    use psi_core::{EdgeId, OperationId};

    #[test]
    fn call_site_owner_preserves_disjoint_operation_and_cleanup_action_identities() {
        let operation = OperationId::new(7).expect("nonzero operation");
        let edge = EdgeId::new(7).expect("nonzero edge");

        let operation_owner = CallSiteOwner::Operation(operation);
        assert_eq!(operation_owner.operation(), Some(operation));
        assert_eq!(operation_owner.edge(), None);

        let edge_owner = CallSiteOwner::CleanupAction {
            edge,
            action_ordinal: 3,
        };
        assert_eq!(edge_owner.operation(), None);
        assert_eq!(edge_owner.edge(), Some(edge));
        assert_eq!(edge_owner.cleanup_action_ordinal(), Some(3));
        assert_eq!(operation_owner.cleanup_action_ordinal(), None);
        assert_ne!(operation_owner, edge_owner);
    }
}
