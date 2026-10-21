//! Storage-selection roles belong to the program whose expressions use them.
//!
//! Index operator selection queries these roles repeatedly during preparation
//! and checking. A program address plus table lengths cannot establish their
//! freshness: replacements can reuse an address, and an expression or store
//! can change without changing any count. The existing program-owned index
//! lifetime and exact revision tuple handle both, including frozen readers.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::machine::Machine;
use crate::statement::StatementNode;
use arena::HandleSpan;

pub(super) type PlaceOccurrenceRevision = (u64, u64, u64, u64, HandleSpan<Machine>);

impl TypedTrees {
    /// Structural storage selection, independently of collection/operator type.
    /// The operator owner still decides whether builtin geometry applies.
    pub(crate) fn index_occurrence_selects_place(&self, occurrence: ExpressionHandle) -> bool {
        self.with_place_occurrences(|places| {
            places
                .binary_search_by_key(
                    &(occurrence.arena_index(), occurrence.generation()),
                    |handle| (handle.arena_index(), handle.generation()),
                )
                .is_ok()
        })
    }

    pub(super) fn with_place_occurrences<Output>(
        &self,
        run: impl FnOnce(&[ExpressionHandle]) -> Output,
    ) -> Output {
        let revision = (
            self.expression_table.revision(),
            self.statement_table.revision(),
            self.tables.machines.revision(),
            self.tables.machine_states.revision(),
            self.roots.machines,
        );
        self.declaration_indexes.place_occurrences.with(
            revision,
            || collect_place_occurrences(self),
            |places| run(places),
        )
    }
}

/// Receivers of a field or further index and assignment destinations select
/// storage rather than copying the indexed element. Ordering is lookup-only;
/// neither query nor publication order depends on the index's internal rows.
fn collect_place_occurrences(program: &TypedTrees) -> Vec<ExpressionHandle> {
    let mut places = Vec::new();
    for (_, node) in program.expression_table.iter_expressions() {
        match node {
            ExpressionNode::Member(member) => places.push(member.receiver),
            ExpressionNode::Indexed(indexed) => places.push(indexed.collection),
            _ => {}
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Assignment(assignment) = statement {
                    places.push(assignment.target);
                }
            }
        }
    }
    places.retain(|handle| handle.is_valid());
    places.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    places.dedup();
    places
}
