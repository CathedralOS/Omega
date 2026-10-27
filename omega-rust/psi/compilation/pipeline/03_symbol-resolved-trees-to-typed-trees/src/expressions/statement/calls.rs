use crate::expressions::proposition::is_proposition_call_target;
use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

use super::arguments::{lower_statement_argument_span, lower_statement_path_members};

pub(super) fn lower_call_statement(
    lowerer: &mut Lowerer,
    call: &resolved::statement::TableCall,
) -> Result<typed::statement::StatementNode, Diagnostic> {
    if let Some(request) = crate::expressions::product_selections::request(
        lowerer.source_trees,
        call.receiver_symbol,
        call.target_symbol,
        call.target.as_str(),
        &call.machine_arguments,
        call.arguments.len(),
        call.evidence_arguments.len(),
        call.target.source_span(),
    )? {
        let mut occurrences = call.authored_call_selection.into_iter().collect::<Vec<_>>();
        if let language_semantics::declaration_selection::BuildSelectionRequest::Declaration(
            binding,
        ) = &request
        {
            occurrences.extend(
                lowerer
                    .source_trees
                    .authored_declaration_selections()
                    .iter()
                    .filter(|selection| {
                        binding.operands.iter().any(|operand| {
                            let span = selection.source_span();
                            span.source_id == operand.source_span.source_id
                                && span.span.start >= operand.source_span.span.start
                                && span.span.end <= operand.source_span.span.end
                        })
                    })
                    .map(|selection| selection.occurrence_id()),
            );
        }
        let arguments = lowerer
            .source_trees
            .tables
            .bodies
            .statements
            .expression_handles(call.arguments)
            .to_vec()
            .into_iter()
            .map(|argument| super::arguments::lower_statement_expression(lowerer, argument))
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let expression = crate::expressions::build_selections::lower(
            lowerer.source_trees,
            &mut lowerer.typed_trees,
            request,
            arguments,
            call.operational_acknowledgement,
        )?;
        lowerer
            .typed_trees
            .expression_table
            .attach_authored_selection_occurrences(expression, occurrences);
        return Ok(typed::statement::StatementNode::Expression(expression));
    }
    Ok(typed::statement::StatementNode::Call(lower_ordinary_call(
        lowerer, call,
    )?))
}

fn lower_ordinary_call(
    lowerer: &mut Lowerer,
    call: &resolved::statement::TableCall,
) -> Result<typed::statement::TableCall, Diagnostic> {
    if is_proposition_call_target(&lowerer.source_trees.symbols, call.target_symbol) {
        return Err(Diagnostic::error(
            "a proposition application is proof-only and cannot appear as an executable call statement",
        ));
    }
    let arguments = lower_statement_argument_span(lowerer, call.arguments)?;

    Ok(typed::statement::TableCall {
        authored_machine_application: Default::default(),
        receiver_root_symbol: call.receiver_root_symbol,
        receiver_symbol: call.receiver_symbol,
        target_symbol: call.target_symbol,
        receiver: lower_statement_path_members(lowerer, call.receiver),
        target: crate::lowerer::name::lower_name(&call.target),
        static_machine_parameter: symbols::SymbolHandle::invalid(),
        static_requirement_dispatch: None,
        machine_arguments: crate::signatures::lexical_context::call_arguments(
            Some(lowerer.source_trees),
            &mut lowerer.typed_trees,
            lowerer.type_reference_exposure,
            call.target_symbol,
            &call.machine_arguments,
        )?,
        arguments,
        evidence_arguments: call
            .evidence_arguments
            .iter()
            .map(crate::lowerer::name::lower_name)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: call.discards_result,
        source_span: call.target.source_span(),
        authored_call_selection: call.authored_call_selection,
    })
}
