//! The original machine owner resolves operands before a standalone value probe.

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
};
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::types::ConstArgumentOrigin;

pub(crate) fn retain(
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
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Indexed(indexed) => {
                pending.push(indexed.index);
                pending.push(indexed.collection);
            }
            ExpressionNode::Call(call) => {
                pending.extend(
                    syntax
                        .expressions
                        .expression_handles(call.arguments)
                        .iter()
                        .rev()
                        .copied(),
                );
                if call.receiver.is_valid()
                    && !selected_call_has_namespace_receiver(syntax, resolved, call)?
                {
                    pending.push(call.receiver);
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

/// Retain lexical call selections even in branches which will not execute.
/// Invocation admission and concrete premise discharge belong to the probe.
pub(crate) fn retain_calls(
    syntax: &SyntaxTrees,
    resolved: &SymbolResolvedTrees,
    root: ExpressionHandle,
) -> Result<Vec<(SourceSpan, SourceSpan)>, String> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut calls = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match syntax.expressions.expression(expression) {
            ExpressionNode::Call(call) => {
                let selected = selected_call(syntax, resolved, call)?;
                let reference = selected.target.source_span();
                let symbol = resolved.symbols.get(selected.target_symbol);
                if symbol.kind != SymbolKind::Machine
                    && !(symbol.kind == SymbolKind::State
                        && resolved.symbols.get(symbol.parent).kind == SymbolKind::Machine)
                {
                    return Err(
                        "machine index call must select an ordinary machine or state".into(),
                    );
                }
                let declaration = resolved
                    .symbols
                    .symbol_source_span(selected.target_symbol)
                    .or_else(|| {
                        // Generated entry states retain their exact machine owner,
                        // rather than acquiring an invented authored declaration.
                        if symbol.kind != SymbolKind::State
                            || resolved.symbols.get(symbol.parent).kind != SymbolKind::Machine
                        {
                            return None;
                        }
                        let mut owners = resolved
                            .machines
                            .iter()
                            .filter(|owner| owner.symbol == symbol.parent);
                        let owner = owners.next()?;
                        if owners.next().is_some() {
                            return None;
                        }
                        let storage = &resolved.tables.declarations;
                        let states = storage.machine_state_handles.span_or_empty(owner.states);
                        if states.len() != owner.states.len()
                            || storage.machine_states.get(*states.first()?).symbol
                                != selected.target_symbol
                        {
                            return None;
                        }
                        resolved.symbols.symbol_source_span(symbol.parent)
                    })
                    .ok_or("machine index call lost its exact selected declaration source")?;
                let selection = (reference, declaration);
                if !calls.contains(&selection) {
                    calls.push(selection);
                }
                pending.extend(
                    syntax
                        .expressions
                        .expression_handles(call.arguments)
                        .iter()
                        .rev()
                        .copied(),
                );
                if call.receiver.is_valid()
                    && !selected_call_has_namespace_receiver(syntax, resolved, call)?
                {
                    pending.push(call.receiver);
                }
            }
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                    pending.push(arm.value);
                    if let syntax_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Indexed(indexed) => {
                pending.push(indexed.index);
                pending.push(indexed.collection);
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => {}
        }
    }
    Ok(calls)
}

fn selected_call<'resolved>(
    syntax: &SyntaxTrees,
    resolved: &'resolved SymbolResolvedTrees,
    authored: &syntax_trees::expression::TableCallExpression,
) -> Result<&'resolved symbol_resolved_trees::expression::TableCallExpression, String> {
    let reference = authored.target.source_span();
    // Module-call normalization removes the namespace receiver and records
    // the complete authored path as the call occurrence, not just its leaf.
    let qualified_reference = match syntax.expressions.expression(authored.receiver) {
        ExpressionNode::Name(path) => syntax
            .expressions
            .identifier_path_members(*path)
            .first()
            .filter(|first| first.source_span().source_id == reference.source_id)
            .map(|first| {
                SourceSpan::new(
                    reference.source_id,
                    source::Span::new(first.source_span().span.start, reference.span.end),
                )
            }),
        _ => None,
    };
    let table = &resolved.tables.bodies.expressions;
    let mut selected_call = None;
    for (expression, node) in table.iter_expressions() {
        let symbol_resolved_trees::expression::ExpressionNode::Call(call) = node else {
            continue;
        };
        let selected_reference = call.target.source_span();
        if selected_reference != reference
            && !(qualified_reference == Some(selected_reference) && !call.receiver.is_valid())
        {
            continue;
        }
        let mut exact_selection = false;
        for occurrence in table.authored_selection_occurrences(expression) {
            let selection = resolved
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("machine index call lost its lexical selection occurrence")?;
            if selection.kind() != AuthoredDeclarationSelectionKind::Call
                || selection.source_span() != selected_reference
            {
                continue;
            }
            if !matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
                if target.selected_symbol() == call.target_symbol)
            {
                return Err("machine index call has no exact original lexical target".into());
            }
            exact_selection = true;
        }
        if !exact_selection {
            return Err("machine index call lost its original lexical selection".into());
        }
        if selected_call.is_some_and(
            |previous: &symbol_resolved_trees::expression::TableCallExpression| {
                previous.target_symbol != call.target_symbol
                    || has_namespace_receiver(resolved, previous)
                        != has_namespace_receiver(resolved, call)
            },
        ) {
            return Err("machine index call has conflicting original lexical owners".into());
        }
        selected_call = Some(call);
    }
    selected_call.ok_or_else(|| "machine index call lost its original lexical occurrence".into())
}

fn selected_call_has_namespace_receiver(
    syntax: &SyntaxTrees,
    resolved: &SymbolResolvedTrees,
    authored: &syntax_trees::expression::TableCallExpression,
) -> Result<bool, String> {
    let call = selected_call(syntax, resolved, authored)?;
    Ok((authored.receiver.is_valid() && !call.receiver.is_valid())
        || has_namespace_receiver(resolved, call))
}

fn has_namespace_receiver(
    resolved: &SymbolResolvedTrees,
    call: &symbol_resolved_trees::expression::TableCallExpression,
) -> bool {
    if !call.receiver.is_valid() {
        return false;
    }
    let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
        resolved.tables.bodies.expressions.expression(call.receiver)
    else {
        return false;
    };
    matches!(
        resolved.symbols.get(path.symbol).kind,
        SymbolKind::Module
            | SymbolKind::Data
            | SymbolKind::Domain
            | SymbolKind::BuiltinType
            | SymbolKind::Machine
            | SymbolKind::Trait
    )
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
