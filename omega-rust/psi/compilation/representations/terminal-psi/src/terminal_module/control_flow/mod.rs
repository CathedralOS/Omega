//! Machines, blocks, operations, and successor edges.

mod atomic;
mod float_result_policy;
mod machines;
mod operations;
mod ranking;
mod record_field;
mod register_access;
mod scalar_case_field;
mod target_families;
mod termination;
mod trapping_integer;

pub use atomic::{AtomicAccessEvent, AtomicReadModifyWrite, FenceOrdering, MemoryOrdering};
pub use float_result_policy::FloatResultPolicy;
pub use machines::{Block, TerminalMachine, structural_parameter_position};
pub use operations::{
    ByteSequenceOperand, ElementViewRegion, Operation, OperationKind, OperationResult,
    ReferenceStorageView, StructuralOperationResult, StructuralResultClaimBinding,
    StructuralStoreDisposition,
};
pub use ranking::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc,
};
pub use record_field::{RecordFieldInitializer, RecordFieldValue};
pub use register_access::RegisterAccessEvent;
pub use scalar_case_field::ScalarCaseField;
pub use target_families::{
    TargetMachineCallable, TargetMachineFamily, TargetMachineFamilyMember, TargetMachineSupply,
};
pub use termination::{CrashCause, StructuralCaseSuccessorEdge, SuccessorEdge, Terminator};
pub use trapping_integer::{TrappingIntegerOperation, TrappingIntegerPrimitive};
