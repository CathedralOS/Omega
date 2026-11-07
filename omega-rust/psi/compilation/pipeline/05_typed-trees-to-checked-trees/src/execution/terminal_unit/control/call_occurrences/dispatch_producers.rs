//! Resolve each saved call to its unique already scheduled producer once.
//! The invocation-order comparison stays at the actual consuming leaf.

use super::{CallOccurrenceLedger, CheckFacts};
use arena::Handle;
use checked_trees::{CheckedUnitEffectOperationPlan, FlowCallFact};
use std::collections::{BTreeMap, BTreeSet};
use typed_trees::{machine::Machine, state::State};

type CallKey = (u32, u32);
pub(super) type CallOrder = (usize, usize);

fn key(handle: Handle<FlowCallFact>) -> CallKey {
    (handle.arena_index(), handle.generation())
}

pub(super) fn authenticated_producer<'a>(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &State,
    candidate: &FlowCallFact,
    find: impl FnOnce(usize, typed_trees::expression::ExpressionHandle) -> Option<&'a FlowCallFact>,
    owners: impl FnOnce(&FlowCallFact) -> Option<usize>,
) -> Option<CallOrder> {
    let (statement, expression) = validation::saved_dispatch_call(
        program,
        state,
        candidate.statement_index,
        candidate.authored_expression,
    )?;
    let producer = find(statement, expression)?;
    if facts.flow.control.is_retired(state.symbol, producer)
        || producer.target_symbol != candidate.target_symbol
        || owners(producer)? != 1
    {
        return None;
    }
    Some((producer.statement_index, producer.call_ordinal))
}

pub(super) struct DispatchProducers {
    by_call: BTreeMap<CallKey, CallOrder>,
}

impl DispatchProducers {
    pub(super) fn prepare(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        state: &State,
        calls: &[FlowCallFact],
        statement: usize,
        predecessors: &[CheckedUnitEffectOperationPlan],
    ) -> Self {
        let mut expressions = BTreeMap::new();
        let mut coordinates = BTreeMap::new();
        for call in calls {
            let handle = facts.flow.control.calls.handle_of(call);
            expressions
                .entry((
                    call.statement_index,
                    call.authored_expression.arena_index(),
                    call.authored_expression.generation(),
                ))
                .and_modify(|prior| *prior = None)
                .or_insert(handle);
            coordinates
                .entry((call.statement_index, call.call_ordinal))
                .and_modify(|prior| *prior = None)
                .or_insert(handle);
        }
        let mut owner_counts = BTreeMap::<CallKey, usize>::new();
        let mut unknown_operands = BTreeMap::<usize, BTreeSet<CallKey>>::new();
        for operation in predecessors {
            let Some(planned) = operation.call() else {
                continue;
            };
            let Some(prior_handle) = coordinates
                .get(&(
                    planned.coordinate.statement_index as usize,
                    planned.coordinate.call_ordinal as usize,
                ))
                .copied()
                .flatten()
            else {
                return Self {
                    by_call: BTreeMap::new(),
                };
            };
            let prior = facts.flow.control.calls.get(prior_handle);
            if facts.flow.control.is_retired(state.symbol, prior) {
                return Self {
                    by_call: BTreeMap::new(),
                };
            }
            let prior_key = key(prior_handle);
            *owner_counts.entry(prior_key).or_default() += 1;
            let mut consumed = CallOccurrenceLedger::default();
            let complete = validation::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                prior.statement_index,
                prior.call_ordinal,
            )
            .and_then(|site| {
                super::collecting::collect_argument_calls(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    prior,
                    validation::call_site_argument_expressions(program, &site),
                    calls,
                    &mut consumed,
                )
            })
            .is_some();
            if !complete {
                unknown_operands
                    .entry(prior.statement_index)
                    .or_default()
                    .insert(prior_key);
                continue;
            }
            // One prior operation owns a producer at most once, even when
            // several retained operand roles name the same completed value.
            let owned = consumed
                .iter()
                .filter_map(|handle| {
                    let row = facts.flow.control.calls.get(*handle);
                    (row.statement_index == prior.statement_index && key(*handle) != prior_key)
                        .then_some(key(*handle))
                })
                .collect::<BTreeSet<_>>();
            for producer in owned {
                *owner_counts.entry(producer).or_default() += 1;
            }
        }
        let mut by_call = BTreeMap::new();
        for candidate in calls
            .iter()
            .filter(|call| call.statement_index == statement)
        {
            let producer = authenticated_producer(
                program,
                facts,
                state,
                candidate,
                |statement, expression| {
                    expressions
                        .get(&(statement, expression.arena_index(), expression.generation()))
                        .copied()
                        .flatten()
                        .map(|handle| facts.flow.control.calls.get(handle))
                },
                |producer| {
                    let producer_key = key(facts.flow.control.calls.handle_of(producer)?);
                    // The direct owner never needed its own operand query.
                    // Every other preceding call at this statement did.
                    if unknown_operands
                        .get(&producer.statement_index)
                        .is_some_and(|unknown| {
                            unknown.len() != 1 || !unknown.contains(&producer_key)
                        })
                    {
                        return None;
                    }
                    Some(owner_counts.get(&producer_key).copied().unwrap_or(0))
                },
            );
            if let Some(producer) = producer
                && let Some(handle) = facts.flow.control.calls.handle_of(candidate)
            {
                by_call.insert(key(handle), producer);
            }
        }
        Self { by_call }
    }

    pub(super) fn producer(&self, handle: Handle<FlowCallFact>) -> Option<CallOrder> {
        self.by_call.get(&key(handle)).copied()
    }

    pub(super) fn before(&self, handle: Handle<FlowCallFact>, invocation: &FlowCallFact) -> bool {
        self.producer(handle).is_some_and(|producer| {
            producer < (invocation.statement_index, invocation.call_ordinal)
        })
    }
}
