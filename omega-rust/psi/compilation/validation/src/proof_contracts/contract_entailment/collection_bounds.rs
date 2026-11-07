//! Collection geometry consumes the ordinary ordered normal-return owner.
//!
//! A call requirement and an index must agree on the same completed values.
//! The previous arithmetic prefix walk lost nested result guarantees and Match
//! alternatives. Reuse the structural prefix, operand scheduler and integer
//! judge already used by call requirements and exact casts, rather than building
//! another call-result graph or admitting captured values as live storage.

mod geometry;
use super::structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
pub(super) use geometry::proves_observed_bounds;
use std::collections::BTreeMap;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

type BoundsCoordinate = ((u32, u32), usize, (u32, u32));

/// Declaration-bound context for exact collection-operation observations.
/// Each query observes the operation after its actual children, before its
/// result or any later sibling can contribute a guarantee.
pub struct CollectionBoundsObservations<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    states: Vec<&'program State>,
    acyclic: std::cell::Cell<Option<bool>>,
    demands: std::cell::RefCell<BTreeMap<BoundsCoordinate, BoundsDemand>>,
    resolved: std::cell::Cell<bool>,
}

struct BoundsDemand {
    state: symbols::SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    proven: bool,
}

impl BoundsDemand {
    fn coordinate(&self) -> BoundsCoordinate {
        (
            (self.state.arena_index(), self.state.generation()),
            self.statement,
            checked_trees::operators::handle_position(self.expression),
        )
    }
}

impl<'program> CollectionBoundsObservations<'program> {
    pub fn new(program: &'program dyn crate::CheckingInput, machine: &'program Machine) -> Self {
        let mut states = program.machine_states(machine).iter().collect::<Vec<_>>();
        states.sort_by_key(|state| (state.symbol.arena_index(), state.symbol.generation()));
        Self {
            program,
            machine,
            states,
            acyclic: std::cell::Cell::new(None),
            demands: std::cell::RefCell::new(BTreeMap::new()),
            resolved: std::cell::Cell::new(false),
        }
    }

    /// The inexpensive range walk records only obligations it could not close.
    /// Until resolution, false is provisional and cannot publish a proof.
    pub fn proves(
        &self,
        state: &State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> bool {
        let mut demand = BoundsDemand {
            state: state.symbol,
            statement: statement_index,
            expression,
            proven: false,
        };
        let coordinate = demand.coordinate();
        let mut demands = self.demands.borrow_mut();
        if let Some(demand) = demands.get(&coordinate) {
            return demand.proven;
        }
        // A replayed request absent from the collected roster still gets the
        // original judgment. Batching cannot turn a missed demand into a new
        // refusal or an assumed proof.
        let proven = if self.resolved.get() {
            drop(demands);
            let proven = self.prove_one(state.symbol, statement_index, expression);
            demands = self.demands.borrow_mut();
            proven
        } else {
            false
        };
        demand.proven = proven;
        demands.insert(coordinate, demand);
        proven
    }

    /// Resolve actual deferred demands together. A successful result causes the
    /// caller to replay its cheap range walk from the same initial facts; failed
    /// batches retain the original single-occurrence fallback below.
    pub fn resolve_deferred(&self) -> bool {
        self.resolved.set(true);
        let mut demands = std::mem::take(&mut *self.demands.borrow_mut())
            .into_values()
            .collect::<Vec<_>>();
        if demands.is_empty() {
            return false;
        }
        let Some(mut queries) = demands
            .iter()
            .map(|demand| self.query(demand.state, demand.statement, demand.expression))
            .collect::<Option<Vec<_>>>()
        else {
            self.retain_demands(demands);
            return false;
        };
        let recognized = if self.is_acyclic() {
            let mut visitors = queries
                .iter_mut()
                .map(|query| query as &mut dyn StructuralPrefixVisitor)
                .collect::<Vec<_>>();
            let recognized = super::structural_prefix::observe_acyclic_invocation_prefixes(
                self.program,
                self.machine,
                &mut visitors,
            )
            .is_some();
            vec![recognized; visitors.len()]
        } else {
            let mut recognized = vec![false; queries.len()];
            // Coordinates order by complete state identity. Form the actual
            // recurrent groups once, rather than scanning all demands for
            // every state declaration.
            let mut offset = 0;
            for group in queries.chunk_by_mut(|left, right| left.site.state == right.site.state) {
                let length = group.len();
                let Some(state) = self.state(group[0].site.state) else {
                    offset += length;
                    continue;
                };
                let mut visitors = group
                    .iter_mut()
                    .map(|query| query as &mut dyn StructuralPrefixVisitor)
                    .collect::<Vec<_>>();
                let state_results = super::structural_prefix::observe_runtime_state_prefixes(
                    self.program,
                    self.machine,
                    state,
                    &mut visitors,
                );
                recognized[offset..offset + length].copy_from_slice(&state_results);
                offset += length;
            }
            recognized
        };
        for ((demand, query), recognized) in demands.iter_mut().zip(&queries).zip(recognized) {
            demand.proven = recognized && query.seen && query.proven;
        }
        let pending = demands
            .iter()
            .enumerate()
            .filter_map(|(index, demand)| (!demand.proven).then_some(index))
            .collect::<Vec<_>>();
        if !pending.is_empty() {
            let accepted = {
                let mut members = queries
                    .iter_mut()
                    .zip(&demands)
                    .filter_map(|(query, demand)| {
                        if demand.proven {
                            return None;
                        }
                        query.seen = false;
                        query.proven = true;
                        Some(query)
                    })
                    .collect::<Vec<_>>();
                super::structural_prefix::observe_private_arrivals_batch(
                    self.program,
                    self.machine,
                    &mut members,
                    false,
                )
                .into_iter()
                .zip(members)
                .map(|(recognized, query)| recognized == Some(true) && query.seen && query.proven)
                .collect::<Vec<_>>()
            };
            for (index, accepted) in pending.into_iter().zip(accepted) {
                demands[index].proven = accepted;
            }
        }
        for demand in &mut demands {
            if !demand.proven {
                demand.proven = self.prove_one(demand.state, demand.statement, demand.expression);
            }
        }
        let proven = demands.iter().any(|demand| demand.proven);
        self.retain_demands(demands);
        proven
    }

    fn retain_demands(&self, demands: Vec<BoundsDemand>) {
        *self.demands.borrow_mut() = demands
            .into_iter()
            .map(|demand| (demand.coordinate(), demand))
            .collect();
    }

    fn state(&self, symbol: symbols::SymbolHandle) -> Option<&'program State> {
        self.states
            .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |state| {
                (state.symbol.arena_index(), state.symbol.generation())
            })
            .ok()
            .map(|index| self.states[index])
    }

    fn query(
        &self,
        state: symbols::SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<Bounds> {
        let state = self.state(state)?;
        Some(Bounds {
            site: StructuralPrefixSite {
                state: state.symbol,
                statement_index,
                in_transition_target: super::structural_prefix::expression_is_transition_target(
                    self.program,
                    state,
                    statement_index,
                    expression,
                ),
            },
            expression,
            seen: false,
            proven: true,
        })
    }

    fn is_acyclic(&self) -> bool {
        self.acyclic.get().unwrap_or_else(|| {
            let computed = self
                .program
                .machine_states(self.machine)
                .first()
                .is_some_and(|entry| {
                    crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
                        self.program,
                        self.machine,
                        entry,
                    )
                });
            self.acyclic.set(Some(computed));
            computed
        })
    }

    fn prove_one(
        &self,
        state: symbols::SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(mut query) = self.query(state, statement_index, expression) else {
            return false;
        };
        let recognized = if self.is_acyclic() {
            super::structural_prefix::observe_acyclic_invocation_prefix(
                self.program,
                self.machine,
                &mut query,
            )
        } else {
            let Some(state) = self.state(state) else {
                return false;
            };
            super::structural_prefix::observe_runtime_state_prefix(
                self.program,
                self.machine,
                state,
                &mut query,
            )
        };
        if recognized.is_some() && query.seen && query.proven {
            return true;
        }
        // A private helper's exact descriptor and index arrive together under
        // its callers' guards. Observe the same closed incoming frames as
        // operand formation; lexical ancestry alone supplies no bound, and
        // any unresolved or unproved arrival keeps the operation inadmissible.
        query.seen = false;
        query.proven = true;
        super::structural_prefix::observe_private_arrivals(
            self.program,
            self.machine,
            &mut query,
            false,
        ) == Some(true)
            && query.seen
            && query.proven
    }
}

#[derive(Clone)]
struct Bounds {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for Bounds {
    fn has_positive_proof(&self) -> bool {
        self.seen && self.proven
    }

    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, judge, environment) = observation;
        let Some(indexed) = program.expression_table.indexed_node(self.expression) else {
            self.proven = false;
            return;
        };
        if !crate::place_has_builtin_coordinates(program, machine, Some(state), self.expression) {
            self.proven = false;
            return;
        }
        let Some(reference) =
            crate::expression_result_type_reference(program, machine, state, indexed.collection)
        else {
            self.proven = false;
            return;
        };
        let array_extent = if crate::fixed_array_type_extent(program, reference).is_some() {
            match crate::resolved_array_extent_at(
                program,
                state,
                self.site.statement_index,
                indexed.collection,
            ) {
                Ok(extent) => Some(extent),
                Err(reason) => {
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT collection-extent machine={} reason={reason}",
                            machine.name
                        );
                    }
                    self.proven = false;
                    return;
                }
            }
        } else {
            None
        };
        let Some(entries) = judge.normal_operation_entries(
            self.site.statement_index,
            self.expression,
            super::structural_judgment::normal_returns::OperationOperandDemand::All,
            environment,
            (machine, state, state.return_type),
        ) else {
            // The operation query deliberately treats no reached operation as
            // unavailable. The expression cut distinguishes a completely
            // explored unreachable occurrence from an incomplete observation;
            // only that independent result can make bounds vacuous.
            self.proven &= judge
                .normal_expression_entries(
                    self.site.statement_index,
                    self.expression,
                    environment,
                    (machine, state, state.return_type),
                )
                .is_some_and(|entries| entries.is_empty());
            return;
        };
        self.proven &= !entries.is_empty();
        for entry in entries {
            if super::struct_trace() {
                eprintln!(
                    "STRUCT collection-operands machine={} values={:?}",
                    machine.name, entry.arguments
                );
            }
            let Some(collection) = entry.arguments.first().and_then(Option::as_ref) else {
                self.proven = false;
                return;
            };
            let mut position = 1;
            self.proven &= proves_observed_bounds(
                program,
                &entry.judge,
                indexed,
                reference,
                array_extent.as_ref(),
                collection.clone(),
                |source| {
                    let value = entry.arguments.get(position)?.as_ref()?.clone();
                    position += 1;
                    Some((
                        value,
                        crate::expression_result_type_reference(program, machine, state, source),
                    ))
                },
            ) && position == entry.arguments.len();
        }
    }
}
