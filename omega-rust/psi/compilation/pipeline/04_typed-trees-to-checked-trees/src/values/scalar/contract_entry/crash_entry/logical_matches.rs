//! Proof-local record subject bindings, distinct from runtime completed homes.
//!
//! A supported match observes one pure readable path at this query's frontier.
//! Its arm contains only the ordinary predicate algebra, so nothing can write
//! between that observation and an alias read. Ambient producers are not replayed.

use super::{ComparisonOperand, FieldPath, LogicalMatchBinding, Reader};
use checked_trees::CheckedBooleanExpression;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

impl Reader<'_> {
    fn pure_match_subject(&mut self, expression: ExpressionHandle, depth: usize) -> Option<()> {
        self.charge(depth)?;
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => Some(()),
            ExpressionNode::Member(member) => self.pure_match_subject(member.receiver, depth + 1),
            ExpressionNode::SavedSubject(_) if !self.logical_captures.is_empty() => {
                self.logical_saved_path(expression, depth + 1).map(|_| ())
            }
            _ => None,
        }
    }

    fn record_binding(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<LogicalMatchBinding> {
        self.charge(depth)?;
        let program = self.program;
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
        else {
            return None;
        };
        self.pure_match_subject(dispatch.subject, depth + 1)?;
        let observed = self.subject(dispatch.subject, depth + 1)?;
        let capture =
            validation::logical_record_match_capture(program, expression, observed.type_reference)?;
        Some(LogicalMatchBinding { capture, observed })
    }

    pub(super) fn logical_match_boolean(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        let binding = self.record_binding(expression, depth + 1)?;
        let arm = binding.capture.arm_value();
        let previous = self.logical_captures.len();
        self.logical_captures.push(binding);
        let result = self.boolean(arm, depth + 1);
        self.logical_captures.truncate(previous);
        result
    }

    pub(super) fn logical_match_operand(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ComparisonOperand> {
        let binding = self.record_binding(expression, depth + 1)?;
        let arm = binding.capture.arm_value();
        let previous = self.logical_captures.len();
        self.logical_captures.push(binding);
        let result = self.comparison_operand(arm, depth + 1);
        self.logical_captures.truncate(previous);
        result
    }

    fn saved_projection_leaf(&self, mut expression: ExpressionHandle) -> Option<ExpressionHandle> {
        for _ in 0..64 {
            match self.program.expression_table.expression(expression) {
                ExpressionNode::SavedSubject(_) => return Some(expression),
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                ExpressionNode::Borrow(borrow) => expression = borrow.target,
                _ => return None,
            }
        }
        None
    }

    pub(super) fn is_saved_projection(&self, expression: ExpressionHandle) -> bool {
        self.saved_projection_leaf(expression).is_some()
    }

    pub(super) fn logical_saved_path(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<FieldPath> {
        self.charge(depth)?;
        let leaf = self.saved_projection_leaf(expression)?;
        let ExpressionNode::SavedSubject(saved) = self.program.expression_table.expression(leaf)
        else {
            return None;
        };
        let place = validation::canonical_place_from_expression(self.program, expression)?;
        let facts::PlaceRoot::Expression(anchor) = place.root else {
            return None;
        };
        let binding = self.logical_captures.iter().rev().find(|binding| {
            binding.capture.anchor() == anchor && binding.capture.permits_alias(self.program, saved)
        })?;
        let mut observed = binding.observed.clone();
        observed.path.extend(
            super::super::super::structural_fields::structural_projection_path(
                self.program,
                &place.segments,
            )?,
        );
        observed.type_reference =
            super::super::super::structural_fields::readable_entry_parameter_path(
                self.program,
                self.parameters,
                u32::try_from(observed.parameter_position).ok()?,
                &observed.path,
                &mut self.remaining,
            )?;
        Some(observed)
    }
}
