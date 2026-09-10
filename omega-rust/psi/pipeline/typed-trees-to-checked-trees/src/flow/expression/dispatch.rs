//! Match effects fork after each comparison and meet at the result join.
//! A saved subject is not a promise about its storage after a pattern writes it;
//! only unchanged stable inputs can publish branch-local comparison evidence.

use super::*;
use typed_trees::expression::{MatchPattern, TableMatchExpression};

impl Execution<'_, '_, '_> {
    pub(super) fn dispatch(
        &mut self,
        expression: ExpressionHandle,
        dispatch: TableMatchExpression,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let subject_capture_writes = self.operand_writes.len();
        self.expression(dispatch.subject, contexts, constraints);
        let subject_operand = self.capture_operator_operand(
            dispatch.subject,
            subject_capture_writes,
            *contexts,
            *constraints,
        );
        let subject_writes = self.operand_writes.len();
        let mut joined = None;
        let mut completed_writes = Vec::new();
        let mut boolean_coverage = [false; 2];
        for (ordinal, arm) in self
            .program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .enumerate()
        {
            let mut covered = matches!(arm.pattern, MatchPattern::Wildcard);
            if let MatchPattern::Value(pattern) = arm.pattern {
                let pattern_capture_writes = self.operand_writes.len();
                self.expression(pattern, contexts, constraints);
                let pattern_operand = self.capture_operator_operand(
                    pattern,
                    pattern_capture_writes,
                    *contexts,
                    *constraints,
                );
                if let Ok(ordinal) = u32::try_from(ordinal)
                    && let Some(arm_index) =
                        dispatch.arms.start().arena_index().checked_add(ordinal)
                {
                    let source_arm =
                        arena::Handle::from_parts(arm_index, dispatch.arms.start().generation());
                    self.record_operator_invocation(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm },
                        &[subject_operand, pattern_operand],
                        *constraints,
                    );
                }
                if let ExpressionNode::Boolean(value) =
                    self.program.expression_table.expression(pattern)
                {
                    boolean_coverage[usize::from(*value)] = true;
                    covered = boolean_coverage.iter().all(|value| *value);
                }
            }
            let mut selected_contexts = *contexts;
            let mut selected_constraints = *constraints;
            // Arm writes are absent on the fallthrough path. Retain their union
            // only after all alternatives, for later enclosing operands.
            let branch_writes = self.operand_writes.len();
            if self
                .changed_operand_sources(&[(dispatch.subject, subject_writes)])
                .is_empty()
                && let Ok(ordinal) = u32::try_from(ordinal)
                && let Some(arm_index) = dispatch.arms.start().arena_index().checked_add(ordinal)
            {
                let source_arm =
                    arena::Handle::from_parts(arm_index, dispatch.arms.start().generation());
                let point = ProgramPoint::Statement {
                    machine_symbol: self.machine.symbol,
                    state_symbol: self.state.symbol,
                    statement_index: self.statement_index,
                };
                for (matched, branch_contexts, branch_constraints) in [
                    (true, &mut selected_contexts, &mut selected_constraints),
                    (false, &mut *contexts, &mut *constraints),
                ] {
                    super::super::exits::append_match_pattern_context(
                        self.program,
                        self.semantic,
                        self.context,
                        self.state.symbol,
                        self.statement_index,
                        FactPayload::MatchPattern {
                            expression,
                            arm: source_arm,
                            matched,
                        },
                        point,
                        branch_contexts,
                        branch_constraints,
                    );
                }
            }
            self.expression(arm.value, &mut selected_contexts, &mut selected_constraints);
            completed_writes.extend(self.operand_writes.drain(branch_writes..));
            if let Some((previous_contexts, previous_constraints)) = joined {
                self.meet(
                    previous_contexts,
                    previous_constraints,
                    &mut selected_contexts,
                    &mut selected_constraints,
                );
            }
            joined = Some((selected_contexts, selected_constraints));
            if covered {
                break;
            }
        }
        self.operand_writes.extend(completed_writes);
        if let Some((joined_contexts, joined_constraints)) = joined {
            *contexts = joined_contexts;
            *constraints = joined_constraints;
        }
    }
}
