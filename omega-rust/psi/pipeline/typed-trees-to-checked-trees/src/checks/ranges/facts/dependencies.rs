//! Exact typed reads behind the range checker's legacy expression labels.

mod captures;
mod reads;
#[cfg(test)]
mod tests;

use reads::collect_reads;

use super::RangeFacts;
use crate::flow::{CanonicalPlace, canonical_place_from_expression_in_state};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::{TypedTrees, machine::Machine, state::State};

#[derive(Clone)]
pub(super) struct ExpressionDependencies {
    expression: ExpressionHandle,
    label: String,
    machine: SymbolHandle,
    state: SymbolHandle,
    /// None is an incomplete read set, not a storage-free computation.
    reads: Option<Vec<CanonicalPlace>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::checks::ranges) struct ReceiverLength {
    expression: ExpressionHandle,
    label: String,
    place: CanonicalPlace,
    length: i64,
}

impl ReceiverLength {
    pub(in crate::checks::ranges) fn same_extent(&self, other: &Self) -> bool {
        self.place == other.place && self.length == other.length
    }
}

impl RangeFacts<'_> {
    pub(in crate::checks::ranges) fn forget_collection_expression(&mut self, label: &str) {
        self.forget_collection_facts(label);
        // A later state or binding can reuse the display label for different
        // storage. New extent evidence must not attach to its old typed reads.
        self.expression_dependencies
            .retain(|row| row.label != label);
    }

    pub(in crate::checks::ranges) fn expression_exact_length(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        if self.exact_lengths.is_empty() {
            return None;
        }
        let mut reads = Vec::new();
        if !collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            expression,
            &mut reads,
            0,
        ) || reads.len() != 1
        {
            return None;
        }
        reads[0] = crate::flow::rebase_exact_local_place(
            program,
            state.symbol,
            self.statement_index,
            reads[0].clone(),
        )?;
        let mut length = None;
        for row in &self.expression_dependencies {
            if row.machine != machine.symbol || row.state != state.symbol {
                continue;
            }
            let Some(candidate) = self.exact_length(&row.label) else {
                continue;
            };
            let row_reads = row.reads.as_ref().and_then(|row_reads| {
                let [place] = row_reads.as_slice() else {
                    return None;
                };
                crate::flow::rebase_exact_local_place(
                    program,
                    state.symbol,
                    self.statement_index,
                    place.clone(),
                )
                .map(|place| vec![place])
            });
            if row.machine == machine.symbol
                && row.state == state.symbol
                && same_reads(program, row_reads.as_deref(), Some(&reads))
            {
                if length.is_some_and(|known| known != candidate) {
                    return None;
                }
                length = Some(candidate);
            }
        }
        length
    }

    pub(in crate::checks::ranges) fn receiver_lengths(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
    ) -> Vec<ReceiverLength> {
        self.expression_dependencies
            .iter()
            .filter_map(|row| {
                let [place] = row.reads.as_deref()? else {
                    return None;
                };
                let place = crate::flow::rebase_exact_local_place(
                    program,
                    state.symbol,
                    self.statement_index,
                    place.clone(),
                )?;
                (row.machine == machine.symbol
                    && row.state == state.symbol
                    && crate::flow::normalized_event_place_root(program, place.root)
                        == facts::PlaceRoot::Symbol(machine.symbol)
                    && !place.segments.is_empty()
                    && place
                        .segments
                        .iter()
                        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. })))
                .then(|| {
                    Some(ReceiverLength {
                        expression: row.expression,
                        label: row.label.clone(),
                        place: CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(machine.symbol),
                            segments: place.segments.clone(),
                        },
                        length: self.exact_length(&row.label)?,
                    })
                })?
            })
            .collect()
    }

    pub(in crate::checks::ranges) fn seed_receiver_lengths(
        &mut self,
        machine: SymbolHandle,
        state: SymbolHandle,
        lengths: &[ReceiverLength],
    ) {
        for length in lengths {
            self.prove_exact_length(length.label.clone(), length.length);
            self.expression_dependencies.push(ExpressionDependencies {
                expression: length.expression,
                label: length.label.clone(),
                machine,
                state,
                reads: Some(vec![length.place.clone()]),
            });
        }
    }

    pub(in crate::checks::ranges) fn record_expression_dependencies(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) {
        self.record_dependencies(program, machine, state, expression, 0);
    }

    fn record_dependencies(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        depth: usize,
    ) {
        if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
            return;
        }
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
        ) {
            return;
        }
        if self.expression_dependencies.iter().any(|row| {
            row.expression == expression
                && row.machine == machine.symbol
                && row.state == state.symbol
        }) {
            return;
        }
        let mut reads = Vec::new();
        let complete = validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            expression,
        ) && collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            expression,
            &mut reads,
            0,
        );
        self.expression_dependencies.push(ExpressionDependencies {
            expression,
            label: program.expression_table.display_name(expression),
            machine: machine.symbol,
            state: state.symbol,
            reads: complete.then_some(reads),
        });
        let children = match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => [binary.left, binary.right],
            ExpressionNode::Unary(unary) => [unary.operand, ExpressionHandle::invalid()],
            ExpressionNode::Cast(cast) => [cast.value, ExpressionHandle::invalid()],
            _ => return,
        };
        for child in children {
            self.record_dependencies(program, machine, state, child, depth + 1);
        }
    }

    pub(super) fn preserved_expression_labels(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        writes: Option<&[CanonicalPlace]>,
    ) -> Vec<String> {
        let Some(writes) = writes else {
            return Vec::new();
        };
        let mut writes = writes.to_vec();
        for write in &mut writes {
            crate::flow::normalize_attached_place_root(
                program,
                machine.symbol,
                state.symbol,
                write,
            );
            if !matches!(write.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid() && program.symbols.get(symbol).kind != symbols::SymbolKind::Field)
            {
                return Vec::new();
            }
        }
        let mut preserved = Vec::new();
        for row in &self.expression_dependencies {
            if !row.reads.as_ref().is_some_and(|reads| {
                row.machine == machine.symbol
                    && row.state == state.symbol
                    && program.expression_table.expression_is_valid(row.expression)
                    && reads.iter().all(|read| {
                        writes
                            .iter()
                            .all(|write| !places_overlap(program, read, write))
                    })
            }) {
                continue;
            }
            // Equal display text is only a lookup key for the old fact tables.
            // It cannot select one of several incompatible typed meanings.
            if self.expression_dependencies.iter().any(|other| {
                other.label == row.label
                    && (other.machine != row.machine
                        || other.state != row.state
                        || !same_reads(program, other.reads.as_deref(), row.reads.as_deref()))
            }) {
                continue;
            }
            if !preserved.contains(&row.label) {
                preserved.push(row.label.clone());
            }
        }
        preserved
    }

    pub(super) fn affected_expression_labels(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        writes: Option<&[CanonicalPlace]>,
    ) -> Vec<String> {
        let preserved = self.preserved_expression_labels(program, machine, state, writes);
        self.expression_dependencies
            .iter()
            .filter(|row| {
                row.machine == machine.symbol
                    && row.state == state.symbol
                    && !preserved.contains(&row.label)
            })
            .map(|row| row.label.clone())
            .collect()
    }

    pub(in crate::checks::ranges) fn expression_is_disjoint_from_writes(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        writes: Option<&[CanonicalPlace]>,
    ) -> bool {
        self.preserved_expression_labels(program, machine, state, writes)
            .contains(&program.expression_table.display_name(expression))
    }
}

fn places_overlap(program: &TypedTrees, left: &CanonicalPlace, right: &CanonicalPlace) -> bool {
    crate::flow::normalized_event_place_root(program, left.root)
        == crate::flow::normalized_event_place_root(program, right.root)
        && crate::flow::canonical_place_segments_may_overlap(
            program,
            &left.segments,
            &right.segments,
        )
}

fn same_reads(
    program: &TypedTrees,
    left: Option<&[CanonicalPlace]>,
    right: Option<&[CanonicalPlace]>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let equal = |left: &CanonicalPlace, right: &CanonicalPlace| {
        crate::flow::normalized_event_place_root(program, left.root)
            == crate::flow::normalized_event_place_root(program, right.root)
            && left.segments.len() == right.segments.len()
            && left
                .segments
                .iter()
                .zip(&right.segments)
                .all(|(left, right)| match (*left, *right) {
                    (
                        facts::PlaceSegment::Index { expression: left },
                        facts::PlaceSegment::Index { expression: right },
                    ) => program
                        .expression_table
                        .expressions_structurally_equal(left, right),
                    (left, right) => crate::flow::canonical_place_segments_equal(left, right),
                })
    };
    // Separate guard occurrences may copy the same typed selector tree into
    // different arena slots. Compare their meaning, not those allocation slots.
    left.iter()
        .all(|read| right.iter().any(|other| equal(read, other)))
        && right
            .iter()
            .all(|read| left.iter().any(|other| equal(read, other)))
}
