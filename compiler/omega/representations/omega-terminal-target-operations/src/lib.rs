#![forbid(unsafe_code)]

//! Target-selected operations derived from source-independent terminal Omega
//! requirements.

use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use omega_target::NativeTarget;
pub use omega_terminal_abstract_operations::TerminalCompletionClaimSource;
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashPredicateTerm, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
    TerminalPsiIdentity,
};

pub use omega_calling_conventions::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetOperationPlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TerminalTargetFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetFunction {
    pub machine: MachineId,
    /// Exact terminal attachment retained for artifact-side nominal cleanup validation.
    pub attachment: Option<StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub operation: TerminalTargetOperation,
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
pub enum TerminalCallSiteOwner {
    Operation(OperationId),
    CleanupAction { edge: EdgeId, action_ordinal: u32 },
}

impl TerminalCallSiteOwner {
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

/// Installation-selected provider-plan identity for one bodyless boundary.
/// This is realization metadata, not executable authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProviderPlanIdentity(u64);

impl TerminalProviderPlanIdentity {
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Exact admitted provider execution selected for this terminal realization.
/// The execution fingerprint covers the normalized root, selected plan,
/// entry/boundary contract, resource realizations, and exit assurance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProviderExecutionBinding {
    provider_plan: TerminalProviderPlanIdentity,
    provider_execution_identity: u64,
    provider_execution_fingerprint: u64,
    normalized_root_identity: u64,
    boundary_contract_fingerprint: u64,
}

impl TerminalProviderExecutionBinding {
    /// Non-authoritative data projection. Production lowering obtains these
    /// fields from `omega_external_roots::ProviderExecution`; constructing a
    /// record does not grant root admission or executable authority.
    pub fn from_execution_record(
        provider_plan: TerminalProviderPlanIdentity,
        provider_execution_identity: u64,
        provider_execution_fingerprint: u64,
        normalized_root_identity: u64,
        boundary_contract_fingerprint: u64,
    ) -> Option<Self> {
        [
            provider_execution_identity,
            provider_execution_fingerprint,
            normalized_root_identity,
            boundary_contract_fingerprint,
        ]
        .iter()
        .all(|identity| *identity != 0)
        .then_some(Self {
            provider_plan,
            provider_execution_identity,
            provider_execution_fingerprint,
            normalized_root_identity,
            boundary_contract_fingerprint,
        })
    }

    pub const fn provider_plan(self) -> TerminalProviderPlanIdentity {
        self.provider_plan
    }

    pub const fn provider_execution_identity(self) -> u64 {
        self.provider_execution_identity
    }

    pub const fn provider_execution_fingerprint(self) -> u64 {
        self.provider_execution_fingerprint
    }

    pub const fn normalized_root_identity(self) -> u64 {
        self.normalized_root_identity
    }

    pub const fn boundary_contract_fingerprint(self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalMetadataOnlyPortRealization {
    pub effect_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalDirectPortReadU8Realization {
    pub service: ServiceId,
    pub port: u16,
}

/// Import-free Linux process termination through the kernel's `exit_group`
/// syscall. The syscall number and register assignment are target facts, not
/// producer-selected metadata, so this realization carries no configurable
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalLinuxExitGroupI32Realization;

/// Import-free Linux standard-output realization through the kernel's
/// `write(2)` ABI. The emitted loop consumes the complete immutable payload
/// and one trailing newline or traps; no hosted import is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalLinuxWriteLineRealization;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBoundaryRealization {
    MetadataOnlyPort(TerminalMetadataOnlyPortRealization),
    DirectPortReadU8(TerminalDirectPortReadU8Realization),
    LinuxWriteLine(TerminalLinuxWriteLineRealization),
    LinuxExitGroupI32(TerminalLinuxExitGroupI32Realization),
}

impl From<TerminalMetadataOnlyPortRealization> for TerminalBoundaryRealization {
    fn from(realization: TerminalMetadataOnlyPortRealization) -> Self {
        Self::MetadataOnlyPort(realization)
    }
}

impl From<TerminalLinuxExitGroupI32Realization> for TerminalBoundaryRealization {
    fn from(realization: TerminalLinuxExitGroupI32Realization) -> Self {
        Self::LinuxExitGroupI32(realization)
    }
}

impl From<TerminalLinuxWriteLineRealization> for TerminalBoundaryRealization {
    fn from(realization: TerminalLinuxWriteLineRealization) -> Self {
        Self::LinuxWriteLine(realization)
    }
}

/// Exact scalar value consumed by a native boundary realization.
///
/// The value identity and type bind this row back to terminal Psi; the
/// immediate and destination register make the emitted provider interval
/// independently replayable. This is deliberately separate from structural
/// settlement custody and from a machine result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBoundaryScalarArgument {
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub immediate: IntegerValue,
    pub destination: MachineRegister,
}

/// Exact structural source consumed by one native byte-sequence boundary.
/// The literal operation and declaration bind the payload back to terminal
/// Psi independently of target byte placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryByteSequenceArgument {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
}

/// The first boundary realization is metadata-only: an exact selected
/// provider execution settles the claim, while the preceding semantic effect
/// (for example `PortWrite`) performs the hardware operation. No code is
/// silently erased; this row must survive installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBoundarySettlementBinding {
    pub boundary: BoundaryMachineId,
    pub provider_execution: TerminalProviderExecutionBinding,
    pub realization: TerminalBoundaryRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetUnitBody {
    /// Canonical verifier-owned structural declaration closure used to replay
    /// projected-layout and partial-cleanup partitions at artifact boundaries.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    pub parameters: Vec<TerminalTargetStructuralParameter>,
    pub operations: Vec<TerminalTargetUnitOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetStructuralParameter {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetStructuralArgument {
    pub place: PlaceId,
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
pub enum TerminalTargetUnitOperation {
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
        arguments: Vec<TerminalTargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
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
        provider_execution: TerminalProviderExecutionBinding,
        realization: TerminalBoundaryRealization,
        scalar_arguments: Vec<TerminalBoundaryScalarArgument>,
        arguments: Vec<StructuralArgument>,
        byte_sequence_arguments: Vec<TerminalBoundaryByteSequenceArgument>,
        completion_claim_sources: Vec<TerminalCompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetOperation {
    /// Ordered straight-line Unit/effect body. Scalar expression trees remain
    /// a separate representation so value-less execution cannot fabricate a
    /// pseudo-result.
    UnitBody(TerminalTargetUnitBody),
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
        structural_parameters: Vec<TerminalTargetStructuralParameter>,
        arguments: Vec<TerminalTargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
    },
    /// A scalar return plus the exact structural cleanup frontier that runs
    /// after result materialization and before native return teardown.
    ScalarReturnWithCleanup {
        scalar: Box<TerminalTargetOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TerminalTargetStructuralParameter>,
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
        provider_execution: TerminalProviderExecutionBinding,
        realization: TerminalDirectPortReadU8Realization,
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<TerminalCompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
        call_plan: CallPlan,
        structural_parameters: Vec<TerminalTargetStructuralParameter>,
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
        provider_execution: TerminalProviderExecutionBinding,
        realization: TerminalLinuxExitGroupI32Realization,
        argument: TerminalBoundaryScalarArgument,
        completion_claim_sources: Vec<TerminalCompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// One finite short-circuit Boolean tree whose value-return leaves all
    /// execute the same complete structural cleanup stream. Each leaf retains
    /// its own terminal-Psi return edge in `control`; there is deliberately no
    /// synthetic shared cleanup edge.
    BooleanControlWithCleanup {
        control: TerminalTargetBooleanControl,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TerminalTargetStructuralParameter>,
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
        location: TerminalScalarParameterLocation,
    },
    /// Return one caller-supplied Boolean from its selected native ABI
    /// location.
    ReturnBooleanParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
    },
    /// Return the logical negation of one caller-supplied canonical Boolean.
    ReturnBooleanNotParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
    },
    /// One verified terminal-Psi Boolean convergence tree whose value leaves
    /// join one physical return/cleanup tail.
    ReturnBooleanSharedConvergence {
        psi_edge: EdgeId,
        control: TerminalTargetBooleanControl,
    },
    /// Return a runtime Boolean expression lowered from terminal-Psi logical
    /// operations. Every node produces a canonical zero/one Boolean.
    ReturnBooleanExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        expression: TerminalTargetBooleanExpression,
    },
    /// Return a runtime integer expression lowered from exact-width terminal
    /// Psi operations. Every node has the enclosing result's integer type.
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        expression: TerminalTargetIntegerExpression,
    },
    /// Execute an acyclic conditional-control tree whose leaves return integer
    /// expressions. Every structural and return edge remains explicit.
    ReturnIntegerConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalScalarParameterLocation,
        scalar_type: IntegerType,
        when_true: TerminalTargetConditionalIntegerArm,
        when_false: TerminalTargetConditionalIntegerArm,
    },
    /// Execute integer-returning control whose root condition is a recursive
    /// runtime Boolean expression rather than one direct ABI parameter.
    ReturnIntegerExpressionConditionalControl {
        condition_source: ValueId,
        condition: TerminalTargetBooleanExpression,
        scalar_type: IntegerType,
        when_true: TerminalTargetConditionalIntegerArm,
        when_false: TerminalTargetConditionalIntegerArm,
    },
    /// Execute an acyclic conditional-control tree whose leaves return
    /// canonical Boolean values.
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalScalarParameterLocation,
        when_true: TerminalTargetConditionalBooleanArm,
        when_false: TerminalTargetConditionalBooleanArm,
    },
    /// Execute Boolean control whose root condition is a recursive runtime
    /// Boolean expression rather than one direct ABI parameter.
    ReturnBooleanExpressionConditionalControl {
        condition_source: ValueId,
        condition: TerminalTargetBooleanExpression,
        when_true: TerminalTargetConditionalBooleanArm,
        when_false: TerminalTargetConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetBooleanExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TerminalTargetCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
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
        operand: Box<TerminalTargetBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<TerminalTargetBooleanExpression>,
        right: Box<TerminalTargetBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<TerminalTargetBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetBooleanControl {
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
        location: TerminalScalarParameterLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TerminalTargetBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalScalarParameterLocation,
        when_true: TerminalTargetConditionalBooleanArm,
        when_false: TerminalTargetConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TerminalTargetBooleanExpression,
        when_true: TerminalTargetConditionalBooleanArm,
        when_false: TerminalTargetConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<TerminalTargetIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        expression: TerminalTargetIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalScalarParameterLocation,
        when_true: TerminalTargetConditionalIntegerArm,
        when_false: TerminalTargetConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition: TerminalTargetBooleanExpression,
        when_true: TerminalTargetConditionalIntegerArm,
        when_false: TerminalTargetConditionalIntegerArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetIntegerExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: MachineId,
        arguments: Vec<TerminalTargetCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<TerminalTargetIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TerminalTargetIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TerminalTargetIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    WrappingShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TerminalTargetIntegerExpression>,
        count: Box<TerminalTargetIntegerExpression>,
    },
    WrappingShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TerminalTargetIntegerExpression>,
        count: Box<TerminalTargetIntegerExpression>,
    },
    ExactShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TerminalTargetIntegerExpression>,
        count: Box<TerminalTargetIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TerminalTargetIntegerExpression>,
        count: Box<TerminalTargetIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    SaturatingAdd {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    WrappingSubtract {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    SaturatingSubtract {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    WrappingMultiply {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<TerminalTargetIntegerExpression>,
        right: Box<TerminalTargetIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetScalarExpression {
    Boolean(TerminalTargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: TerminalTargetIntegerExpression,
    },
}

impl TerminalTargetScalarExpression {
    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetCallArgument {
    pub scalar_type: ScalarType,
    pub location: TerminalScalarParameterLocation,
    pub expression: TerminalTargetScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarParameterLocation {
    Register(MachineRegister),
    /// Byte offset in the ABI's incoming stack-argument area, excluding an
    /// architecture-specific return-address bias.
    IncomingStack {
        byte_offset: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::TerminalCallSiteOwner;
    use psi_core::{EdgeId, OperationId};

    #[test]
    fn call_site_owner_preserves_disjoint_operation_and_cleanup_action_identities() {
        let operation = OperationId::new(7).expect("nonzero operation");
        let edge = EdgeId::new(7).expect("nonzero edge");

        let operation_owner = TerminalCallSiteOwner::Operation(operation);
        assert_eq!(operation_owner.operation(), Some(operation));
        assert_eq!(operation_owner.edge(), None);

        let edge_owner = TerminalCallSiteOwner::CleanupAction {
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
