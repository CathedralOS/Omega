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
    /// Phases entered so far. A route that entered more phases before
    /// declining got further than one that stopped at its precondition.
    progress: Cell<u32>,
}

/// One position of the trace, so a builder trying alternative routes for
/// the same statement can return to the route that got furthest.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceMark {
    phase: &'static str,
    statement_index: Option<u32>,
    progress: u32,
}

impl LocalConstructionTrace {
    /// Enter a phase; the statement position resets with it.
    pub(crate) fn phase(&self, phase: &'static str) {
        self.phase.set(phase);
        self.statement_index.set(None);
        self.progress.set(self.progress.get().wrapping_add(1));
    }

    /// The current position, to compare routes against or return to.
    pub(crate) fn mark(&self) -> TraceMark {
        TraceMark {
            phase: self.phase.get(),
            statement_index: self.statement_index.get(),
            progress: self.progress.get(),
        }
    }

    /// Whichever of `furthest` and the current position entered more phases
    /// since `baseline`. A later alternative that declines at its first
    /// precondition must not replace the route that reached the decisive
    /// requirement; on a tie the earlier route keeps its place.
    pub(crate) fn furthest(
        &self,
        furthest: Option<TraceMark>,
        baseline: &TraceMark,
    ) -> Option<TraceMark> {
        let current = self.mark();
        match furthest {
            Some(best)
                if best.progress.wrapping_sub(baseline.progress)
                    >= current.progress.wrapping_sub(baseline.progress) =>
            {
                Some(best)
            }
            _ => Some(current),
        }
    }

    /// Return to a marked position.
    pub(crate) fn restore(&self, mark: &TraceMark) {
        self.phase.set(mark.phase);
        self.statement_index.set(mark.statement_index);
        self.progress.set(mark.progress);
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
