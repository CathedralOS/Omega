#![forbid(unsafe_code)]

//! Target-selected operations derived from source-independent terminal Omega
//! requirements.

use omega_target::NativeTarget;
use psi_core::{ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};
use psi_terminal::{CrashCause, CrashPredicateIdentity, TerminalPsiIdentity};

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
    pub provenance: TerminalPsiProvenance,
    pub operation: TerminalTargetOperation,
}

/// Ordered terminal-Psi sources refined into one target function.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalPsiProvenance {
    pub operations: Vec<OperationId>,
    pub edges: Vec<EdgeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetOperation {
    /// End the execution domain at one verified terminal-Psi crash edge.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateIdentity>,
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
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
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
        site_guard: Vec<CrashPredicateIdentity>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarParameterLocation {
    Register(MachineRegister),
    /// Byte offset in the ABI's incoming stack-argument area, excluding an
    /// architecture-specific return-address bias.
    IncomingStack {
        byte_offset: u32,
    },
}
