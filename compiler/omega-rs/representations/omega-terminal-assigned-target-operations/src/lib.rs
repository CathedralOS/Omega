#![forbid(unsafe_code)]

//! Concrete register and stack homes assigned to the clean terminal-Psi target
//! operation lane.

use omega_target::NativeTarget;
use omega_terminal_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};
use psi_terminal::TerminalPsiIdentity;

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
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalAssignedScalarLocation,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAssignedConditionalBooleanArm {
    pub psi_edge: EdgeId,
    pub control: Box<TerminalAssignedBooleanControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAssignedBooleanControl {
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
    Conditional {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalAssignedScalarLocation,
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
