use super::{backend_state_call_receiver_name, backend_state_name};

use crate::BackendReportInput;

pub(super) fn write_state_call_sections(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## State Call Lowering\n");
    output.push_str(&format!(
        "calls: {}\n",
        backend_plan.state_calls.calls.len()
    ));
    if backend_plan.state_calls.calls.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, state_call) in backend_plan.state_calls.calls.iter() {
            let source_name = backend_state_name(backend_plan, state_call.source_key);
            let target_name = if state_call.target_key.is_valid() {
                backend_state_name(backend_plan, state_call.target_key)
            } else {
                "unresolved".to_owned()
            };
            output.push_str(&format!(
                "- {} statement {} `{}` -> {} args {} {:?}/{:?} reachable {} required {}\n",
                source_name,
                state_call.statement_index,
                backend_state_call_receiver_name(backend_plan, state_call),
                target_name,
                state_call.argument_count,
                state_call.resolution,
                state_call.lowering,
                state_call.reachable,
                state_call.required
            ));

            match backend_plan
                .state_calls
                .arguments
                .span(state_call.arguments)
            {
                Some(arguments) if arguments.is_empty() => {
                    output.push_str("  arguments: none\n");
                }
                Some(arguments) => {
                    output.push_str("  arguments:\n");
                    for argument in arguments {
                        output.push_str(&format!(
                            "    - #{} `{}` {:?}: `{}` required {}\n",
                            argument.index,
                            argument.parameter_name,
                            argument.kind,
                            backend_plan
                                .state_calls
                                .expressions
                                .display_name(argument.expression),
                            argument.required
                        ));
                    }
                }
                None => output.push_str("  arguments: invalid span\n"),
            }
        }
    }
    output.push('\n');

    output.push_str("## Alias Flow\n");
    output.push_str(&format!(
        "aliases: {}\n",
        backend_plan.alias_flow.aliases.len()
    ));
    if backend_plan.alias_flow.aliases.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, alias) in backend_plan.alias_flow.aliases.iter() {
            let caller_name = backend_state_name(backend_plan, alias.caller_key);
            let callee_name = backend_state_name(backend_plan, alias.callee_key);
            output.push_str(&format!(
                "- {} statement {} -> {} `{}` aliases `{}` required {}\n",
                caller_name,
                alias.statement_index,
                callee_name,
                alias.parameter_name,
                backend_plan
                    .alias_flow
                    .expressions
                    .display_name(alias.argument),
                alias.required
            ));
        }
    }
    output.push('\n');
}
