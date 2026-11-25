//! Executable Psi machines, their blocks, operations, invocations and successor edges.

pub(crate) mod call_contexts;
pub mod direct_calls;
pub mod generic_calls;
mod instructions;

pub(crate) mod atomic;
mod block;
pub(crate) mod float_result_policy;
pub(crate) mod invocations;
mod machine;
mod operands;
mod operation;
mod operation_arguments;
mod operation_patterns;
mod operation_result;
pub(crate) mod ranking;
pub(crate) mod record_field;
pub(crate) mod register_access;
pub(crate) mod scalar_case_field;
mod terminator;
pub(crate) mod trapping_integer;

/// The instruction or terminator whose execution consumes an obligation.
/// A block coordinate names its terminator, after its operations and before
/// choosing an outgoing edge; it never names block entry or one chosen edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExecutionSite {
    Block(semantic_vocabulary::BlockId),
    Operation(semantic_vocabulary::OperationId),
}

impl From<semantic_vocabulary::OperationId> for ExecutionSite {
    fn from(operation: semantic_vocabulary::OperationId) -> Self {
        Self::Operation(operation)
    }
}

pub use atomic::{AtomicAccessEvent, AtomicReadModifyWrite, FenceOrdering, MemoryOrdering};
pub use float_result_policy::FloatResultPolicy;
pub use invocations::{
    BoundaryInvocation, BoundaryInvocationCompletion, CallableInvocation, CallableInvocationInputs,
    ExternalReach, ModuleInvocation, ModuleInvocationInputs,
};
pub use machine::{Block, TerminalMachine, structural_parameter_position};
pub use operation::{
    ByteSequenceOperand, ElementViewRegion, Operation, OperationKind, OperationResult,
    ReferenceStorageView, StorageLocationQuery, StructuralOperationResult,
    StructuralResultClaimBinding, StructuralStoreDisposition,
};
pub use ranking::{
    TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc,
};
pub use record_field::{RecordFieldInitializer, RecordFieldValue};
pub use register_access::RegisterAccessEvent;
pub use scalar_case_field::ScalarCaseField;
pub use semantic_vocabulary::TargetMachineCallable;
pub use terminator::{CrashCause, StructuralCaseSuccessorEdge, SuccessorEdge, Terminator};
pub use trapping_integer::{TrappingIntegerOperation, TrappingIntegerPrimitive};
