#![forbid(unsafe_code)]

//! Concrete register and stack homes assigned to the clean terminal-Psi target
//! operation lane.

use omega_target::NativeTarget;
use omega_terminal_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};
use psi_terminal::{CrashCause, CrashPredicateIdentity, TerminalPsiIdentity};

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
    pub provenance: TerminalPsiProvenance,
    pub operation: TerminalAssignedOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedOperation {
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateIdentity>,
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
pub enum TerminalAssignedBooleanExpression {
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalAssignedScalarLocation,
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
        site_guard: Vec<CrashPredicateIdentity>,
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
        site_guard: Vec<CrashPredicateIdentity>,
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
        count_type: IntegerType,
        value: Box<TerminalAssignedIntegerExpression>,
        count: Box<TerminalAssignedIntegerExpression>,
    },
    ExactShiftRight {
        psi_operation: OperationId,
        count_type: IntegerType,
        value: Box<TerminalAssignedIntegerExpression>,
        count: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingAdd {
        psi_operation: OperationId,
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
    ExactDivide {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    ExactRemainder {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingDivide {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    WrappingRemainder {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingDivide {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingRemainder {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
    },
    SaturatingMultiply {
        psi_operation: OperationId,
        left: Box<TerminalAssignedIntegerExpression>,
        right: Box<TerminalAssignedIntegerExpression>,
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
