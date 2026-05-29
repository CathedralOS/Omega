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
        arguments,
    })
}
