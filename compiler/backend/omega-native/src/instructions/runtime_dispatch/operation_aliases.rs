use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::{
    RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
};

use super::super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, set_runtime_alias,
    strip_mutable_expression,
};
use super::super::lookups::state_call_for_statement;

pub(super) fn bind_runtime_operation_aliases(
    native_plan: &NativePlan,
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
        state_call_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        if argument.kind != crate::state_calls::StateCallArgumentKind::MutableAlias {
            continue;
        }

        let expression = strip_mutable_expression(resolve_runtime_alias_expression(
            &argument.expression,
            state_call.source_key,
            aliases,
        ));
        set_runtime_alias(
            aliases,
            RuntimeAliasBinding {
                source_key: state_call.target_key,
                parameter_symbol: argument.parameter_symbol,
                parameter_name: argument.parameter_name.clone(),
                expression,
            },
        );
    }
}
