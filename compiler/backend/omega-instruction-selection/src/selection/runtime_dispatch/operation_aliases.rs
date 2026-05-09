use crate::InstructionSelectionInput;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};

use super::super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_binding, set_runtime_alias, strip_mutable_expression,
};
use super::super::lookups::state_call_for_statement;

pub(super) fn bind_runtime_operation_aliases(
    input: &InstructionSelectionInput<'_>,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &mut Vec<RuntimeAliasBinding>,
) {
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
        | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
        RuntimeDispatchBodyOperationKind::HostCall { .. }
        | RuntimeDispatchBodyOperationKind::LocalStorage { .. }
        | RuntimeDispatchBodyOperationKind::Mutation { .. }
        | RuntimeDispatchBodyOperationKind::Other => return,
    }

    let Some(state_call) =
        state_call_for_statement(input, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let resolved_expression =
            resolve_runtime_alias_binding(&argument.expression, state_call.source_key, aliases);
        let expression = strip_mutable_expression(resolved_expression.expression);
        set_runtime_alias(
            aliases,
            RuntimeAliasBinding {
                source_key: state_call.target_key,
                parameter_symbol: argument.parameter_symbol,
                parameter_name: argument.parameter_name.clone(),
                expression_source_key: resolved_expression.source_key,
                expression,
            },
        );
    }
}
