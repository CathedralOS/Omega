//! Where the ordinary checked-machine builder stopped.
//!
//! The builder returns `None` from dozens of guards. Rather than threading a
//! result through every helper, the builder marks each phase as it enters it
//! and the call statement it is planning; when it returns `None`, the last
//! marks name the requirement family that failed. The trace is diagnostic
//! only: it never changes which bodies are admitted.

use std::cell::Cell;

use checked_trees::CheckedUnitPlanOmissionStage;

#[derive(Debug, Default)]
pub(crate) struct LocalConstructionTrace {
    phase: Cell<&'static str>,
    state_index: Cell<Option<u32>>,
    statement_index: Cell<Option<u32>>,
}

/// One position of the trace, so a builder that set a statement aside for a
/// later owner can return to where it was.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceMark {
    phase: &'static str,
    statement_index: Option<u32>,
}

impl LocalConstructionTrace {
    /// Enter a phase; the statement position resets with it.
    pub(crate) fn phase(&self, phase: &'static str) {
        self.phase.set(phase);
        self.statement_index.set(None);
    }

    /// The current position, to return to.
    pub(crate) fn mark(&self) -> TraceMark {
        TraceMark {
            phase: self.phase.get(),
            statement_index: self.statement_index.get(),
        }
    }

    /// Return to a marked position.
    pub(crate) fn restore(&self, mark: &TraceMark) {
        self.phase.set(mark.phase);
        self.statement_index.set(mark.statement_index);
    }

    /// The call statement the current phase is planning.
    pub(crate) fn statement(&self, index: Option<u32>) {
        self.statement_index.set(index);
    }

    /// The authored state a multi-state builder is planning; it persists
    /// across that state's phases until the builder moves on.
    pub(crate) fn state(&self, index: Option<u32>) {
        self.state_index.set(index);
    }

    pub(crate) fn stage(&self) -> CheckedUnitPlanOmissionStage {
        CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: self.phase.get(),
            state_index: self.state_index.get(),
            statement_index: self.statement_index.get(),
        }
    }
}
