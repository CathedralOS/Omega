//! The original machine owner resolves operands before a standalone value probe.

use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::types::ConstArgumentOrigin;

pub(super) fn retain(
    syntax: &SyntaxTrees,
    resolved: &SymbolResolvedTrees,
    root: ExpressionHandle,
) -> Result<Vec<ConstArgumentOrigin>, String> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut origins = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match syntax.expressions.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                    pending.push(arm.value);
                    if let syntax_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            ExpressionNode::Name(path) => {
                let members = syntax.expressions.identifier_path_members(*path);
                let (Some(first), Some(last)) = (members.first(), members.last()) else {
                    return Err("machine index operand lost its authored name path".to_owned());
                };
                let reference = SourceSpan::new(
                    first.source_span().source_id,
                    source::Span::new(first.source_span().span.start, last.source_span().span.end),
                );
                let origin = selected_constant(resolved, reference)?;
                if let Some(normalization) = syntax.root_items().find_map(|item| match item {
                    syntax_trees::item::Item::Const(definition)
                        if definition.name.source_span() == origin.declaration =>
                    {
                        definition.normalization.as_ref()
                    }
                    _ => None,
                }) {
                    for dependency in &normalization.selections {
                        if !origins.contains(dependency) {
                            origins.push(dependency.clone());
                        }
                    }
                }
                if !origins.contains(&origin) {
                    origins.push(origin);
                }
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => {}
        }
    }
    Ok(origins)
}

fn selected_constant(
    resolved: &SymbolResolvedTrees,
    reference: SourceSpan,
) -> Result<ConstArgumentOrigin, String> {
    let table = &resolved.tables.bodies.expressions;
    let mut origin = None;
    for (expression, node) in table.iter_expressions() {
        let mut matches_reference = false;
        if let symbol_resolved_trees::expression::ExpressionNode::Name(path) = node {
            let members = table.name_path_members(path.members);
            if let (Some(first), Some(last)) = (members.first(), members.last()) {
                matches_reference = first.source_span().source_id == reference.source_id
                    && first.source_span().span.start == reference.span.start
                    && last.source_span().span.end == reference.span.end;
            }
        }
        let mut selected_origin = None;
        for occurrence in table.authored_selection_occurrences(expression) {
            let selection = resolved
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("machine index lost its original declaration selection")?;
            if selection.source_span() != reference {
                continue;
            }
            matches_reference = true;
            let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
                continue;
            };
            let Some(declaration) = resolved
                .const_declarations
                .iter()
                .find(|declaration| declaration.symbol == selected.selected_symbol())
            else {
                continue;
            };
            let candidate = ConstArgumentOrigin {
                reference,
                declaration: resolved
                    .symbols
                    .symbol_source_span(declaration.symbol)
                    .ok_or("machine index constant lost its declaration source")?,
                initializer: declaration.initializer_source_span,
                canonical_value_encoding: declaration
                    .canonical_value_encoding
                    .clone()
                    .ok_or("machine index constant has no canonical value")?,
            };
            if selected_origin
                .as_ref()
                .is_some_and(|previous| previous != &candidate)
            {
                return Err("machine index operand has conflicting declaration custody".to_owned());
            }
            selected_origin = Some(candidate);
        }
        if !matches_reference {
            continue;
        }
        // Runtime locals, parameters and open binders intentionally do not
        // publish a constant selection. They cannot disappear from this check.
        let selected_origin = selected_origin.ok_or(
            "machine index operand must select a constant in its original lexical scope; runtime locals, parameters and open binders cannot be evaluated",
        )?;
        if origin
            .as_ref()
            .is_some_and(|previous| previous != &selected_origin)
        {
            return Err("machine index operand has conflicting lexical owners".to_owned());
        }
        origin = Some(selected_origin);
    }
    origin.ok_or_else(|| "machine index operand lost its original lexical occurrence".to_owned())
}
