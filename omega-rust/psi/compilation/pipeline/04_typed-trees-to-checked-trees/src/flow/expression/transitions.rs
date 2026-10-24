use super::super::state_values::qualifications;
use super::{TableTransition, TransitionTargetHandle, TransitionTargetNode};
use crate::flow::expression::Execution;
use crate::flow::expression::InvocationSite;
use arena::HandleSpan;
use checked_trees::{FlowConstraintRef, FlowSemanticContextRef};

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
        self.evaluation = checked_trees::FlowPermissionEvaluation::TransitionTarget(target);
        let mut returned_value = arena::Handle::invalid();
        let mut operands = Vec::new();
        let mut captured_operands = Vec::new();
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
                    let first_write = self.operand_writes.len();
                    operands.push((*argument, first_write));
                    let destination =
                        self.invocation_parameter_type(InvocationSite::Transition(target), ordinal);
                    captured_operands.push(
                        self.evaluated_operand_as(*argument, destination, contexts, constraints)
                            .capture,
                    );
                    captured_qualifications.extend(qualifications::capture_argument(
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
                let completed = self.evaluated_operand_as(
                    *expression,
                    self.state.return_type,
                    contexts,
                    constraints,
                );
                returned_value = self.context.control.operands.append(completed.capture);
            }
            TransitionTargetNode::Terminal => {}
        }
        // Owned values have already been captured. Reference-backed claims
        // still depend on live storage and independently stable bindings.
        // An all-isolated (or empty) capture list never consults the verdict,
        // so the resolver walk only runs when a non-isolated claim needs it.
        let bindings_stable = captured_qualifications
            .iter()
            .any(|captured| !captured.is_isolated())
            && self.context.call_frames.is_some_and(|frames| {
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
            self.statement_index,
            transition,
            target,
            *contexts,
            &captured_operands,
            &values,
            argument_qualifications,
        );
        super::super::exits::append_outgoing_state_fact(
            self.context,
            self.machine.symbol,
            self.state.symbol,
            self.statement_index,
            target,
            *constraints,
            returned_value,
        );
        self.invoke(
            InvocationSite::Transition(target),
            super::InvocationInputs {
                operands: &captured_operands,
                arguments: &captured_operands,
            },
            contexts,
            constraints,
        );
    }
}
