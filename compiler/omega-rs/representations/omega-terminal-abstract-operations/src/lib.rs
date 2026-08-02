#![forbid(unsafe_code)]

//! Source-independent Omega realization requirements lowered from terminal Psi.
//!
//! This small representation is the replacement seed for the legacy
//! source-shaped abstract-operation plan. It deliberately carries stable Psi
//! provenance and scalar semantics, but no syntax tree, arena handle,
//! `ExpressionHandle`, source statement, target register, or storage choice.

use psi_core::{
    BlockId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};
use psi_terminal::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAbstractOperationPlan {
    pub terminal_psi: TerminalPsiIdentity,
    pub entry: MachineId,
    pub functions: Vec<TerminalAbstractFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAbstractFunction {
    pub machine: MachineId,
    pub entry: BlockId,
    /// Runtime values supplied by the caller, in declared terminal-Psi order.
    pub parameters: Vec<TerminalAbstractParameter>,
    pub result: TerminalAbstractResult,
    /// Operations in the verified terminal machine's executable order.
    pub operations: Vec<TerminalAbstractOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAbstractParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAbstractResult {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAbstractOperation {
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        value: IntegerValue,
    },
    BooleanConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
    },
    WrappingIntegerAdd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerAdd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerSubtract {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerSubtract {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerMultiply {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerMultiply {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    Jump {
        psi_edge: EdgeId,
        target: BlockId,
        bindings: Vec<TerminalValueBinding>,
    },
    Return {
        psi_edge: EdgeId,
        result: ValueId,
        value: ValueId,
        scalar_type: ScalarType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalValueBinding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
}
