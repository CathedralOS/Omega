//! Reconstruct authored trust grants and their exact source occurrences.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use typed_trees::TypedTrees;

/// Harvest namespace-qualified acceptances from the admitted build machine.
/// Classification checks the compiler's Build namespace and explicit argument;
/// an ordinary machine with the same spelling cannot grant authority.
pub fn harvest_root_grants(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<crate::AuthoredRootGrant>, Diagnostic> {
    let mut grants = Vec::new();
    let mut record = |selector: &str, source_span: source::SourceSpan| {
        if !grants
            .iter()
            .any(|grant: &crate::AuthoredRootGrant| grant.selector == selector)
        {
            grants.push(crate::AuthoredRootGrant {
                selector: selector.to_owned(),
                selecting_machine: machine.symbol,
                source_span,
            });
        }
    };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            if let typed_trees::statement::StatementNode::Expression(expression) = statement
                && typed_trees_to_checked_trees::typed_build_operation(typed, *expression)
                    == Some(BuildOperation::BoundaryAcceptance)
                && let typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(*expression)
            {
                let [argument] = call.machine_arguments.as_ref() else {
                    return Err(Diagnostic::error(
                        "boundary acceptance requires one declaration",
                    ));
                };
                let selector = argument
                    .path
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                record(
                    &selector,
                    authored_root_grant_expression_span(typed, *expression)?,
                );
            }
        }
    }
    Ok(grants)
}

fn authored_root_grant_expression_span(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<source::SourceSpan, Diagnostic> {
    let occurrences = typed
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            typed
                .authored_declaration_selections()
                .get(occurrence)
                .filter(|selection| authored_root_grant_selection(**selection))
                .map(|_| occurrence)
        })
        .collect::<Vec<_>>();
    let [occurrence] = occurrences.as_slice() else {
        return Err(Diagnostic::error(format!(
            "build boundary grant expression has {} exact authored selection occurrences",
            occurrences.len(),
        )));
    };
    authored_root_grant_selection_span(typed, *occurrence)
}

fn authored_root_grant_selection_span(
    typed: &TypedTrees,
    occurrence: language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
) -> Result<source::SourceSpan, Diagnostic> {
    let selection = typed
        .authored_declaration_selections()
        .get(occurrence)
        .filter(|selection| authored_root_grant_selection(**selection))
        .ok_or_else(|| {
            Diagnostic::error("build boundary grant has no exact authored selection evidence")
        })?;
    let span = selection.source_span();
    if span.span.start >= span.span.end {
        return Err(Diagnostic::error(
            "build boundary grant has an empty authored source span",
        ));
    }
    Ok(span)
}

fn authored_root_grant_selection(
    selection: language_semantics::declaration_selection::AuthoredDeclarationSelection,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    selection.kind() == Kind::Call
        && matches!(
            selection.target(),
            Target::LateBound(LateBinding::CheckedCall)
                | Target::Intrinsic(Intrinsic::BuildBoundaryAcceptance)
        )
}
