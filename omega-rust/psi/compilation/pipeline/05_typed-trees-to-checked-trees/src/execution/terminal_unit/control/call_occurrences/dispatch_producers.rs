//! Resolve each saved call to its unique already scheduled producer once.
//! The invocation-order comparison stays at the actual consuming leaf.

use super::{CallOccurrenceLedger, CheckFacts};
use arena::Handle;
use checked_trees::{CheckedUnitEffectOperationPlan, FlowCallFact};
use std::collections::{BTreeMap, BTreeSet};
use typed_trees::{machine::Machine, state::State};

type CallKey = (u32, u32);
type CallOrder = (usize, usize);

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
    let producer = source_producer(program, facts, state, candidate, find)?;
    (owners(producer)? == 1).then_some((producer.statement_index, producer.call_ordinal))
}

fn source_producer<'a>(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &State,
    candidate: &FlowCallFact,
    find: impl FnOnce(usize, typed_trees::expression::ExpressionHandle) -> Option<&'a FlowCallFact>,
) -> Option<&'a FlowCallFact> {
    let (statement, expression) = validation::saved_dispatch_call(
        program,
        state,
        candidate.statement_index,
        candidate.authored_expression,
    )?;
    let producer = find(statement, expression)?;
    (!facts.flow.control.is_retired(state.symbol, producer)
        && producer.target_symbol == candidate.target_symbol)
        .then_some(producer)
}

#[derive(Clone, Copy)]
struct Producer {
    key: CallKey,
    order: CallOrder,
}

#[derive(Default)]
struct Owners {
    counts: BTreeMap<CallKey, usize>,
    unknown_operands: BTreeMap<usize, BTreeSet<CallKey>>,
    invalid: bool,
}

impl Owners {
    fn count(&self, producer: Producer) -> Option<usize> {
        // A direct owner skips its own operand query. All other preceding
        // calls at this statement must supply complete operand inventories.
        if self.invalid
            || self
                .unknown_operands
                .get(&producer.order.0)
                .is_some_and(|unknown| unknown.len() != 1 || !unknown.contains(&producer.key))
        {
            return None;
        }
        Some(self.counts.get(&producer.key).copied().unwrap_or(0))
    }
}

pub(super) struct DispatchProducers {
    by_call: BTreeMap<CallKey, Producer>,
    orders: BTreeMap<CallKey, CallOrder>,
    coordinates: BTreeMap<CallOrder, Option<Handle<FlowCallFact>>>,
    common: Owners,
    predecessor_count: usize,
}

/// Prefix candidates retain their producer identities independently of who
/// scheduled them. A leaf can add an owner or make an earlier owner ambiguous.
pub(super) struct PrefixProducers {
    candidates: BTreeSet<CallKey>,
    common: BTreeMap<usize, BTreeSet<(usize, CallKey)>>,
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
        let mut orders = BTreeMap::new();
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
            if let Some(handle) = handle {
                orders.insert(key(handle), (call.statement_index, call.call_ordinal));
            }
        }
        let mut by_call = BTreeMap::new();
        for candidate in calls
            .iter()
            .filter(|call| call.statement_index == statement)
        {
            let producer =
                source_producer(program, facts, state, candidate, |statement, expression| {
                    expressions
                        .get(&(statement, expression.arena_index(), expression.generation()))
                        .copied()
                        .flatten()
                        .map(|handle| facts.flow.control.calls.get(handle))
                });
            if let Some(producer) = producer
                && let Some(producer_handle) = facts.flow.control.calls.handle_of(producer)
                && let Some(handle) = facts.flow.control.calls.handle_of(candidate)
            {
                by_call.insert(
                    key(handle),
                    Producer {
                        key: key(producer_handle),
                        order: (producer.statement_index, producer.call_ordinal),
                    },
                );
            }
        }
        let mut result = Self {
            by_call,
            orders,
            coordinates,
            common: Owners::default(),
            predecessor_count: predecessors.len(),
        };
        result.common = result.owners(program, facts, machine, state, calls, predecessors);
        result
    }

    fn owners(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        state: &State,
        calls: &[FlowCallFact],
        predecessors: &[CheckedUnitEffectOperationPlan],
    ) -> Owners {
        let mut owners = Owners::default();
        for operation in predecessors {
            let Some(planned) = operation.call() else {
                continue;
            };
            let Some(prior_handle) = self
                .coordinates
                .get(&(
                    planned.coordinate.statement_index as usize,
                    planned.coordinate.call_ordinal as usize,
                ))
                .copied()
                .flatten()
            else {
                owners.invalid = true;
                break;
            };
            let prior = facts.flow.control.calls.get(prior_handle);
            if facts.flow.control.is_retired(state.symbol, prior) {
                owners.invalid = true;
                break;
            }
            let prior_key = key(prior_handle);
            *owners.counts.entry(prior_key).or_default() += 1;
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
                owners
                    .unknown_operands
                    .entry(prior.statement_index)
                    .or_default()
                    .insert(prior_key);
                continue;
            }
            // Several roles in one prior operation still constitute one owner.
            let owned = consumed
                .iter()
                .filter_map(|handle| {
                    let row = facts.flow.control.calls.get(*handle);
                    (row.statement_index == prior.statement_index && key(*handle) != prior_key)
                        .then_some(key(*handle))
                })
                .collect::<BTreeSet<_>>();
            for producer in owned {
                *owners.counts.entry(producer).or_default() += 1;
            }
        }
        owners
    }

    pub(super) fn local_owners(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        state: &State,
        calls: &[FlowCallFact],
        predecessors: &[CheckedUnitEffectOperationPlan],
    ) -> Option<LocalProducers> {
        let local = self.owners(
            program,
            facts,
            machine,
            state,
            calls,
            predecessors.get(self.predecessor_count..)?,
        );
        Some(LocalProducers { owners: local })
    }

    pub(super) fn prefix(&self, calls: &[Handle<FlowCallFact>]) -> PrefixProducers {
        let mut prefix = PrefixProducers {
            candidates: BTreeSet::new(),
            common: BTreeMap::new(),
        };
        for handle in calls {
            if let Some(producer) = self.by_call.get(&key(*handle)).copied() {
                prefix.candidates.insert(producer.key);
                if self.common.count(producer) == Some(1) {
                    prefix
                        .common
                        .entry(producer.order.0)
                        .or_default()
                        .insert((producer.order.1, producer.key));
                }
            }
        }
        prefix
    }

    pub(super) fn before(&self, handle: Handle<FlowCallFact>, invocation: &FlowCallFact) -> bool {
        self.by_call.get(&key(handle)).is_some_and(|producer| {
            producer.order < (invocation.statement_index, invocation.call_ordinal)
                && self.common.count(*producer) == Some(1)
        })
    }
}

pub(super) struct LocalProducers {
    owners: Owners,
}

impl LocalProducers {
    fn owns(&self, prepared: &DispatchProducers, producer: Producer) -> bool {
        prepared
            .common
            .count(producer)
            .zip(self.owners.count(producer))
            .is_some_and(|(common, local)| common + local == 1)
    }

    pub(super) fn rescheduled(
        &self,
        prepared: &DispatchProducers,
        prefix: &PrefixProducers,
        operands: &[Handle<FlowCallFact>],
        before: CallOrder,
    ) -> bool {
        if !self.owners.invalid {
            for (statement, candidates) in &prefix.common {
                if *statement > before.0 {
                    break;
                }
                // An unknown operand owner invalidates every other producer at
                // this statement. Its own producer already has a local owner,
                // which would duplicate the one required by this common set.
                if self.owners.unknown_operands.contains_key(statement) {
                    continue;
                }
                for (ordinal, producer) in candidates {
                    if (*statement, *ordinal) >= before {
                        break;
                    }
                    if self.owners.counts.get(producer).copied().unwrap_or(0) == 0 {
                        return true;
                    }
                    // Every skipped candidate has a new local owner, so this
                    // walk is bounded by the new work rather than prefix size.
                }
            }
        }
        // A producer absent from the common schedule can be owned by this
        // leaf's preparation. Only locally touched identities need this join.
        for producer in self
            .owners
            .counts
            .keys()
            .filter(|key| prefix.candidates.contains(key))
        {
            if let Some(order) = prepared.orders.get(producer).copied()
                && order < before
                && self.owns(
                    prepared,
                    Producer {
                        key: *producer,
                        order,
                    },
                )
            {
                return true;
            }
        }
        operands.iter().any(|handle| {
            prepared
                .by_call
                .get(&key(*handle))
                .is_some_and(|producer| producer.order < before && self.owns(prepared, *producer))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_owner_index_matches_complete_join() {
        let first = (11, 3);
        let second = (12, 3);
        let third = (13, 4);
        let orders = BTreeMap::from([(first, (1, 1)), (second, (1, 2)), (third, (2, 1))]);
        let prepared = DispatchProducers {
            by_call: BTreeMap::new(),
            orders,
            coordinates: BTreeMap::new(),
            common: Owners {
                counts: BTreeMap::from([(first, 1), (third, 1)]),
                ..Default::default()
            },
            predecessor_count: 0,
        };
        let prefix = PrefixProducers {
            candidates: BTreeSet::from([first, second, third]),
            common: BTreeMap::from([
                (1, BTreeSet::from([(1, first)])),
                (2, BTreeSet::from([(1, third)])),
            ]),
        };
        let locals = [
            Owners::default(),
            Owners {
                counts: BTreeMap::from([(second, 1)]),
                ..Default::default()
            },
            Owners {
                counts: BTreeMap::from([(first, 1), (third, 1)]),
                ..Default::default()
            },
            Owners {
                counts: BTreeMap::from([(first, 1)]),
                unknown_operands: BTreeMap::from([(1, BTreeSet::from([first]))]),
                ..Default::default()
            },
            Owners {
                counts: BTreeMap::from([(second, 1)]),
                unknown_operands: BTreeMap::from([(1, BTreeSet::from([first, second]))]),
                ..Default::default()
            },
            Owners {
                invalid: true,
                ..Default::default()
            },
        ];
        for owners in locals {
            let local = LocalProducers { owners };
            for before in [(1, 1), (1, 3), (2, 1), (3, 0)] {
                let linear = prefix.candidates.iter().any(|key| {
                    let order = prepared.orders[key];
                    order < before && local.owns(&prepared, Producer { key: *key, order })
                });
                assert_eq!(local.rescheduled(&prepared, &prefix, &[], before), linear);
            }
        }
    }
}
