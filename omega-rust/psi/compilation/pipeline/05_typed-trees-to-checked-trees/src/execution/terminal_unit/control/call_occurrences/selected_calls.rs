//! Account a selected invocation tree without treating its other leaves as
//! unscheduled work on the current branch. Each leaf still proves its complete
//! operand custody. Only the final all-leaf join can cover the statement.

use super::{CallOccurrenceLedger, CallOccurrencePrefix, CheckFacts, SymbolHandle};
use arena::{Handle, HandleSpan};
use checked_trees::{CheckedCompletionExpressionKind as Kind, FlowCallFact};
use std::collections::{BTreeMap, BTreeSet};

type CallKey = (u32, u32);

pub(in crate::execution::terminal_unit) struct SelectedInvocationAccounting {
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    calls: HandleSpan<FlowCallFact>,
    leaves: BTreeSet<CallKey>,
    bound: BTreeSet<CallKey>,
    accounted: BTreeSet<CallKey>,
    prefix: CallOccurrencePrefix,
    guard_maximum: Option<usize>,
    first: Handle<FlowCallFact>,
    expression_calls: BTreeMap<(usize, u32, u32), Option<Handle<FlowCallFact>>>,
}

fn key(call: Handle<FlowCallFact>) -> CallKey {
    (call.arena_index(), call.generation())
}

fn in_span(span: HandleSpan<FlowCallFact>, handle: Handle<FlowCallFact>) -> bool {
    handle
        .arena_index()
        .checked_sub(span.start().arena_index())
        .and_then(|offset| span.handles().nth(offset as usize))
        == Some(handle)
}

impl SelectedInvocationAccounting {
    pub(in crate::execution::terminal_unit) fn new(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        root: &checked_trees::CheckedCompletionRoot,
    ) -> Option<Self> {
        if root.refusal.is_some() {
            return None;
        }
        let calls = facts.flow.control.state(root.machine, root.state)?.calls;
        let roster = facts.flow.control.calls.span_or_empty(calls);
        let mut expression_calls = BTreeMap::new();
        for (handle, call) in calls.handles().zip(roster) {
            if call.statement_index != root.statement_ordinal as usize {
                continue;
            }
            let coordinate = (
                call.call_ordinal,
                call.authored_expression.arena_index(),
                call.authored_expression.generation(),
            );
            expression_calls
                .entry(coordinate)
                .and_modify(|prior| *prior = None)
                .or_insert(Some(handle));
        }
        let plans = &facts.values.completions;
        let mut pending = vec![(root.root, false)];
        let mut visited = BTreeMap::new();
        let mut leaves = BTreeSet::new();
        let mut first = None;
        while let Some((handle, completed)) = pending.pop() {
            if !plans.nodes.is_valid(handle) {
                return None;
            }
            let coordinate = (handle.arena_index(), handle.generation());
            if completed {
                visited.insert(coordinate, false);
                continue;
            }
            match visited.get(&coordinate) {
                Some(true) => return None,
                Some(false) => continue,
                None => {}
            }
            visited.insert(coordinate, true);
            let node = plans.nodes.get(handle);
            match &node.kind {
                Kind::Missing => return None,
                Kind::Invocation { source_call } => {
                    if !in_span(calls, *source_call) {
                        return None;
                    }
                    let call = facts.flow.control.calls.get(*source_call);
                    if call.statement_index != root.statement_ordinal as usize
                        || call.authored_expression != node.expression
                    {
                        return None;
                    }
                    leaves.insert(key(*source_call));
                    first.get_or_insert(*source_call);
                    visited.insert(coordinate, false);
                }
                Kind::Dispatch { arms, .. } => {
                    let arms = plans.arms.span(*arms)?;
                    if arms.is_empty() {
                        return None;
                    }
                    pending.push((handle, true));
                    pending.extend(arms.iter().rev().map(|arm| (arm.completion, false)));
                }
            }
        }
        let first = first?;
        let (prefix, guard_maximum) = super::collecting::statement_computation_prefix(
            program,
            facts,
            root.machine,
            root.state,
            root.statement_ordinal as usize,
            roster,
        )?;
        if !prefix.pending_minima.is_empty()
            || prefix.prefix.is_some()
            || prefix.calls.iter().any(|call| leaves.contains(&key(*call)))
        {
            return None;
        }
        let accounted = prefix.calls.iter().copied().map(key).collect();
        let computations = prefix
            .accounted_computations
            .into_iter()
            .map(|entry| {
                (
                    (
                        entry.state.arena_index(),
                        entry.state.generation(),
                        entry.statement,
                        entry.computation.arena_index(),
                        entry.computation.generation(),
                    ),
                    entry.minimum_call_ordinal,
                )
            })
            .collect();
        Some(Self {
            machine: root.machine,
            state: root.state,
            statement: root.statement_ordinal as usize,
            calls,
            leaves,
            bound: BTreeSet::new(),
            accounted,
            prefix: CallOccurrencePrefix {
                call_keys: prefix.calls.iter().copied().map(key).collect(),
                calls: prefix.calls,
                computations,
            },
            guard_maximum,
            first,
            expression_calls,
        })
    }

    pub(super) fn contains_call(&self, facts: &CheckFacts, call: &FlowCallFact) -> bool {
        facts
            .flow
            .control
            .calls
            .handle_of(call)
            .is_some_and(|handle| in_span(self.calls, handle))
    }

    pub(super) fn contains_leaf(
        &self,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &FlowCallFact,
        calls: &[FlowCallFact],
    ) -> bool {
        machine == self.machine
            && state == self.state
            && call.statement_index == self.statement
            && std::ptr::eq(calls, facts.flow.control.calls.span_or_empty(self.calls))
            && facts
                .flow
                .control
                .calls
                .handle_of(call)
                .is_some_and(|handle| self.leaves.contains(&key(handle)))
    }

    pub(super) fn operand_ledger(&self) -> (CallOccurrenceLedger<'_>, Option<usize>) {
        (
            CallOccurrenceLedger {
                prefix: Some(&self.prefix),
                ..Default::default()
            },
            self.guard_maximum,
        )
    }

    pub(super) fn account(
        &mut self,
        facts: &CheckFacts,
        call: &FlowCallFact,
        consumed: &[Handle<FlowCallFact>],
    ) -> Option<()> {
        let call = key(facts.flow.control.calls.handle_of(call)?);
        if !self.leaves.contains(&call)
            || consumed
                .iter()
                .any(|handle| self.leaves.contains(&key(*handle)))
        {
            return None;
        }
        self.accounted.extend(consumed.iter().copied().map(key));
        self.accounted.insert(call);
        self.bound.insert(call);
        Some(())
    }

    /// A checked operand may stop before its callee is invoked. Retain only
    /// calls actually owned by that completed prefix, plus the leaf's known
    /// stopped disposition. Unknown or unrelated source rows remain debt.
    pub(in crate::execution::terminal_unit) fn stopped_prefix(
        &mut self,
        facts: &CheckFacts,
        call: &FlowCallFact,
        operations: &[checked_trees::CheckedUnitEffectOperationPlan],
    ) -> Option<()> {
        let leaf = key(facts.flow.control.calls.handle_of(call)?);
        if !self.leaves.contains(&leaf) || call.statement_index != self.statement {
            return None;
        }
        let stop = operations
            .iter()
            .position(|operation| !operation.has_normal_continuation())?;
        let mut accounted = BTreeSet::new();
        for operation in &operations[..=stop] {
            for operation in operation.with_value_calls() {
                let Some(planned) = operation.call() else {
                    continue;
                };
                let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                    planned.source_site()
                else {
                    return None;
                };
                if planned.coordinate.statement_index as usize != self.statement {
                    return None;
                }
                let handle = self
                    .expression_calls
                    .get(&(
                        planned.coordinate.call_ordinal as usize,
                        expression.arena_index(),
                        expression.generation(),
                    ))
                    .copied()
                    .flatten()?;
                if self.leaves.contains(&key(handle)) {
                    return None;
                }
                accounted.insert(key(handle));
            }
        }
        // This accounts for source ownership, not execution of the callee.
        self.accounted.extend(accounted);
        self.accounted.insert(leaf);
        self.bound.insert(leaf);
        Some(())
    }

    pub(in crate::execution::terminal_unit) fn finish(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
    ) -> Option<()> {
        if self.bound != self.leaves {
            return None;
        }
        let machine = program.machine_by_symbol(self.machine)?;
        let state = program.state_in_machine(machine, self.state)?;
        let first = facts.flow.control.calls.get(self.first);
        let calls = facts.flow.control.calls.span_or_empty(self.calls);
        let active_arm = validation::transition_call_target(
            program,
            machine,
            state,
            self.statement,
            first.call_ordinal,
        );
        calls
            .iter()
            .filter(|call| call.statement_index == self.statement)
            .all(|call| {
                facts
                    .flow
                    .control
                    .calls
                    .handle_of(call)
                    .is_some_and(|handle| self.accounted.contains(&key(handle)))
                    || super::collecting::scheduled_dispatch_subject(
                        program,
                        facts,
                        machine,
                        state,
                        first,
                        call,
                        calls,
                        predecessors,
                    )
                    || super::collecting::scheduled_by_active_arm(call, predecessors)
                    || super::collecting::other_transition_arm(
                        program, machine, state, active_arm, call,
                    )
                    || facts.flow.control.is_retired(self.state, call)
                    || program
                        .symbols
                        .builtin_function_for_symbol(call.target_symbol)
                        .is_some_and(|function| function.has_empty_write_frame())
            })
            .then_some(())
    }
}
