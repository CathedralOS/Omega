#![forbid(unsafe_code)]

/*
Each native compilation target needs a concrete operation-level representation that joins portable
Terminal Psi semantics to target ABI placements, calling plans, and admitted boundary realizations
before instruction selection. We expose `TargetOperationPlan` and its subordinate call,
control-flow, boundary, operation, value, storage, and provenance vocabularies as the canonical
target-operations representation.
*/

/*
Joining target ABI facts to semantic operation and value identities at the plan root ensures that
legalization, register home assignment, and instruction selection can verify boundary and call
contracts without re-running provider admission or structural layout derivation.
*/

/*
Subordinate modules own call contracts (`calls`), block control graphs (`control_flow`), terminal
provenance (`provenance`), boundary realizations (`boundary`), ordered unit operations
(`operations`), value expressions (`values`), and durable home requirements (`storage`).
*/

use semantic_vocabulary::MachineId;
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    /// Selected deployment identity; native geometry alone cannot authorize platform services.
    /// Geometry-only inputs may omit it, but cannot use deployment-specific operations.
    pub profile: Option<target::TargetProfile>,
    pub entry: MachineId,
    pub functions: Vec<TargetFunction>,
    pub native_callback_arguments: Vec<TargetNativeCallbackArgument>, // Admission-retained callback custody rows joined to their registrar by Terminal operation and entry plan.
    pub borrowed_storage_layouts: runtime_abi::NativeBorrowedStorageLayouts, // Native storage geometry of borrowed boundary inputs and owned roots reached by foreign calls.
}

impl TargetOperationPlan {
    pub fn function(&self, machine: MachineId) -> Option<&TargetFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }
}

pub use target_vocabulary::*;
pub use target_vocabulary::{boundary, calls, provenance, storage, values};

pub mod control_flow;
pub use control_flow::{
    TargetControlBlock, TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlGraph,
    TargetControlSuccessor, TargetControlTerminator, TargetFunction, TargetOperationMetadata,
    TargetScalarBlockParameter, TargetStructuralCaseSource, TargetStructuralContract,
    TargetStructuralReturnSource,
};
pub mod operations;
pub use operations::{TargetInstruction, TargetInstructionSource, TargetUnitOperation};
