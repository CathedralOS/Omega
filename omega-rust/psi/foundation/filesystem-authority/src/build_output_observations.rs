//! Observed required-output obligations and their sealed-file receipts.
//!
//! Execution owners issue these records; settlement checks them against actual
//! staged files. They describe one activation and confer no filesystem access.

use crate::FilesystemGrantRootIdentity;

/// Maximum `BuildOutput::require` obligations one build activation may issue.
/// This is an evaluator evidence ceiling, not a language limit; it matches the
/// compiler's declared required-output roster bound.
pub const MAX_BUILD_OUTPUT_OBLIGATIONS: usize = 4_096;

/// Settlement state of one compiler-issued required-output obligation.
///
/// Obligations are linear at the custody boundary: `Pending` is the only
/// state `complete`/`fail` accept, a completed obligation cannot complete
/// again, and a failed obligation is sticky for the rest of the activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOutputObligationState {
    /// Reserved by `require`; not yet completed or failed.
    Pending,
    /// `complete` accepted the sealed file at `completed_at` (the count of
    /// completed filesystem attempts at that point) and issued receipt
    /// `receipt` (an index into the run's `build_output_receipts`).
    Completed { completed_at: usize, receipt: usize },
    /// `fail` consumed the obligation with the authored diagnostic. The
    /// activation can never publish a successful product set.
    Failed {
        failed_at: usize,
        diagnostic: Vec<u8>,
    },
}

/// One compiler-issued required-output obligation created by
/// `BuildOutput::require`. The `RequiredOutput` value evaluated code holds
/// carries only this row's index, so the declared name and state are
/// compiler-owned evidence that authored code cannot fabricate or reset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutputObligation {
    /// The grant-root identity of the issuing `builder.output` facet.
    pub root: FilesystemGrantRootIdentity,
    /// Declared canonical relative output name.
    pub relative_path: Vec<u8>,
    /// Count of completed filesystem attempts when `require` issued the
    /// obligation.
    pub issued_at: usize,
    pub state: BuildOutputObligationState,
    /// Completed filesystem-attempt ordinals that mutated this obligation's
    /// output path after it was completed. Publication requires this to be
    /// empty.
    pub post_completion_mutations: Vec<usize>,
}

impl BuildOutputObligation {
    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn issued_at(&self) -> usize {
        self.issued_at
    }

    pub const fn state(&self) -> &BuildOutputObligationState {
        &self.state
    }

    pub fn post_completion_mutations(&self) -> &[usize] {
        &self.post_completion_mutations
    }
}

/// One completion receipt issued by `BuildOutput::complete`. Receipts are
/// activation-local custody: the `OutputReceipt` value evaluated code holds
/// carries only this row's index, so a receipt cannot satisfy an obligation
/// from another occurrence or be reused against a different output name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutputReceipt {
    /// Index into the run's `build_output_obligations` identifying the
    /// obligation this receipt completed.
    pub obligation: usize,
    /// The grant-root identity and sealed relative path the completion bound.
    pub root: FilesystemGrantRootIdentity,
    pub relative_path: Vec<u8>,
    /// Filesystem-attempt ordinal that retired the file's last writer.
    pub sealed_at: usize,
    /// Count of completed filesystem attempts when `complete` issued the
    /// receipt.
    pub completed_at: usize,
}

impl BuildOutputReceipt {
    pub const fn obligation(&self) -> usize {
        self.obligation
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn sealed_at(&self) -> usize {
        self.sealed_at
    }

    pub const fn completed_at(&self) -> usize {
        self.completed_at
    }
}
