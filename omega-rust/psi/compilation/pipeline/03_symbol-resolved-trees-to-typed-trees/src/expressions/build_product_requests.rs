//! Build recording operations use one source-issued Product request protocol.
//!
//! Product declarations are deliberately unavailable before the dependency join.
//! Retain declaration-only locators and their lexical occurrence; an ordinary
//! prepare/commit call records execution without importing or invoking Product.
//! Exact compiler namespace and original mutable Build authority remain checked.
//! Product-default declarations are a different context and retain their static
//! formation path. A similarly named ordinary machine is never rewritten here.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, BuildProductOperand, BuildProductRequest, ProductDeclarationLocator,
    ProductDeclarationSelectionRequest, ProductOperandRole,
};
use source::{DependencyScope, SourceOrigin, SourceSpan};
use symbols::{JoinedPath, SymbolHandle};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
};

fn build_context(program: &TypedTrees, span: SourceSpan) -> bool {
    program
        .symbols
        .source_file(span)
        .is_some_and(|file| file.dependency_scope == DependencyScope::Build)
}

fn build_namespace(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && program.symbols.is_build_prelude_symbol(symbol)
        && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == symbol && data.name.as_str() == "Build")
}

fn recording_operation(program: &TypedTrees, call: &TableCallExpression) -> Option<BuildOperation> {
    if call.target_symbol.is_valid()
        || call.build_product_request.is_valid()
        || !call.receiver.is_valid()
    {
        return None;
    }
    let ExpressionNode::Name(receiver) = program.expression_table.expression(call.receiver) else {
        return None;
    };
    build_namespace(program, receiver.symbol).then_some(())?;
    BuildOperation::from_call_target(call.target.as_str())
        .filter(|operation| operation.selects_product_declarations())
}

/// P02 retains recording statements as expression calls with their complete
/// directly attached occurrence roster. One route therefore covers ordinary,
/// discarded and expression calls without reconstructing source associations.
pub(crate) fn normalize(
    program: &mut TypedTrees,
    first_expression: usize,
) -> Result<(), Diagnostic> {
    let calls = program
        .expression_table
        .iter_expressions()
        .skip(first_expression)
        .filter_map(|(expression, node)| {
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            let span = program.expression_table.source_span(expression);
            build_context(program, span).then_some(())?;
            let operation = recording_operation(program, call)?;
            Some((expression, call.clone(), span, operation))
        })
        .collect::<Vec<_>>();
    for (expression, call, span, operation) in calls {
        lower(program, expression, call, span, operation)?;
    }
    Ok(())
}

fn lower(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    mut call: TableCallExpression,
    span: SourceSpan,
    operation: BuildOperation,
) -> Result<(), Diagnostic> {
    let refuse = |reason: &str| Diagnostic::error(reason).with_source_span(span);
    if !operation.product_operand_count_valid(call.machine_arguments.len())
        || !call.evidence_arguments.is_empty()
    {
        return Err(refuse(
            "Build product recording operation has invalid static declaration operands",
        ));
    }
    let values = program
        .expression_table
        .expression_handles(call.arguments)
        .to_vec();
    let with_mode = operation == BuildOperation::ProviderSelection && values.len() == 2;
    if values.is_empty() || values.len() != if with_mode { 2 } else { 1 } {
        return Err(refuse(
            "Build product recording operation requires its explicit mutable Build argument and declared value operands",
        ));
    }
    let occurrences = program
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
        .collect::<Vec<_>>();
    let operands = call.machine_arguments.iter().enumerate().map(|(position, argument)| {
        if !typed_trees::product_operand::is_declaration_operand(argument) {
            return Err(refuse("Build product operand must be an inert declaration path, not a type, application, value or evidence expression"));
        }
        // Positions belong to this exact call's attached occurrences. Value
        // arguments have their own call rosters; no lexical containment scan
        // or source-text parsing can lend their authority to this request.
        let mut selections = occurrences.iter().filter(|selection| matches!(selection.kind(), language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument | language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance) && selection.top_level_static_argument_position() == Some(position));
        let selected = selections.next().ok_or_else(|| refuse("Build product operand lost its authored occurrence"))?;
        if selections.next().is_some() { return Err(refuse("Build product operand has ambiguous authored occurrences")); }
        let operand_span = selected.source_span();
        let path = argument.path.joined_path();
        if operation.product_operand_role(position) == Some(ProductOperandRole::WireCompatibilityFact) {
            let fact = language_semantics::wire::WireCompatibilityFact::from_name(&path)
                .ok_or_else(|| refuse("Build wire demand names an unknown compatibility fact"))?;
            Ok(BuildProductOperand::WireFact { source_span: operand_span, fact })
        } else {
            Ok(BuildProductOperand::Declaration(ProductDeclarationLocator { source_span: operand_span, path }))
        }
    }).collect::<Result<Vec<_>, Diagnostic>>()?;
    let request = ProductDeclarationSelectionRequest {
        source_span: span,
        operation,
        operands: operands.into_boxed_slice(),
    };
    request.validate_shape().map_err(refuse)?;
    let owner = program
        .traits()
        .iter()
        .find(|owner| {
            owner.is_boundary
                && owner.name.as_str() == "BuildProductOperations"
                && program
                    .symbols
                    .symbol_source_span(owner.symbol)
                    .and_then(|span| program.symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == SourceOrigin::Toolchain
                            && file.path == std::path::Path::new("<build-prelude>")
                    })
        })
        .ok_or_else(|| {
            refuse("Build product recording has no compiler-owned execution protocol")
        })?;
    let signatures = program.trait_machine_signatures(owner);
    let prepare = signatures
        .iter()
        .find(|signature| signature.name.as_str() == "prepare")
        .ok_or_else(|| refuse("Build product protocol has no receiver preparation"))?;
    let commit = signatures
        .iter()
        .find(|signature| {
            signature.name.as_str() == if with_mode { "select_mode" } else { "select" }
        })
        .ok_or_else(|| refuse("Build product protocol has no selection commit"))?;
    let (
        owner_symbol,
        owner_name,
        preparation_symbol,
        preparation_name,
        commit_symbol,
        commit_name,
    ) = (
        owner.symbol,
        owner.name.clone(),
        prepare.symbol,
        prepare.name.clone(),
        commit.symbol,
        commit.name.clone(),
    );
    let table = &mut program.expression_table;
    let mut members = Default::default();
    let mut symbols = Default::default();
    table.push_name_path_member(&mut members, owner_name);
    table.push_name_path_member_symbol(&mut symbols, owner_symbol);
    let receiver = table.insert(ExpressionNode::Name(TableNamePath::new(
        members,
        symbols,
        owner_symbol,
        owner_symbol,
    )));
    let request =
        table.insert_build_product_request(BuildProductRequest::DeclarationSelection(request));
    call.receiver = receiver;
    call.machine_arguments = Box::default();
    call.evidence_arguments = Box::default();
    call.authored_machine_application = Default::default();
    call.static_requirement_dispatch = None;
    call.static_machine_parameter = Default::default();
    call.requirement_application = Default::default();
    call.target_symbol = preparation_symbol;
    call.target = preparation_name;
    call.build_product_request = request;
    call.arguments = table.insert_expression_handles([values[0]]);
    let preparation = table.insert(ExpressionNode::Call(call.clone()));
    table.set_source_span(preparation, span);
    call.target_symbol = commit_symbol;
    call.target = commit_name;
    call.arguments = table
        .insert_expression_handles(std::iter::once(preparation).chain(values.into_iter().skip(1)));
    *table.expression_mut(expression) = ExpressionNode::Call(call);
    Ok(())
}
