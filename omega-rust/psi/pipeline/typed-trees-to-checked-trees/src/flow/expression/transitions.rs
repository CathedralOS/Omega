use super::super::state_values::qualifications;
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
        let mut captured_qualifications = Vec::new();
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
                    captured_qualifications.extend(qualifications::capture_argument(
                        self.program,
                        self.semantic,
                        self.context,
                        self.machine,
                        self.state,
                        target,
                        ordinal,
                        *argument,
                        *contexts,
                    ));
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
        // Owned values have already been captured. Reference-backed claims
        // still depend on live storage and independently stable bindings.
        let bindings_stable = self.context.call_frames.is_some_and(|frames| {
            operands.iter().all(|(argument, _)| {
                frames.expression_reference_bindings_are_stable(self.machine, *argument)
            })
        });
        let argument_qualifications = qualifications::finish(
            self.context,
            *contexts,
            captured_qualifications,
            bindings_stable,
        );
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
            argument_qualifications,
        );
        self.invoke(
            InvocationSite::Transition(target),
            &operands,
            contexts,
            constraints,
        );
    }
}
