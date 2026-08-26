#![forbid(unsafe_code)]

//! Concrete register and stack homes assigned to the clean terminal-Psi target
//! operation lane.

use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use omega_target::NativeTarget;
use omega_terminal_target_operations::{
    MachineRegister, TerminalBoundaryByteSequenceArgument, TerminalBoundaryRealization,
    TerminalBoundaryScalarArgument, TerminalCompletionClaimSource,
    TerminalDirectPortReadU8Realization, TerminalLinuxExitGroupI32Realization,
    TerminalProviderExecutionBinding, TerminalPsiProvenance, TerminalTargetStructuralParameter,
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
pub struct TerminalAssignedOperationPlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TerminalAssignedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAssignedFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub operation: TerminalAssignedOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedOperation {
    UnitBody(TerminalAssignedUnitBody),
    ReturnStructuralScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TerminalTargetStructuralParameter>,
        copies: Vec<TerminalAssignedAggregateCopy>,
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
        structural_parameters: Vec<TerminalTargetStructuralParameter>,
        copies: Vec<TerminalAssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        returned_claims: Vec<ClaimId>,
    },
    ScalarReturnWithCleanup {
        scalar: Box<TerminalAssignedOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters:
            Vec<omega_terminal_target_operations::TerminalTargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
        psi_edge: EdgeId,
    },
    ReturnBoundaryPortReadU8 {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        boundary: psi_core::BoundaryMachineId,
        provider_execution: TerminalProviderExecutionBinding,
        realization: TerminalDirectPortReadU8Realization,
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<TerminalCompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
        call_plan: omega_calling_conventions::CallPlan,
        structural_parameters:
            Vec<omega_terminal_target_operations::TerminalTargetStructuralParameter>,
    },
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
    /// Assigned form of one finite short-circuit Boolean tree. Exact cleanup
    /// ownership remains attached to each leaf's terminal-Psi return edge.
    BooleanControlWithCleanup {
        control: TerminalAssignedBooleanControl,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters:
            Vec<omega_terminal_target_operations::TerminalTargetStructuralParameter>,
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
        location: TerminalAssignedScalarLocation,
    },
    ReturnBooleanParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalAssignedScalarLocation,
    },
    ReturnBooleanNotParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalAssignedScalarLocation,
    },
    ReturnBooleanSharedConvergence {
        psi_edge: EdgeId,
        control: TerminalAssignedBooleanControl,
    },
    ReturnBooleanExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        frame: TerminalExpressionFrame,
        expression: TerminalAssignedBooleanExpression,
    },
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        frame: TerminalExpressionFrame,
        expression: TerminalAssignedIntegerExpression,
    },
    ReturnIntegerConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalAssignedScalarLocation,
        scalar_type: IntegerType,
        when_true: TerminalAssignedConditionalIntegerArm,
        when_false: TerminalAssignedConditionalIntegerArm,
    },
    ReturnIntegerExpressionConditionalControl {
        condition_source: ValueId,
        condition_frame: TerminalExpressionFrame,
        condition: TerminalAssignedBooleanExpression,
        scalar_type: IntegerType,
        when_true: TerminalAssignedConditionalIntegerArm,
        when_false: TerminalAssignedConditionalIntegerArm,
    },
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalAssignedScalarLocation,
        when_true: TerminalAssignedConditionalBooleanArm,
        when_false: TerminalAssignedConditionalBooleanArm,
    },
    ReturnBooleanExpressionConditionalControl {
        condition_source: ValueId,
        condition_frame: TerminalExpressionFrame,
        condition: TerminalAssignedBooleanExpression,
        when_true: TerminalAssignedConditionalBooleanArm,
        when_false: TerminalAssignedConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAssignedUnitBody {
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    pub parameters: Vec<TerminalTargetStructuralParameter>,
    pub operations: Vec<TerminalAssignedUnitOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAssignedAggregateCopy {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedUnitOperation {
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
        copies: Vec<TerminalAssignedAggregateCopy>,
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
pub enum TerminalAssignedBooleanExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: psi_core::MachineId,
        arguments: Vec<TerminalAssignedCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalAssignedScalarLocation,
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
        operand: Box<TerminalAssignedBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<TerminalAssignedBooleanExpression>,
        right: Box<TerminalAssignedBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAssignedConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<TerminalAssignedBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedBooleanControl {
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
        location: TerminalAssignedScalarLocation,
    },
    ReturnNotParameter {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalAssignedScalarLocation,
    },
    ReturnExpression {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: TerminalExpressionFrame,
        expression: TerminalAssignedBooleanExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalAssignedScalarLocation,
        when_true: TerminalAssignedConditionalBooleanArm,
        when_false: TerminalAssignedConditionalBooleanArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: TerminalExpressionFrame,
        condition: TerminalAssignedBooleanExpression,
        when_true: TerminalAssignedConditionalBooleanArm,
        when_false: TerminalAssignedConditionalBooleanArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAssignedConditionalIntegerArm {
    pub psi_edge: EdgeId,
    pub control: Box<TerminalAssignedIntegerControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedIntegerControl {
    Crash {
        psi_crash_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Return {
        psi_return_edge: EdgeId,
        source_value: ValueId,
        frame: TerminalExpressionFrame,
        expression: TerminalAssignedIntegerExpression,
    },
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalAssignedScalarLocation,
        when_true: TerminalAssignedConditionalIntegerArm,
        when_false: TerminalAssignedConditionalIntegerArm,
    },
    ConditionalExpression {
        condition_source: ValueId,
        condition_frame: TerminalExpressionFrame,
        condition: TerminalAssignedBooleanExpression,
        when_true: TerminalAssignedConditionalIntegerArm,
        when_false: TerminalAssignedConditionalIntegerArm,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExpressionFrame {
    /// Aligned bytes reserved before evaluating the expression.
    pub byte_size: u32,
    /// Incoming ABI registers copied into stable frame homes before any
    /// expression scratch register can overwrite them.
    pub register_spills: Vec<TerminalEntryRegisterSpill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEntryRegisterSpill {
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedIntegerExpression {
    Call {
        psi_operation: OperationId,
        source_value: ValueId,
        callee: psi_core::MachineId,
        arguments: Vec<TerminalAssignedCallArgument>,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalAssignedScalarLocation,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<TerminalAssignedIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TerminalAssignedIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        source_type: IntegerType,
        operand: Box<TerminalAssignedIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingShiftLeft {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TerminalAssignedIntegerExpression>,
        count: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TerminalAssignedIntegerExpression>,
        count: Box<TerminalAssignedIntegerExpression>,
    },
    ExactShiftLeft {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<TerminalAssignedIntegerExpression>,
        count: Box<TerminalAssignedIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        count_type: IntegerType,
        value: Box<TerminalAssignedIntegerExpression>,
        count: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    ExactAdd {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingAdd {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingSubtract {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    ExactSubtract {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingSubtract {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingMultiply {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    ExactMultiply {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    ExactDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAssignedCallArgument {
    pub scalar_type: psi_core::ScalarType,
    /// Concrete ABI home populated after all sibling arguments have been
    /// evaluated. Outgoing stack offsets are relative to the call plan's ABI
    /// argument area and therefore already include any shadow/home space.
    pub destination: TerminalAssignedCallDestination,
    /// Stable frame slot holding the fully evaluated argument until every
    /// sibling argument is ready for simultaneous ABI placement.
    pub spill_byte_offset: u32,
    pub expression: TerminalAssignedScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAssignedCallDestination {
    Register(MachineRegister),
    OutgoingStack { byte_offset: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedScalarExpression {
    Boolean(TerminalAssignedBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: TerminalAssignedIntegerExpression,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAssignedScalarLocation {
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
