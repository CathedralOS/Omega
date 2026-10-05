use super::{Builder, CheckedScalarExpressionRole, ExpressionHandle, SymbolHandle};
use crate::values::scalar::call_lowering::call_is_boundary;

impl Builder<'_, '_> {
    pub(super) fn record_call_arguments(
        &mut self,
        statement: u32,
        call_ordinal: u32,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) {
        let Some(parameters) = crate::semantic::calls::call_target_parameters(self.program, target)
        else {
            return;
        };
        let explicit_parameters = parameters.iter();
        if parameters.len() != arguments.len() {
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
                // The shared value owner retains construction and copying at
                // their exact argument occurrence. Lending admission remains
                // separate; no source-shape roster decides what a value is.
                if !parameter.is_const {
                    self.retain_structural_argument(*argument, parameter.type_reference);
                }
                continue;
            };
            if parameter.is_const
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
            self.record_root(self.pure, statement, role, *argument, primitive_type);
            let Some(next) = scalar_ordinal.checked_add(1) else {
                return;
            };
            scalar_ordinal = next;
        }
    }
}
