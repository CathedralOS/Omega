#![forbid(unsafe_code)]

//! Target-selected operations derived from source-independent terminal Omega
//! requirements.

use omega_target::NativeTarget;
use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetOperationPlan {
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
}
