//! Where the ordinary checked-machine builder stopped.
//!
//! The builder returns `None` from dozens of guards. Rather than threading a
//! result through every helper, the builder marks each phase as it enters it
//! and the call statement it is planning; when it returns `None`, the last
//! marks name the requirement family that failed and the authored state and
//! statement it was planning. The trace is diagnostic only: it never changes
//! which bodies are admitted.

use std::cell::Cell;

use symbols::SymbolHandle;

use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
use checked_trees::{CheckedUnitPlanGap, CheckedUnitPlanGapReason};

#[derive(Debug, Default)]
pub(crate) struct LocalConstructionTrace {
    phase: Cell<&'static str>,
    state: Cell<Option<SymbolHandle>>,
    statement_index: Cell<Option<u32>>,
}

/// One position of the trace, so a builder that set a statement aside for a
/// later owner can return to where it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// across that state's phases until the builder moves on. A fused graph's
    /// state may belong to another machine of its recursive component.
    pub(crate) fn state(&self, state: Option<SymbolHandle>) {
        self.state.set(state);
    }

    /// The gap at the last marked position. A statement mark without a state
    /// belongs to the single-state builder's only state.
    pub(crate) fn gap(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
    ) -> CheckedUnitPlanGap {
        let state = self.state.get().or_else(|| {
            program
                .machine_by_symbol(machine)
                .and_then(|owner| program.machine_states(owner).first())
                .map(|state| state.symbol)
        });
        let site = match (state, self.statement_index.get()) {
            (Some(state), Some(index)) => GapSite::Statement { state, index },
            (Some(state), None) if self.state.get().is_some() => GapSite::State(state),
            _ => GapSite::Machine,
        };
        plan_gap(
            program,
            machine,
            site,
            CheckedUnitPlanGapReason::Unsupported(self.phase.get()),
        )
    }
}
