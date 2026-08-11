#![forbid(unsafe_code)]

//! Source-independent Omega realization requirements lowered from terminal Psi.
//!
//! This small representation is the replacement seed for the legacy
//! source-shaped abstract-operation plan. It deliberately carries stable Psi
//! provenance and scalar semantics, but no syntax tree, arena handle,
//! `ExpressionHandle`, source statement, target register, or storage choice.

use psi_core::{
    BlockId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    ValueId,
};
use psi_terminal::{CrashCause, TerminalPsiIdentity};

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
    /// Canonical block starts in `operations`. This keeps conditional targets
    /// source-independent without flattening away control-flow identity.
    pub block_entries: Vec<TerminalAbstractBlockEntry>,
    /// Operations in canonical block order. Straight-line functions retain
    /// their historical executable order.
    pub operations: Vec<TerminalAbstractOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalAbstractBlockEntry {
    pub block: BlockId,
    pub operation_offset: usize,
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
    Call {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        arguments: Vec<ValueId>,
    },
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
    BooleanNot {
        psi_operation: OperationId,
        result: ValueId,
        operand: ValueId,
    },
    BooleanEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        operand: ValueId,
    },
    IntegerWiden {
        psi_operation: OperationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    IntegerBitwiseAnd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerShiftLeft {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerShiftRight {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftLeft {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftRight {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
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
    ExactIntegerDivide {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    ExactIntegerRemainder {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerDivide {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerRemainder {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerDivide {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerRemainder {
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
    Conditional {
        condition: ValueId,
        when_true: TerminalAbstractSuccessor,
        when_false: TerminalAbstractSuccessor,
    },
    Return {
        psi_edge: EdgeId,
        result: ValueId,
        value: ValueId,
        scalar_type: ScalarType,
    },
    /// A verified no-successor terminal. The audit-only site guard and frontier
    /// remain attached at the Omega boundary even though native realization
    /// only needs the closed cause and edge identity.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<psi_terminal::CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAbstractSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<TerminalValueBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalValueBinding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
}
