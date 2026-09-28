//! Durable post-layout rewrite evidence, not optimization authority.

use selected_instructions::SelectedInstructionId;

/// Explicit post-layout optimization policy. It is neither part of the
/// required baseline layout nor an encoder heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86BranchRelaxationPolicy {
    X86RelaxConditionalBranchesToRel8V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationRevisionIdentity([u8; 32]);

impl X86BranchRelaxationRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X86BranchRelaxationAttemptOutcome {
    AlreadyShort,
    NearDisplacementOutsideI8,
    SelectedForRelaxation,
}

/// One branch inspected in deterministic function/block/instruction order.
/// Attempts stop at the selected branch in a mutating iteration; the terminal
/// no-change iteration records the complete remaining scan.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationAttempt {
    pub iteration: u64,
    pub input: X86BranchRelaxationRevisionIdentity,
    pub instruction: SelectedInstructionId,
    pub offset: u64,
    pub byte_displacement: i64,
    pub encoded_bytes: u8,
    pub outcome: X86BranchRelaxationAttemptOutcome,
}

/// Exact evidence for one monotone six-byte-near to two-byte-short rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationAction {
    pub iteration: u64,
    pub input: X86BranchRelaxationRevisionIdentity,
    pub output: X86BranchRelaxationRevisionIdentity,
    pub instruction: SelectedInstructionId,
    pub old_offset: u64,
    pub new_offset: u64,
    pub old_displacement: i64,
    pub new_displacement: i64,
    pub old_bytes: Vec<u8>,
    pub new_bytes: Vec<u8>,
}
