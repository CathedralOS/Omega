//! Reanchor live facts at a binding using resolved storage coordinates.
//!
//! The alias captures its source now. Future uses supply only typed destination
//! coordinates, never facts or replayed initializers. Rendered labels remain
//! keys for the legacy fact tables, not evidence that two places agree.
//! A selector temporary can still equal its original observation at capture;
//! only the recorded, write-invalidated scalar relation establishes that join.
//! Once reanchored, the reference depends on its own captured descriptor and
//! descendant selectors, not the variables that selected its original root.

use super::{ExpressionDependencies, collect_reads, reads};
use crate::checks::ranges::RangeFacts;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::{machine::Machine, state::State};

impl RangeFacts<'_> {
    pub(in crate::checks::ranges) fn alias_place_facts(
        &mut self,
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        state: &State,
        captured: ExpressionHandle,
        symbol: SymbolHandle,
    ) {
        if !symbol.is_valid()
            || !validation::place_has_builtin_coordinates(program, machine, Some(state), captured)
        {
            return;
        }
        // A builtin whole view preserves the source coordinates. Resolve that
        // geometry at capture, before adjoining future destination projections;
        // a range selector is not an additional element dimension.
        let captured =
            validation::whole_collection_view_source(program, machine, Some(state), captured)
                .unwrap_or(captured);
        let Some(source) = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            self.statement_index,
            captured,
        )
        .and_then(|place| {
            crate::flow::rebase_exact_local_place(
                program,
                state.symbol,
                self.statement_index,
                place,
                self.checked_calls.and_then(|context| context.call_frames()),
            )
        }) else {
            return;
        };
        if !matches!(source.root, facts::PlaceRoot::Symbol(root) if root.is_valid()) {
            return;
        }
        let transferable = self.preserved_expression_labels(program, machine, state, Some(&[]));
        let originals = self.expression_dependencies.clone();
        let prefix = self.statement_index.saturating_add(1);
        let bound_lookup = self.bound_lookup();
        for (expression, _) in program.expression_table.iter_expressions() {
            let Some(candidate) = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                prefix,
                expression,
            ) else {
                continue;
            };
            if candidate.root != facts::PlaceRoot::Symbol(symbol)
                || !validation::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
            {
                continue;
            }
            let Some(rebased) =
                crate::flow::project_captured_view_place(source.clone(), &candidate.segments)
            else {
                continue;
            };
            for original in &originals {
                if !transferable.contains(&original.label) {
                    continue;
                }
                // Extent facts name the descriptor, not its `.len` projection.
                let subject = validation::collection_length_receiver(
                    program,
                    machine,
                    Some(state),
                    original.expression,
                )
                .unwrap_or(original.expression);
                let Some(original_place) = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    self.statement_index,
                    subject,
                )
                .and_then(|place| {
                    crate::flow::rebase_exact_local_place(
                        program,
                        state.symbol,
                        self.statement_index,
                        place,
                        self.checked_calls.and_then(|context| context.call_frames()),
                    )
                }) else {
                    continue;
                };
                let mut uses_frozen_selector = false;
                let same_place = original_place.root == rebased.root
                    && original_place.segments.len() == rebased.segments.len()
                    && original_place.segments.iter().zip(&rebased.segments).all(
                        |(left, right)| {
                            if let (
                                facts::PlaceSegment::Index { expression: left },
                                facts::PlaceSegment::Index { expression: right },
                            ) = (left, right)
                            {
                                let left =
                                    self.current_scalar_observation(program, machine, state, *left);
                                let right = self
                                    .current_scalar_observation(program, machine, state, *right);
                                if program
                                    .expression_table
                                    .expressions_structurally_equal(left, right)
                                {
                                    return true;
                                }
                                let same_value = super::captures::integer_value_identity(
                                    program,
                                    bound_lookup,
                                    state,
                                    left,
                                )
                                .is_some_and(|value| {
                                    super::captures::integer_value_identity(
                                        program,
                                        bound_lookup,
                                        state,
                                        right,
                                    ) == Some(value)
                                });
                                uses_frozen_selector |= same_value;
                                same_value
                            } else {
                                crate::flow::canonical_place_segments_equal(program, *left, *right)
                            }
                        },
                    );
                if !same_place {
                    continue;
                }
                let mut dependencies = Vec::new();
                // Typed uses may spell an immutable selector copy introduced
                // later. Read through its shared frozen-value identity, then
                // require all resulting roots to exist at this capture. A
                // future mutable read or new computed value cannot pass that
                // currency check merely because its declaration exists.
                if !collect_reads(
                    program,
                    machine,
                    state,
                    if uses_frozen_selector {
                        program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len()
                    } else {
                        prefix
                    },
                    expression,
                    self.checked_calls,
                    self.checked_operators,
                    &mut dependencies,
                    0,
                    bound_lookup,
                ) || !dependencies
                    .iter()
                    .all(|read| reads::root_is_current(program, machine, state, prefix, read.root))
                {
                    continue;
                }
                let source_label = program.expression_table.display_name(subject);
                let label = program.expression_table.display_name(expression);
                self.prove_window_parent(label.clone(), source_label.clone(), None);
                self.alias_collection(&source_label, &label);
                self.alias_index(&source_label, &label);
                if !self.expression_dependencies.iter().any(|row| {
                    row.expression == expression
                        && row.machine == machine.symbol
                        && row.state == state.symbol
                }) {
                    self.expression_dependencies.push(ExpressionDependencies {
                        expression,
                        label,
                        machine: machine.symbol,
                        state: state.symbol,
                        reads: Some(dependencies),
                        descriptor: original.descriptor.as_ref().map(|_| candidate.clone()),
                    });
                }
            }
        }
    }

    /// Follow only recorded, still-live equalities. This never replays an
    /// initializer after writes or guesses a selector from its rendered name.
    fn current_scalar_observation(
        &self,
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        state: &State,
        mut expression: ExpressionHandle,
    ) -> ExpressionHandle {
        for _ in 0..super::EXPRESSION_WALK_DEPTH_BOUND {
            let Some(symbol) = validation::single_segment_name_symbol(program, expression) else {
                break;
            };
            let Some(value) = self.scalar_binding_dependencies.iter().find_map(|row| {
                (row.machine == machine.symbol
                    && row.state == state.symbol
                    && row.place.root == facts::PlaceRoot::Symbol(symbol)
                    && row.place.segments.is_empty())
                .then_some(row.value.as_ref())
                .flatten()
            }) else {
                break;
            };
            expression = value.expression;
        }
        expression
    }
}
