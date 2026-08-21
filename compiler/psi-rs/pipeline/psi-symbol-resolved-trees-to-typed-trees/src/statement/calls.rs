use crate::lowerer::Lowerer;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

use super::arguments::{lower_statement_argument_span, lower_statement_path_members};

pub(super) fn lower_call_statement(
    lowerer: &mut Lowerer,
    call: &resolved::statement::TableCall,
) -> Result<typed::statement::TableCall, Diagnostic> {
    if call.target_symbol.is_valid()
        && matches!(
            lowerer.source_trees.symbols.get(call.target_symbol).kind,
            psi_symbols::SymbolKind::Proposition | psi_symbols::SymbolKind::PropositionParameter
        )
    {
        return Err(Diagnostic::error(
            "a proposition application is proof-only and cannot appear as an executable call statement",
        ));
    }
    let arguments = lower_statement_argument_span(lowerer, call.arguments)?;

    Ok(typed::statement::TableCall {
        receiver_symbol: call.receiver_symbol,
        target_symbol: call.target_symbol,
        receiver: lower_statement_path_members(lowerer, call.receiver),
        target: crate::name::lower_name(&call.target),
        machine_arguments: call
            .machine_arguments
            .iter()
            .map(crate::expression::lower_static_machine_argument)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        arguments,
        evidence_arguments: call
            .evidence_arguments
            .iter()
            .map(crate::name::lower_name)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: call.discards_result,
    })
}
