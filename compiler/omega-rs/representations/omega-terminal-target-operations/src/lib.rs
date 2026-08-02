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
