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

impl RangeFacts<'_> {
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
        left.root == right.root
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
