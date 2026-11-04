//! Cover every private call with an actual entry or a proved excluded arm.
//! Missing observations remain unknown; an empty live entry set proves only
//! that the requested private prefix cannot execute, never a runtime value.
use super::super::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
use super::{Arrival, ClosedArrivals, ExpressionHandle};
use symbols::SymbolHandle;
use typed_trees::machine::Machine;

pub(super) struct PrivateArrivalObserver<'query> {
    pub(super) requirement: &'query mut dyn StructuralPrefixVisitor,
    pub(super) entry: SymbolHandle,
    pub(super) root: SymbolHandle,
    pub(super) observed: Vec<Arrival>,
    pub(super) excluded: Vec<Arrival>,
    pub(super) boundaries: Vec<SymbolHandle>,
    pub(super) closed: std::sync::Arc<ClosedArrivals>,
}

impl PrivateArrivalObserver<'_> {
    /// Every closed syntactic use needs either an actual entry or a formed
    /// guard that excludes it. Missing observations are never absence proof.
    pub(super) fn finish_coverage(&mut self) -> bool {
        let complete =
            !self.closed.expected.is_empty()
                && self.closed.expected.iter().all(|arrival| {
                    self.observed.contains(arrival) || self.excluded.contains(arrival)
                });
        if complete && self.observed.is_empty() {
            // All calls were excluded before their operands. The requested
            // private prefix has no executable path and needs no value fact.
            self.requirement.entered();
        }
        complete
    }
}

impl StructuralPrefixVisitor for PrivateArrivalObserver<'_> {
    fn site(&self) -> StructuralPrefixSite {
        self.requirement.site()
    }
    fn is_complete(&self) -> bool {
        self.requirement.is_complete()
    }
    fn entered(&mut self) {
        self.requirement.entered();
    }
    fn unresolved(&mut self, machine: &Machine) {
        self.requirement.unresolved(machine);
    }
    fn stops_at_machine(&self, machine: SymbolHandle) -> bool {
        // Each independent entry is checked from arbitrary inputs under its
        // own requirements. A call back to it starts another invocation; do
        // not unfold it with values from the invocation currently being checked.
        // Calls outside this helper's closed callers cannot add an arrival.
        self.boundaries.contains(&machine)
            || !self.closed.bodies.contains(&machine)
            || self.requirement.stops_at_machine(machine)
    }
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let program = observation.0;
        if let Some(root) = program.machine_by_symbol(self.root) {
            self.requirement.observe_private_arrival(root, observation);
        } else {
            self.requirement.unresolved(observation.1);
        }
    }
    fn entered_body(
        &mut self,
        caller: SymbolHandle,
        statement: usize,
        expression: Option<ExpressionHandle>,
        transition_target: typed_trees::statement::TransitionTargetHandle,
        entry: SymbolHandle,
    ) {
        self.requirement
            .entered_body(caller, statement, expression, transition_target, entry);
        if entry == self.entry {
            let arrival = Arrival {
                caller,
                statement,
                expression,
                transition_target,
            };
            if !self.observed.contains(&arrival) {
                self.observed.push(arrival);
            }
        }
    }

    fn excluded_body(
        &mut self,
        caller: SymbolHandle,
        statement: usize,
        expression: Option<ExpressionHandle>,
        transition_target: typed_trees::statement::TransitionTargetHandle,
        entry: SymbolHandle,
    ) {
        self.requirement
            .excluded_body(caller, statement, expression, transition_target, entry);
        if entry == self.entry {
            let arrival = Arrival {
                caller,
                statement,
                expression,
                transition_target,
            };
            if !self.excluded.contains(&arrival) {
                self.excluded.push(arrival);
            }
        }
    }
}
