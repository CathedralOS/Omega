use super::{
    Builder, CheckedScalarExpressionPlans, CheckedScalarExpressionRole, ExpressionHandle,
    SymbolHandle,
};
use crate::values::scalar::call_lowering::call_is_boundary;

impl Builder<'_, '_> {
    pub(super) fn record_call_arguments(
        &mut self,
        pure: &CheckedScalarExpressionPlans,
        values: &mut checked_trees::CheckedStructuralValuePlans,
        statement: u32,
        call_ordinal: u32,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) {
        let Some(parameters) = crate::semantic_calls::call_target_parameters(self.program, target)
        else {
            return;
        };
        let explicit_self = arguments.len()
            > parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
        let explicit_parameters = parameters
            .iter()
            .filter(|parameter| !parameter.is_self || explicit_self)
            .collect::<Vec<_>>();
        if explicit_parameters.len() != arguments.len() {
            return;
        }
        let boundary = call_is_boundary(self.program, target);
        let mut scalar_ordinal = 0u32;
        for (argument, parameter) in arguments.iter().zip(explicit_parameters) {
            // An erased parameter position has no scalar ordinal: the callee
            // plan (`execution::unit::calls`) skips it the same way.
            match crate::execution::terminal_unit::strips_erased_parameter(parameter) {
                Some(true) => continue,
                Some(false) => {}
                None => return,
            }
            let Some(primitive_type) = self
                .program
                .primitive_type_reference(parameter.type_reference)
            else {
                // An inline case construction at a non-primitive position is
                // still a structural value: retain it rooted at the authored
                // argument expression so the Unit statement sequence can
                // establish it as a state-local operand before the call.
                // Payload fields are primitive by `scalar_case_constructor`,
                // so the construction introduces no place custody.
                if !parameter.is_self
                    && !parameter.is_const
                    && validation::scalar_case_constructor(self.program, *argument).is_some_and(
                        |constructor| {
                            self.program
                                .normalized_type_identity(constructor.type_reference)
                                == self
                                    .program
                                    .normalized_type_identity(parameter.type_reference)
                        },
                    )
                    && values
                        .root_for_expression(self.state, statement, *argument)
                        .is_none()
                    && let Some(root) =
                        self.structural_value(*argument, parameter.type_reference, values, pure)
                {
                    values
                        .roots
                        .append(checked_trees::CheckedStructuralValueRoot {
                            machine: self.machine,
                            state: self.state,
                            statement_ordinal: statement,
                            expression: *argument,
                            type_reference: parameter.type_reference,
                            root,
                        });
                }
                continue;
            };
            if parameter.is_self
                || parameter.is_const
                || (parameter.is_mutable
                    && crate::values::mutable_scalar_parameter_type(self.program, parameter)
                        .is_none())
            {
                return;
            }
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal,
                    argument_ordinal: scalar_ordinal,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal,
                    argument_ordinal: scalar_ordinal,
                }
            };
            self.record_root(pure, statement, role, *argument, primitive_type);
            let Some(next) = scalar_ordinal.checked_add(1) else {
                return;
            };
            scalar_ordinal = next;
        }
    }
}
