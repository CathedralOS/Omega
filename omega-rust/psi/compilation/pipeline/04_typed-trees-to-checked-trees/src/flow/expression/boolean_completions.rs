//! A Boolean completion retains its possible evaluation continuations.
//! An enclosing selective operand consumes the selected continuation before
//! evaluating its right child; ordinary value completion still meets both.
//! These are evaluation-local scratch handles, not expression-keyed facts or
//! permission to rerun a call. Each continuation retains the exact captured
//! descriptor inventory, constraints and write prefix of its actual route.

use super::{EvaluatedWritePrefix, Execution};
use arena::{Handle, HandleSpan};
use checked_trees::{FlowConstraintRef, FlowOperandReferent, FlowSemanticContextRef};
use typed_trees::expression::{BinaryOperator, TableBinaryExpression};
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
struct BooleanContinuation {
    writes: EvaluatedWritePrefix,
    contexts: HandleSpan<FlowSemanticContextRef>,
    constraints: HandleSpan<FlowConstraintRef>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct BooleanCompletions {
    when_true: Option<BooleanContinuation>,
    when_false: Option<BooleanContinuation>,
}

impl BooleanCompletions {
    fn selected(self, value: bool) -> Option<BooleanContinuation> {
        if value {
            self.when_true
        } else {
            self.when_false
        }
    }

    fn known_value(self) -> Option<bool> {
        match (self.when_true.is_some(), self.when_false.is_some()) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }
}

impl Execution<'_, '_, '_> {
    pub(super) fn negated_boolean_completion(
        &mut self,
        completion: Handle<BooleanCompletions>,
    ) -> Handle<BooleanCompletions> {
        if !completion.is_valid() {
            return completion;
        }
        let original = *self.boolean_completions.get(completion);
        self.boolean_completions.insert(BooleanCompletions {
            when_true: original.when_false,
            when_false: original.when_true,
        })
    }

    fn boolean_paths(
        &self,
        completion: Handle<BooleanCompletions>,
        known: Option<bool>,
        contexts: HandleSpan<FlowSemanticContextRef>,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> BooleanCompletions {
        if completion.is_valid() {
            let mut completed = *self.boolean_completions.get(completion);
            if known == Some(true) {
                completed.when_false = None;
            }
            if known == Some(false) {
                completed.when_true = None;
            }
            return completed;
        }
        let continuation = BooleanContinuation {
            writes: self.write_prefix(),
            contexts,
            constraints,
        };
        BooleanCompletions {
            when_true: (known != Some(false)).then_some(continuation),
            when_false: (known != Some(true)).then_some(continuation),
        }
    }

    fn select_boolean_path(
        &mut self,
        continuation: BooleanContinuation,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        // Keep the append-only mutation timeline: an enclosing value must
        // still invalidate anything any feasible left outcome could write.
        // Selection narrows the current prefix, not that enclosing may-write
        // inventory or any already emitted call row.
        self.storage_snapshot = continuation.writes.storage_snapshot;
        self.entry_origin_writes = continuation.writes.writes;
        self.entry_origin_unknown_write = continuation.writes.unknown;
        self.entry_binding_writes = continuation.writes.binding_writes;
        self.entry_binding_unknown_write = continuation.writes.binding_unknown;
        *contexts = continuation.contexts;
        *constraints = continuation.constraints;
    }

    fn join_mutation_places(
        &mut self,
        first: HandleSpan<FlowOperandReferent>,
        second: HandleSpan<FlowOperandReferent>,
    ) -> HandleSpan<FlowOperandReferent> {
        if first == second {
            return first;
        }
        let mut places = self
            .context
            .control
            .operand_origin_writes
            .span_or_empty(first)
            .to_vec();
        for place in self
            .context
            .control
            .operand_origin_writes
            .span_or_empty(second)
        {
            if !places.contains(place) {
                places.push(*place);
            }
        }
        self.context
            .control
            .operand_origin_writes
            .insert_many(places)
    }

    fn join_boolean_paths(
        &mut self,
        first: Option<BooleanContinuation>,
        second: Option<BooleanContinuation>,
    ) -> Option<BooleanContinuation> {
        let (first, second) = match (first, second) {
            (Some(first), Some(second)) => (first, second),
            (one, None) | (None, one) => return one,
        };
        self.join_storage_prefixes(&[first.writes, second.writes]);
        let mut contexts = second.contexts;
        let mut constraints = second.constraints;
        self.meet(
            first.contexts,
            first.constraints,
            &mut contexts,
            &mut constraints,
        );
        Some(BooleanContinuation {
            writes: EvaluatedWritePrefix {
                length: self.operand_writes.len(),
                storage_snapshot: self.storage_snapshot,
                writes: self.join_mutation_places(first.writes.writes, second.writes.writes),
                unknown: first.writes.unknown || second.writes.unknown,
                binding_writes: self.join_mutation_places(
                    first.writes.binding_writes,
                    second.writes.binding_writes,
                ),
                binding_unknown: first.writes.binding_unknown || second.writes.binding_unknown,
            },
            contexts,
            constraints,
        })
    }

    pub(super) fn logical_expression(
        &mut self,
        binary: &TableBinaryExpression,
        known_left: Option<bool>,
        left_completion: Handle<BooleanCompletions>,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> (Option<bool>, Handle<BooleanCompletions>) {
        let evaluate_when = binary.operator == BinaryOperator::And;
        let left = self.boolean_paths(left_completion, known_left, *contexts, *constraints);
        let skipped = left.selected(!evaluate_when);
        let Some(selected) = left.selected(evaluate_when) else {
            return (left.known_value(), self.boolean_completions.insert(left));
        };
        self.select_boolean_path(selected, contexts, constraints);
        super::super::exits::append_predicate_context(
            self.program,
            self.semantic,
            self.context,
            self.state.symbol,
            self.statement_index,
            crate::flow::GuardObservation::evaluated(
                binary.left,
                evaluate_when,
                self.statement_index,
            ),
            facts::ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
            },
            selected.writes.writes,
            selected.writes.unknown,
            contexts,
            constraints,
        );
        let (known_right, completed) = self.expression_result(
            binary.right,
            TypeReferenceHandle::invalid(),
            contexts,
            constraints,
        );
        let right = self.boolean_paths(
            completed.boolean_completion,
            known_right,
            *contexts,
            *constraints,
        );
        let completions = if evaluate_when {
            BooleanCompletions {
                when_true: right.when_true,
                when_false: self.join_boolean_paths(skipped, right.when_false),
            }
        } else {
            BooleanCompletions {
                when_true: self.join_boolean_paths(skipped, right.when_true),
                when_false: right.when_false,
            }
        };
        if let Some(joined) = self.join_boolean_paths(completions.when_true, completions.when_false)
        {
            self.select_boolean_path(joined, contexts, constraints);
        }
        (
            completions.known_value(),
            self.boolean_completions.insert(completions),
        )
    }
}
