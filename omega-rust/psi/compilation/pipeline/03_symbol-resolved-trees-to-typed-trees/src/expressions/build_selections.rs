//! Normalize source-issued Build choices to the ordinary boundary-call path.
//! Preparation authenticates the explicit activation before later value operands
//! execute. The private request context is captured beside those exact calls.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, RootBindingOperand,
};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
};

pub(crate) fn lower(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    target: &mut typed_trees::TypedTrees,
    request: typed_trees::expression::BuildSelectionRequest,
    mut arguments: Vec<ExpressionHandle>,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
) -> Result<ExpressionHandle, Diagnostic> {
    let source_span = request.source_span();
    let operation = match (&request, arguments.len()) {
        (BuildSelectionRequest::ProviderReadiness(_), 1) => "await_provider",
        (BuildSelectionRequest::Exclusion(_), 2) => "exclude",
        (BuildSelectionRequest::Entry(binding), 1)
            if matches!(binding.implementation, RootBindingOperand::AuthoredPath(_)) =>
        {
            "bind"
        }
        (BuildSelectionRequest::Entry(binding), 2)
            if matches!(binding.implementation, RootBindingOperand::Description) =>
        {
            "bind_description"
        }
        (BuildSelectionRequest::Declaration(binding), 1)
            if binding.operation.selects_product_declarations() =>
        {
            "select"
        }
        (BuildSelectionRequest::Declaration(binding), 2)
            if binding.operation == BuildOperation::ProviderSelection =>
        {
            "select_mode"
        }
        _ => {
            return Err(
                Diagnostic::error("Build selection has unexpected ordinary operands")
                    .with_source_span(source_span),
            );
        }
    };
    let owner = source
        .traits
        .iter()
        .find(|definition| {
            definition.is_boundary
                && definition.name.as_str() == "BuildBindingOperations"
                && source
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .and_then(|span| source.symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == source::SourceOrigin::Toolchain
                            && file.path == std::path::Path::new("<build-prelude>")
                    })
        })
        .ok_or_else(|| {
            Diagnostic::error("Build selection requires its exact toolchain protocol")
                .with_source_span(source_span)
        })?;
    let find = |name| {
        source
            .trait_machine_signatures(owner.machines)
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "Build selection protocol has no `{name}` operation"
                ))
                .with_source_span(source_span)
            })
    };
    let preparation = find("prepare")?;
    let commit = find(operation)?;
    let value_helper = match &request {
        BuildSelectionRequest::Exclusion(request) => Some(request.vocabulary.helper()),
        _ if operation == "select_mode" => Some("build_selection_independent"),
        _ => None,
    };
    let table = &mut target.expression_table;
    let mut members = Default::default();
    let mut member_symbols = Default::default();
    table.push_name_path_member(&mut members, crate::lowerer::name::lower_name(&owner.name));
    table.push_name_path_member_symbol(&mut member_symbols, owner.symbol);
    let receiver = table.insert(ExpressionNode::Name(TableNamePath::new(
        members,
        member_symbols,
        owner.symbol,
        owner.symbol,
    )));
    let build_selection = table.insert_build_selection_request(request);
    let mut emit = |symbol,
                    name: typed_trees::name::Identifier,
                    receiver,
                    arguments: Vec<ExpressionHandle>,
                    context,
                    operational_acknowledgement| {
        let arguments = table.insert_expression_handles(arguments);
        let expression = table.insert(ExpressionNode::Call(TableCallExpression {
            authored_call_binding: None,
            authored_machine_application: Default::default(),
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver,
            target_symbol: symbol,
            static_machine_parameter: Default::default(),
            target: name,
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            quotient_operation: None,
            private_layout_operation: None,
            layout_query: None,
            native_code_layout: None,
            build_selection: context,
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement,
        }));
        table.set_source_span(expression, source_span);
        expression
    };
    if let Some(helper_name) = value_helper {
        // Closed toolchain enums are ordinary checked data. Exhaustive helpers
        // encode protocol values; hosts never guess structural layout tags.
        let helper = source
            .machines
            .iter()
            .find(|machine| {
                machine.name.as_str() == helper_name
                    && source.symbols.is_build_prelude_symbol(machine.symbol)
            })
            .and_then(|machine| source.machine_state_handles(machine.states).first())
            .map(|state| source.machine_state(*state))
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "Build selection has no checked value helper `{helper_name}`"
                ))
                .with_source_span(source_span)
            })?;
        arguments[1] = emit(
            helper.symbol,
            crate::lowerer::name::lower_name(&helper.name),
            ExpressionHandle::invalid(),
            vec![arguments[1]],
            Default::default(),
            Default::default(),
        );
    }
    arguments[0] = emit(
        preparation.symbol,
        crate::lowerer::name::lower_name(&preparation.name),
        receiver,
        vec![arguments[0]],
        build_selection,
        Default::default(),
    );
    Ok(emit(
        commit.symbol,
        crate::lowerer::name::lower_name(&commit.name),
        receiver,
        arguments,
        build_selection,
        acknowledgement,
    ))
}
