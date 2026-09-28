//! Reconstruct authored trust grants and their exact source occurrences.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use typed_trees::TypedTrees;

/// The static grant harvest: every `accept_boundary#<path>` marker call in
/// the build machine's states (the postfix carve's desugar of
/// `b.accept_boundary<path>();`). Order-preserving, deduplicated.
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
            let handles: Vec<typed_trees::expression::ExpressionHandle> = match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    vec![*expression]
                }
                typed_trees::statement::StatementNode::Call(call) => {
                    // A statement-level call keeps the marker in its target.
                    if let Some(path) =
                        BuildOperation::BoundaryAcceptance.marker_operands(call.target.as_str())
                    {
                        record(path, authored_root_grant_statement_span(typed, call)?);
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for handle in handles {
                if let typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(handle)
                    && let Some(path) =
                        BuildOperation::BoundaryAcceptance.marker_operands(call.target.as_str())
                {
                    record(path, authored_root_grant_expression_span(typed, handle)?);
                }
            }
        }
    }
    Ok(grants)
}

fn authored_root_grant_statement_span(
    typed: &TypedTrees,
    call: &typed_trees::statement::TableCall,
) -> Result<source::SourceSpan, Diagnostic> {
    let Some(occurrence) = call.authored_call_selection else {
        return Err(Diagnostic::error(
            "build boundary grant has no authored selection occurrence",
        ));
    };
    authored_root_grant_selection_span(typed, occurrence)
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
