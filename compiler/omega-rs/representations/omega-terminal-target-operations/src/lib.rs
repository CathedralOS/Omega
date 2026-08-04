#![forbid(unsafe_code)]

//! Target-selected operations derived from source-independent terminal Omega
//! requirements.

use omega_target::NativeTarget;
use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};
use psi_terminal::TerminalPsiIdentity;

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
    /// Return a runtime integer expression lowered from exact-width terminal
    /// Psi operations. Every node has the enclosing result's integer type.
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        expression: TerminalTargetIntegerExpression,
    },
    /// Select between two integer expressions using one caller-supplied
    /// Boolean. Both structural and return edges remain explicit.
    ReturnIntegerConditionalExpressions {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: TerminalScalarParameterLocation,
        scalar_type: IntegerType,
        when_true: TerminalTargetConditionalIntegerExpression,
        when_false: TerminalTargetConditionalIntegerExpression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetConditionalIntegerExpression {
    pub psi_edge: EdgeId,
    pub psi_return_edge: EdgeId,
    pub source_value: ValueId,
    pub expression: TerminalTargetIntegerExpression,
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
