//! Exact performed calls used by one immutable return-custody question.
//! A state's expression roster and each invocation's owners are read once.

use arena::Handle;
use checked_trees::{FlowCallFact, FlowControlFacts, FlowOperandFact};
use std::cell::RefCell;
use std::collections::BTreeMap;
use symbols::SymbolHandle;

type Identity = (u32, u32);
type ExpressionSite = (usize, Identity);
type StateCalls = BTreeMap<ExpressionSite, Option<Handle<FlowCallFact>>>;

fn key<T>(handle: Handle<T>) -> Identity {
    (handle.arena_index(), handle.generation())
}

pub(super) struct CompletionCalls<'facts> {
    control: &'facts FlowControlFacts,
    states: RefCell<BTreeMap<Identity, Option<StateCalls>>>,
    owners: RefCell<BTreeMap<Identity, Vec<SymbolHandle>>>,
    targets: RefCell<BTreeMap<Identity, Option<Option<SymbolHandle>>>>,
}

impl<'facts> CompletionCalls<'facts> {
    pub(super) fn new(control: &'facts FlowControlFacts) -> Self {
        Self {
            control,
            states: RefCell::default(),
            owners: RefCell::default(),
            targets: RefCell::default(),
        }
    }

    fn prepare_state(&self, state: SymbolHandle) -> Option<StateCalls> {
        let flow = self.control.unique_state_by_symbol(state)?;
        self.control.calls.span(flow.calls)?;
        let mut calls = StateCalls::new();
        for handle in flow.calls.handles() {
            let call = self.control.calls.get(handle);
            if !call.authored_expression.is_valid()
                || call.authored_dispatch_role
                    == checked_trees::FlowCallDispatchRole::SavedSubjectRead
            {
                continue;
            }
            let site = (call.statement_index, key(call.authored_expression));
            calls
                .entry(site)
                .and_modify(|previous| *previous = None)
                .or_insert(Some(handle));
        }
        Some(calls)
    }

    /// None withholds the state or an ambiguous occurrence; Some(None) is a
    /// complete roster with no performed call at this exact expression site.
    pub(super) fn ordinary(
        &self,
        operand: &FlowOperandFact,
    ) -> Option<Option<Handle<FlowCallFact>>> {
        let state = key(operand.state_symbol);
        if !self.states.borrow().contains_key(&state) {
            let calls = self.prepare_state(operand.state_symbol);
            self.states.borrow_mut().insert(state, calls);
        }
        let states = self.states.borrow();
        let calls = states.get(&state)?.as_ref()?;
        if !operand.expression.is_valid() {
            return Some(None);
        }
        match calls.get(&(operand.statement_index, key(operand.expression))) {
            Some(Some(call)) => Some(Some(*call)),
            Some(None) => None,
            None => Some(None),
        }
    }

    pub(super) fn invocation(
        &self,
        state: SymbolHandle,
        invocation: Handle<FlowCallFact>,
    ) -> Option<&'facts FlowCallFact> {
        // Preserve the old unique-state requirement independently of the
        // reverse query, which deliberately retains every possible owner.
        self.control.unique_state_by_symbol(state)?;
        if !self.control.calls.is_valid(invocation) {
            return None;
        }
        let identity = key(invocation);
        if !self.owners.borrow().contains_key(&identity) {
            let mut owners = self
                .control
                .call_owners(invocation)
                .into_iter()
                .map(|row| row.state_symbol)
                .collect::<Vec<_>>();
            owners.sort_by_key(|state| key(*state));
            self.owners.borrow_mut().insert(identity, owners);
        }
        self.owners
            .borrow()
            .get(&identity)?
            .binary_search_by_key(&key(state), |candidate| key(*candidate))
            .is_ok()
            .then(|| self.control.calls.get(invocation))
    }

    pub(super) fn target(
        &self,
        target: SymbolHandle,
        classify: impl FnOnce() -> Option<Option<SymbolHandle>>,
    ) -> Option<Option<SymbolHandle>> {
        let identity = key(target);
        if let Some(result) = self.targets.borrow().get(&identity) {
            return *result;
        }
        let result = classify();
        self.targets.borrow_mut().insert(identity, result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{FlowCallDispatchRole, FlowStateFact};
    use typed_trees::expression::ExpressionHandle;

    #[test]
    fn captured_return_call_roster_keeps_exact_sites_and_ambiguity() {
        let mut control = FlowControlFacts::default();
        let state = SymbolHandle::from_parts(1, 1);
        let expression = ExpressionHandle::from_parts(10, 1);
        let ordinary = |statement_index, authored_expression| FlowCallFact {
            statement_index,
            authored_expression,
            ..FlowCallFact::default()
        };
        let calls = control.calls.insert_many([
            ordinary(1, expression),
            FlowCallFact {
                authored_dispatch_role: FlowCallDispatchRole::SavedSubjectRead,
                ..ordinary(1, expression)
            },
            ordinary(2, expression),
            ordinary(3, expression),
            ordinary(3, expression),
            ordinary(1, ExpressionHandle::from_parts(10, 2)),
        ]);
        control.states.insert(FlowStateFact {
            state_symbol: state,
            calls,
            ..FlowStateFact::default()
        });
        let prepared = CompletionCalls::new(&control);
        let mut operand = FlowOperandFact {
            state_symbol: state,
            statement_index: 1,
            expression,
            ..FlowOperandFact::default()
        };
        assert_eq!(prepared.ordinary(&operand), Some(Some(calls.handle_at(0))));
        operand.statement_index = 2;
        assert_eq!(prepared.ordinary(&operand), Some(Some(calls.handle_at(2))));
        operand.statement_index = 3;
        assert_eq!(prepared.ordinary(&operand), None);
        operand.statement_index = 4;
        assert_eq!(prepared.ordinary(&operand), Some(None));
        operand.statement_index = 1;
        operand.expression = ExpressionHandle::from_parts(10, 2);
        assert_eq!(prepared.ordinary(&operand), Some(Some(calls.handle_at(5))));
        operand.state_symbol = SymbolHandle::from_parts(1, 2);
        assert_eq!(prepared.ordinary(&operand), None);
    }

    #[test]
    fn captured_return_invocation_keeps_owner_and_generation_checks() {
        let mut control = FlowControlFacts::default();
        let state = SymbolHandle::from_parts(1, 1);
        let foreign = SymbolHandle::from_parts(2, 1);
        let calls = control.calls.insert_many([FlowCallFact::default()]);
        control.states.insert(FlowStateFact {
            state_symbol: state,
            calls,
            ..FlowStateFact::default()
        });
        control.states.insert(FlowStateFact {
            state_symbol: foreign,
            ..FlowStateFact::default()
        });
        let prepared = CompletionCalls::new(&control);
        assert!(prepared.invocation(state, calls.start()).is_some());
        assert!(prepared.invocation(foreign, calls.start()).is_none());
        assert!(
            prepared
                .invocation(
                    state,
                    Handle::from_parts(calls.start().arena_index(), calls.start().generation() + 1)
                )
                .is_none()
        );
        drop(prepared);
        control.states.insert(FlowStateFact {
            state_symbol: state,
            calls,
            ..FlowStateFact::default()
        });
        assert!(
            CompletionCalls::new(&control)
                .invocation(state, calls.start())
                .is_none()
        );
    }

    #[test]
    fn captured_return_target_retains_negative_and_known_empty_classification() {
        let control = FlowControlFacts::default();
        let prepared = CompletionCalls::new(&control);
        let unavailable = SymbolHandle::from_parts(1, 1);
        let empty = SymbolHandle::from_parts(1, 2);
        let classifications = std::cell::Cell::new(0);
        for _ in 0..2 {
            assert_eq!(
                prepared.target(unavailable, || {
                    classifications.set(classifications.get() + 1);
                    None
                }),
                None
            );
            assert_eq!(
                prepared.target(empty, || {
                    classifications.set(classifications.get() + 1);
                    Some(None)
                }),
                Some(None)
            );
        }
        assert_eq!(classifications.get(), 2);
    }
}
