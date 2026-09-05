use super::*;

impl Execution<'_, '_, '_> {
    /// Evaluate one selected arm's operands with the ordinary expression
    /// schedule, saving each completed value before evaluating the next operand.
    pub(in crate::flow) fn transition_target(
        &mut self,
        transition: &TableTransition,
        target: TransitionTargetHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let mut operands = Vec::new();
        let mut values = Vec::new();
        match self.program.statement_table.transition_target(target) {
            TransitionTargetNode::Named { arguments, .. } => {
                for (ordinal, argument) in self
                    .program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .enumerate()
                {
                    operands.push((*argument, self.operand_writes.len()));
                    self.expression(*argument, contexts, constraints);
                    values.push(super::super::state_values::capture_argument(
                        self.program,
                        self.semantic,
                        self.context,
                        self.machine,
                        self.state,
                        self.statement_index,
                        target,
                        ordinal,
                        *argument,
                        *contexts,
                    ));
                }
            }
            TransitionTargetNode::Value(expression) => {
                self.expression(*expression, contexts, constraints);
            }
            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
        }
        super::super::state_values::record_transition(
            self.program,
            self.semantic,
            self.context,
            self.machine,
            self.state,
            transition,
            target,
            *contexts,
            &values,
        );
        self.invoke(
            InvocationSite::Transition(target),
            &operands,
            contexts,
            constraints,
        );
    }
}
