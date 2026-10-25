//! Completed values keep their exact producer and authored operand position.
//! Aliases authenticate against that owner and have no executable source edge.
//! Typing can inspect the supplying carrier; execution must read an already
//! established home and reject a missing dominating capture.

impl crate::TypedTrees {
    /// Authenticate a non-executable alias against its real value producer.
    /// This names the source for typing and capture lookup, never for replay.
    pub fn saved_value_source(
        &self,
        saved: &crate::expression::TableSavedSubject,
    ) -> Option<crate::expression::ExpressionHandle> {
        use crate::expression::ValueCaptureOwner;
        let table = &self.expression_table;
        if !table.contains_value_capture(saved.capture) {
            return None;
        }
        let capture = table.value_capture(saved.capture);
        let source = match capture.owner {
            ValueCaptureOwner::Match => return table.saved_subject_source(saved),
            ValueCaptureOwner::ExpressionArgument { call, position } => {
                if !table.expression_is_valid(call) {
                    return None;
                }
                let call = table.call_node(call)?;
                *table
                    .expression_handles(call.arguments)
                    .get(position as usize)?
            }
            ValueCaptureOwner::StatementArgument { call, position } => {
                if !self.statement_table.statement_is_valid(call) {
                    return None;
                }
                let call = self.statement_table.statement(call).as_call()?;
                *self
                    .statement_table
                    .expression_handles(call.arguments)
                    .get(position as usize)?
            }
        };
        (!saved.pattern_head.is_valid()
            && source == capture.subject
            && table.expression_is_valid(source))
        .then_some(source)
    }

    /// A call-owned alias may only observe its completed argument home. It
    /// never supplies another operation which could establish or transfer it.
    pub fn saved_argument_source(
        &self,
        expression: crate::expression::ExpressionHandle,
    ) -> Option<crate::expression::ExpressionHandle> {
        let crate::expression::ExpressionNode::SavedSubject(saved) =
            self.expression_table.expression(expression)
        else {
            return None;
        };
        if !self.expression_table.contains_value_capture(saved.capture)
            || matches!(
                self.expression_table.value_capture(saved.capture).owner,
                crate::expression::ValueCaptureOwner::Match
            )
        {
            return None;
        }
        self.saved_value_source(saved)
    }
}
