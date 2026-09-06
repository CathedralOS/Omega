//! Exact typed reads behind the range checker's legacy expression labels.

#[cfg(test)]
mod tests;

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
                        || other.reads != row.reads)
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

fn collect_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Binary(binary) => {
            collect_reads(
                program,
                machine,
                state,
                statement_index,
                binary.left,
                reads,
                depth + 1,
            ) && collect_reads(
                program,
                machine,
                state,
                statement_index,
                binary.right,
                reads,
                depth + 1,
            )
        }
        ExpressionNode::Unary(unary) => collect_reads(
            program,
            machine,
            state,
            statement_index,
            unary.operand,
            reads,
            depth + 1,
        ),
        ExpressionNode::Cast(cast) => collect_reads(
            program,
            machine,
            state,
            statement_index,
            cast.value,
            reads,
            depth + 1,
        ),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            if !has_resolved_read_identity(program, expression, 0) {
                return false;
            }
            let Some(mut place) = canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            ) else {
                return false;
            };
            let facts::PlaceRoot::Symbol(root) = place.root else {
                return false;
            };
            if !root.is_valid()
                || place
                    .segments
                    .iter()
                    .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment))
            {
                return false;
            }
            crate::flow::normalize_attached_place_root(
                program,
                machine.symbol,
                state.symbol,
                &mut place,
            );
            let root_is_current = place.root == facts::PlaceRoot::Symbol(machine.symbol)
                || program.state_parameters(state).iter().any(|parameter| parameter.symbol == root)
                || program.statement_table.statements(state.statement_nodes).iter().take(statement_index).any(|statement| {
                    matches!(statement, typed_trees::statement::StatementNode::LocalData(local) if local.symbol == root)
                });
            if !root_is_current
                || place.segments.iter().any(|segment| {
                    !matches!(
                        segment,
                        facts::PlaceSegment::Field { .. } | facts::PlaceSegment::Case { .. }
                    )
                })
            {
                return false;
            }
            if !reads.contains(&place) {
                reads.push(place);
            }
            true
        }
        // Calls can read implicit storage; atomic and indexed operands need
        // their own complete read/selector evidence, not an argument-only scan.
        _ => false,
    }
}

/// Contextual place lookup can recover a name for ordinary source analysis.
/// Preservation needs the typed identity carriers before that recovery runs.
fn has_resolved_read_identity(
    program: &TypedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            crate::lookup::first_valid_name_path_symbol(path, &program.expression_table).is_some()
                && (members.len() <= 1 || members.len() == symbols.len())
                && symbols.iter().all(|symbol| symbol.is_valid())
        }
        ExpressionNode::Member(member) => {
            member.member_symbol.is_valid()
                && has_resolved_read_identity(program, member.receiver, depth + 1)
        }
        _ => false,
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
