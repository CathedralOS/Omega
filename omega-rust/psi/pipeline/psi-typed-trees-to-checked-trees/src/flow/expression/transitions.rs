use super::*;

impl Execution<'_, '_> {
    /// Evaluate one selected arm's operands with the ordinary expression
    /// schedule, then capture state inputs before the jump consumes its frame.
    pub(in crate::flow) fn transition_target(
        &mut self,
        transition: &TableTransition,
        target: TransitionTargetHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let first_write = self.operand_writes.len();
        let mut operands = Vec::new();
        match self.program.statement_table.transition_target(target) {
            TransitionTargetNode::Named { arguments, .. } => {
                for argument in self.program.statement_table.expression_handles(*arguments) {
                    operands.push((*argument, self.operand_writes.len()));
                    self.expression(*argument, contexts, constraints);
                }
            }
            TransitionTargetNode::Value(expression) => {
                self.expression(*expression, contexts, constraints);
            }
            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
        }
        let mut operand_writes = Some(Vec::new());
        for writes in &self.operand_writes[first_write..] {
            match (&mut operand_writes, writes) {
                (Some(combined), Some(writes)) => {
                    for place in writes {
                        if !combined.contains(place) {
                            combined.push(place.clone());
                        }
                    }
                }
                (_, None) => operand_writes = None,
                (None, _) => {}
            }
        }
        super::super::state_values::record_transition(
            self.program,
            self.semantic,
            self.context,
            self.machine,
            self.state,
            self.statement_index,
            transition,
            target,
            *contexts,
            operand_writes.as_deref(),
        );
        self.invoke(
            InvocationSite::Transition(target),
            &operands,
            contexts,
            constraints,
        );
    }
}
