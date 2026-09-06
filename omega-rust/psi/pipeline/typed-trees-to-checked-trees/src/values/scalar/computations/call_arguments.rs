use super::*;

impl Builder<'_, '_> {
    pub(super) fn record_call_arguments(
        &mut self,
        pure: &CheckedScalarExpressionPlans,
        statement: u32,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) {
        let Some(parameters) = crate::call_target_parameters(self.program, target) else {
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
            let Some(primitive_type) = self
                .program
                .primitive_type_reference(parameter.type_reference)
            else {
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
                    call_ordinal: 0,
                    argument_ordinal: scalar_ordinal,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
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
