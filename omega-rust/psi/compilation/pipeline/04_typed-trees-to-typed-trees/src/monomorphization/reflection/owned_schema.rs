//! An authorized source query becomes ordinary owned descriptive program data.

use super::*;

pub(super) fn elaborate(
    program: &mut TypedTrees,
    candidate: &Candidate,
    selection: &CallSelection,
) -> Result<(), Vec<Diagnostic>> {
    // Replacing the query must not erase an invalid actual (and its effects)
    // before ordinary call checking can diagnose it.
    let operands_are_empty = match selection.site {
        CallSite::Expression(expression) => {
            let ExpressionNode::Call(call) = expressions_of(program).expression(expression) else {
                return Err(error("semantic schema lost its selected call occurrence"));
            };
            !call.receiver.is_valid()
                && call.arguments.is_empty()
                && call.evidence_arguments.is_empty()
        }
        CallSite::Statement(statement) => {
            let StatementNode::Call(call) = program.statement_table.statement(statement) else {
                return Err(error("semantic schema lost its selected call occurrence"));
            };
            call.arguments.is_empty() && call.evidence_arguments.is_empty()
        }
    };
    if !operands_are_empty {
        return Err(error(
            "semantic schema query accepts no runtime or evidence operands",
        ));
    }
    let [Some(subject)] = candidate.type_bindings.as_slice() else {
        return Err(error("semantic schema requires its exact selected subject"));
    };
    let subject = program
        .closed_data_definition(*subject)
        .cloned()
        .ok_or_else(|| error("semantic schema requires a closed semantic data application"))?;
    let author = selection.lexical_owner;
    let author_span = program
        .symbols
        .symbol_provenance_source_span(author)
        .ok_or_else(|| error("semantic schema requires its lexical source authority"))?;
    if !program
        .symbols
        .source_reference_may_select_symbol(author_span, subject.symbol)
    {
        return Err(error(
            "semantic schema cannot select the subject from its lexical author's scope",
        ));
    }
    let requester_identity = program
        .normalized_hermetic_symbol_identity(author)
        .map_err(|message| error(format!("semantic schema author identity: {message}")))?;
    let authority = if program
        .symbols
        .same_symbol_source_package(author, subject.symbol)
    {
        validation::reflection::SchemaQueryAuthority::OwningScope { requester_identity }
    } else {
        validation::reflection::SchemaQueryAuthority::ForeignScope { requester_identity }
    };
    let graph =
        validation::reflection::construct_semantic_schema_graph(program, subject.symbol, authority)
            .map_err(error)?;
    // The query's exact declared result owns the source vocabulary. No lookup
    // by another package's TypeSchema spelling can acquire catalog privileges.
    let query = &program.machines()[candidate.template.machine_index];
    let result = program
        .machine_states(query)
        .first()
        .ok_or_else(|| error("semantic schema lost its declared result"))?
        .return_type;
    let result = program
        .closed_data_definition(result)
        .cloned()
        .filter(|definition| is_reflection_declaration(program, definition.symbol))
        .ok_or_else(|| error("semantic schema requires its toolchain-owned result vocabulary"))?;
    let bytes =
        expressions_of_mut(program).insert(ExpressionNode::String(graph.runtime_image().into()));
    let field = info_field(program, &result, "storage", bytes)?;
    let fields = expressions_of_mut(program).insert_struct_fields([field]);
    let value = ExpressionNode::StructLiteral(TableStructLiteral {
        type_name: result.name.clone(),
        type_symbol: result.symbol,
        fields,
        ..Default::default()
    });
    match selection.site {
        CallSite::Expression(expression) => {
            *expressions_of_mut(program).expression_mut(expression) = value
        }
        CallSite::Statement(statement) => {
            let value = expressions_of_mut(program).insert(value);
            *program.statement_table.statement_mut(statement) = StatementNode::Expression(value);
        }
    }
    Ok(())
}
