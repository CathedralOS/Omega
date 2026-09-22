//! Machines, blocks, operations, and successor edges.

mod machines;
mod operations;
mod ranking;
mod record_field;
mod scalar_case_field;
mod termination;

pub use machines::{Block, TerminalMachine};
pub use operations::{
    Operation, OperationKind, OperationResult, StructuralOperationResult,
    StructuralResultClaimBinding,
};
pub use ranking::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc,
};
pub use record_field::{RecordFieldInitializer, RecordFieldValue};
pub use scalar_case_field::ScalarCaseField;
pub use termination::{CrashCause, StructuralCaseSuccessorEdge, SuccessorEdge, Terminator};
