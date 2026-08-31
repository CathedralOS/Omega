#![forbid(unsafe_code)]

//! Concrete register and stack homes assigned to the clean terminal-Psi target
//! operation lane.

use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use omega_target::NativeTarget;
use omega_target_operations::{
    BoundaryByteSequenceArgument, BoundaryRealization, BoundaryScalarArgument,
    CompletionClaimSource, DirectPortReadU8Realization, LinuxExitGroupI32Realization,
    MachineRegister, NormalizedForeignScalarArgument, ProviderExecutionBinding,
    RankedU32CountdownCustody, TargetStructuralParameter, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashPredicateTerm, StructuralArgument,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultClaimTransfer, StructuralResultDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<AssignedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub fixed_integer_scalar_abi: Option<omega_target_operations::FixedIntegerScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub operation: AssignedOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedOperation {
    RankedU32Countdown(AssignedRankedU32Countdown),
    UnitBody(AssignedUnitBody),
    ReturnStructuralScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
    },
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
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        returned_claims: Vec<ClaimId>,
    },
    ScalarReturnWithCleanup {
        scalar: Box<AssignedOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<omega_target_operations::TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
        psi_edge: EdgeId,
    },
    ReturnBoundaryPortReadU8 {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        boundary: psi_core::BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        realization: DirectPortReadU8Realization,
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
        call_plan: omega_calling_conventions::CallPlan,
        structural_parameters: Vec<omega_target_operations::TargetStructuralParameter>,
    },
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
    /// Assigned form of one finite short-circuit Boolean tree. Exact cleanup
    /// ownership remains attached to each leaf's terminal-Psi return edge.
    BooleanControlWithCleanup {
        control: AssignedBooleanControl,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<omega_target_operations::TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    ReturnStructuralParameter {
        call_plan: CallPlan,
        parameters: Vec<StructuralParameterDeclaration>,
        source: StructuralParameterDeclaration,
        result: StructuralResultDeclaration,
        shape: ValueShape,
        source_placement: ValuePlacement,
        result_placement: ValuePlacement,
        psi_edge: EdgeId,
        returned_claims: Vec<ClaimId>,
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        trivial_affine_discards: Vec<PlaceId>,
    },
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    ReturnIntegerImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    ReturnBooleanImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    ReturnIntegerParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnBooleanParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnBooleanNotParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnBooleanSharedConvergence {
        psi_edge: EdgeId,
        control: AssignedBooleanControl,
    },
    ReturnBooleanExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedBooleanExpression,
    },
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        frame: ExpressionFrame,
        expression: AssignedIntegerExpression,
    },
    ReturnIntegerConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        scalar_type: IntegerType,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
    ReturnIntegerExpressionConditionalControl {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        scalar_type: IntegerType,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
    ReturnBooleanExpressionConditionalControl {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedRankedU32Countdown {
    pub custody: RankedU32CountdownCustody,
    pub call_plan: CallPlan,
    /// Stable mutable home of the loop-carried rank. The first exact slice
    /// requires this to be the canonical incoming target-native register.
    pub rank_home: MachineRegister,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedUnitBody {
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    pub parameters: Vec<TargetStructuralParameter>,
    pub operations: Vec<AssignedUnitOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedAggregateCopy {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
}

/// Durable physical home assigned to one fixed-width integer value produced
/// by a scalar call in an attached Unit body.
///
/// `byte_offset` is relative to the function's allocated Unit frame. Machine
/// emission independently reconstructs the complete structural-plus-scalar
/// frame and rejects a stale, overlapping, or substituted home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignedUnitScalarHome {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: IntegerType,
    pub shape: ValueShape,
    pub byte_offset: u32,
}

/// Exact physical source of one attached-Unit scalar-call argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedUnitScalarArgumentSource {
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    Home(AssignedUnitScalarHome),
}

impl AssignedUnitScalarArgumentSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> IntegerType {
        match self {
            Self::IntegerImmediate { scalar_type, .. } => scalar_type,
            Self::Home(home) => home.scalar_type,
        }
    }
}

/// One positional scalar argument after durable-home assignment. The complete
/// ABI placement remains explicit; it is not reconstructed from register
/// ordinals during emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedUnitScalarCallArgument {
    pub parameter_index: u32,
    pub source: AssignedUnitScalarArgumentSource,
    pub destination: AssignedCallDestination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedUnitOperation {
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
        result: Option<ScalarType>,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
    },
    /// One real in-module fixed-width integer call in an attached Unit body.
    /// The result home survives subsequent call-register clobbers and is the
    /// only accepted source for a later scalar-call argument.
    ScalarCall {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        result_home: AssignedUnitScalarHome,
        arguments: Vec<AssignedUnitScalarCallArgument>,
    },
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: omega_target_operations::NormalizedForeignCallBinding,
        scalar_arguments: Vec<NormalizedForeignScalarArgument>,
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
pub enum AssignedBooleanExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: psi_core::MachineId,
        arguments: Vec<AssignedCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
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
        operand: Box<AssignedBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<AssignedBooleanExpression>,
        right: Box<AssignedBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<AssignedBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedBooleanControl {
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
        location: AssignedScalarLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<AssignedIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionFrame {
    /// Aligned bytes reserved before evaluating the expression.
    pub byte_size: u32,
    /// Incoming ABI registers copied into stable frame homes before any
    /// expression scratch register can overwrite them.
    pub register_spills: Vec<EntryRegisterSpill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRegisterSpill {
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedIntegerExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: psi_core::MachineId,
        arguments: Vec<AssignedCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<AssignedIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<AssignedIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        source_type: IntegerType,
        operand: Box<AssignedIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    WrappingShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    ExactShiftLeft {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<AssignedIntegerExpression>,
        count: Box<AssignedIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactAdd {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingAdd {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingSubtract {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactSubtract {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingSubtract {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingMultiply {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactMultiply {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<AssignedIntegerExpression>,
        right: Box<AssignedIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedCallArgument {
    pub scalar_type: psi_core::ScalarType,
    /// Concrete ABI home populated after all sibling arguments have been
    /// evaluated. Outgoing stack offsets are relative to the call plan's ABI
    /// argument area and therefore already include any shadow/home space.
    pub destination: AssignedCallDestination,
    /// Stable frame slot holding the fully evaluated argument until every
    /// sibling argument is ready for simultaneous ABI placement.
    pub spill_byte_offset: u32,
    pub expression: AssignedScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedCallDestination {
    Register(MachineRegister),
    OutgoingStack { byte_offset: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedScalarExpression {
    Boolean(AssignedBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: AssignedIntegerExpression,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedScalarLocation {
    Register(MachineRegister),
    /// Stable storage reserved by the assignment stage in the current frame.
    FrameSpill {
        byte_offset: u32,
    },
    /// Byte offset in the ABI's incoming stack-argument area. Machine emission
    /// accounts only for the assigned frame and return-address bias.
    IncomingStack {
        byte_offset: u32,
    },
}
