use crate::lowerer::Lowerer;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

use super::arguments::{lower_statement_argument_span, lower_statement_path_members};

pub(super) fn lower_call_statement(
    lowerer: &mut Lowerer,
    call: &resolved::statement::TableCall,
) -> Result<typed::statement::TableCall, Diagnostic> {
    let arguments = lower_statement_argument_span(lowerer, call.arguments)?;

    Ok(typed::statement::TableCall {
        receiver_symbol: call.receiver_symbol,
        target_symbol: call.target_symbol,
        receiver: lower_statement_path_members(lowerer, call.receiver),
        target: crate::name::lower_name(&call.target),
        machine_arguments: call
            .machine_arguments
            .iter()
            .map(|argument| typed::expression::StaticMachineArgument {
                path: argument
                    .path
                    .iter()
                    .map(crate::name::lower_name)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                symbol: argument.symbol,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        arguments,
        discards_result: call.discards_result,
    })
}
