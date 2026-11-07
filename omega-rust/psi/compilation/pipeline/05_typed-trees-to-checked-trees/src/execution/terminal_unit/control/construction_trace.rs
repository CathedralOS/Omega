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
    declined_step: Cell<Option<DeclinedStep>>,
    result_custody: Cell<Option<ResultCustodyFailure>>,
}

/// One position of the trace, so a builder that set a statement aside for a
/// later owner can return to where it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TraceMark {
    phase: &'static str,
    statement_index: Option<u32>,
    result_custody: Option<ResultCustodyFailure>,
}

/// Counts from the actual failed result join, not a second custody analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResultCustodyFailure {
    pub(crate) binding: u32,
    pub(crate) local: SymbolHandle,
    pub(crate) producer_operation: usize,
    /// Implicit exit, call, cleanup, construction, reference and view owners.
    pub(crate) dispositions: [usize; 6],
    pub(crate) normal_caller_completion: bool,
}

/// A declined attempt belongs to one complete authored trace position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeclineMark {
    position: TraceMark,
    state: Option<SymbolHandle>,
}

#[derive(Debug, Clone, Copy)]
struct DeclinedStep {
    at: DeclineMark,
    phase: &'static str,
}

/// Preserve a specific attempted plan's refusal if every alternative fails.
/// Successful alternatives explicitly discard the note; early refusal through
/// `?` keeps the original statement and owner instead of the last fallback.
pub(crate) struct DeferredFailure<'a> {
    trace: &'a LocalConstructionTrace,
    at: Option<DeclineMark>,
}

impl DeferredFailure<'_> {
    pub(crate) fn retain(&mut self) {
        self.at = Some(self.trace.decline_mark());
    }

    pub(crate) fn succeeded(&mut self) {
        self.at = None;
    }
}

impl Drop for DeferredFailure<'_> {
    fn drop(&mut self) {
        if let Some(at) = self.at {
            self.trace.restore(&at.position);
            self.trace.state.set(at.state);
        }
    }
}

impl LocalConstructionTrace {
    pub(crate) fn deferred_failure(&self) -> DeferredFailure<'_> {
        DeferredFailure {
            trace: self,
            at: None,
        }
    }

    /// Enter a phase; the statement position resets with it.
    pub(crate) fn phase(&self, phase: &'static str) {
        self.declined_step.set(None);
        self.result_custody.set(None);
        self.phase.set(phase);
        self.statement_index.set(None);
    }

    /// The current position, to return to.
    pub(crate) fn mark(&self) -> TraceMark {
        TraceMark {
            phase: self.phase.get(),
            statement_index: self.statement_index.get(),
            result_custody: self.result_custody.get(),
        }
    }

    /// Return to a marked position.
    pub(crate) fn restore(&self, mark: &TraceMark) {
        self.declined_step.set(None);
        self.phase.set(mark.phase);
        self.statement_index.set(mark.statement_index);
        self.result_custody.set(mark.result_custody);
    }

    pub(crate) fn decline_mark(&self) -> DeclineMark {
        DeclineMark {
            position: self.mark(),
            state: self.state.get(),
        }
    }

    /// Keep the primary refusal site while naming the last attempted step.
    /// A later builder clears this note even when it enters the same phase.
    pub(crate) fn restore_with_declined_step(&self, mark: &DeclineMark) {
        let phase = self.phase.get();
        self.restore(&mark.position);
        if self.state.get() == mark.state && phase != mark.position.phase {
            self.declined_step
                .set(Some(DeclinedStep { at: *mark, phase }));
        }
    }

    /// The call statement the current phase is planning.
    pub(crate) fn statement(&self, index: Option<u32>) {
        self.declined_step.set(None);
        self.statement_index.set(index);
        self.result_custody.set(None);
    }

    /// The authored state a multi-state builder is planning; it persists
    /// across that state's phases until the builder moves on. A fused graph's
    /// state may belong to another machine of its recursive component.
    pub(crate) fn state(&self, state: Option<SymbolHandle>) {
        self.declined_step.set(None);
        self.state.set(state);
        self.result_custody.set(None);
    }

    pub(crate) fn result_custody_failure(&self, failure: ResultCustodyFailure) {
        self.result_custody.set(Some(failure));
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
        let phase = match self.declined_step.get() {
            Some(step) if step.at == self.decline_mark() => std::borrow::Cow::Owned(format!(
                "{} (last declined step: {})",
                self.phase.get(),
                step.phase,
            )),
            _ => std::borrow::Cow::Borrowed(self.phase.get()),
        };
        let phase = match self.result_custody.get() {
            Some(failure) => {
                let [implicit, call, cleanup, construction, reference, view] = failure.dispositions;
                std::borrow::Cow::Owned(format!(
                    "{phase}; result home {} local {:?}, producer operation {}, normal caller completion {}; dispositions: implicit={implicit}, call={call}, cleanup={cleanup}, construction={construction}, reference={reference}, view={view}",
                    failure.binding,
                    failure.local,
                    failure.producer_operation,
                    failure.normal_caller_completion,
                ))
            }
            None => phase,
        };
        plan_gap(
            program,
            machine,
            site,
            CheckedUnitPlanGapReason::Unsupported(phase),
        )
    }
}
