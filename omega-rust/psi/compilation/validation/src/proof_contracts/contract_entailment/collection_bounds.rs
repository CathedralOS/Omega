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
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Declaration-bound context for exact collection-operation observations.
/// Each query observes the operation after its actual children, before its
/// result or any later sibling can contribute a guarantee.
pub struct CollectionBoundsObservations<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    acyclic: std::cell::Cell<Option<bool>>,
}

impl<'program> CollectionBoundsObservations<'program> {
    pub fn new(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
    ) -> Self {
        Self {
            program,
            machine,
            state,
            acyclic: std::cell::Cell::new(None),
        }
    }

    pub fn proves(&self, statement_index: usize, expression: ExpressionHandle) -> bool {
        let mut query = Bounds {
            site: StructuralPrefixSite {
                state: self.state.symbol,
                statement_index,
                in_transition_target: super::structural_prefix::expression_is_transition_target(
                    self.program,
                    self.state,
                    statement_index,
                    expression,
                ),
            },
            expression,
            seen: false,
            proven: true,
        };
        let acyclic = self.acyclic.get().unwrap_or_else(|| {
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
        });
        let recognized = if acyclic {
            super::structural_prefix::observe_acyclic_invocation_prefix(
                self.program,
                self.machine,
                &mut query,
            )
        } else {
            super::structural_prefix::observe_runtime_state_prefix(
                self.program,
                self.machine,
                self.state,
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
        let Some(entries) = judge.normal_operation_entries(
            self.site.statement_index,
            self.expression,
            super::structural_judgment::normal_returns::OperationOperandDemand::All,
            environment,
            (machine, state, state.return_type),
        ) else {
            self.proven = false;
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
