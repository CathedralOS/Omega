//! Source-certified static namespaces for every invocation carrier.
//!
//! Ordinary expressions, statement calls and terminal targets share declaration
//! selection, lexical shadowing and receiver erasure. Qualification retains its
//! source authority and selected callable; it contributes no runtime argument.

use crate::symbols::lookup;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind};

/// Resolve an authored static path through ordinary source-owned selection.
/// A package alias need not have a Module symbol: its loader-certified import
/// already selects the exact declaration owner. A machine stands
/// for its callable `entry` state, while an unresolved member still claims the
/// call with an invalid target so later passes see a selection, not a spelling.
/// Every invocation carrier applies this one law: keeping the
/// lookup in one place is what stops the two carriers from drifting apart.
fn static_call_target(
    symbols: &symbols::SymbolTable,
    receiver_name: &str,
    name: &str,
    reference: source::SourceSpan,
    lexical_head: SymbolHandle,
) -> Option<SymbolHandle> {
    // Fixed proof vocabulary owns its exact namespace before authored module
    // selection. This is the same closed-catalog rule for every call carrier.
    let fixed = symbols::FixedCoreDeclaration::from_name(name)
        .and_then(|declaration| symbols.fixed_core_declaration_symbol(declaration));
    if let Some(fixed) = fixed.filter(|symbol| symbol.is_valid()) {
        return Some(fixed);
    }
    // A lexical value cannot be reinterpreted as a same-spelled module.
    // Loader-certified package prefixes may have no declaration symbol.
    if lexical_head.is_valid() && symbols.get(lexical_head).kind != SymbolKind::Module {
        return None;
    }
    let namespace = symbols.find_top_level_by_name_and_kinds_from_source(
        receiver_name,
        &[SymbolKind::Module],
        reference,
    );
    if !namespace.is_some_and(|symbol| symbols.get(symbol).kind == SymbolKind::Module)
        && !symbols.is_source_package_qualifier(reference, receiver_name)
    {
        return None;
    }
    Some(
        symbols
            .find_top_level_by_name_and_kinds_from_source(name, super::SOURCE_CALL_KINDS, reference)
            .map(|symbol| {
                if symbols.get(symbol).kind == SymbolKind::Machine {
                    symbols
                        .find_child_by_name_and_kind(symbol, "entry", SymbolKind::State)
                        .unwrap_or_else(SymbolHandle::invalid)
                } else {
                    symbol
                }
            })
            .unwrap_or_else(SymbolHandle::invalid),
    )
}

pub(crate) fn normalize_static_module_calls(
    program: &mut SymbolResolvedTrees,
    calls: &[(
        symbol_resolved_trees::expression::ExpressionHandle,
        Vec<symbol_resolved_trees::name::DiagnosticName>,
    )],
    statement_calls: &[(
        source::SourceSpan,
        Vec<symbol_resolved_trees::name::DiagnosticName>,
    )],
) {
    use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbols::JoinedPath;
    for (expression, path) in calls {
        let Some((target, receiver)) = path.split_last() else {
            continue;
        };
        let receiver_name = receiver.joined_path();
        let reference = lookup::diagnostic_path_source_span(path);
        let name = format!("{receiver_name}::{}", target.as_str());
        let original_receiver = match program.tables.bodies.expressions.expression(*expression) {
            ExpressionNode::Call(call) => call.receiver,
            _ => continue,
        };
        let lexical_head = if original_receiver.is_valid() {
            program
                .tables
                .bodies
                .expressions
                .name_path_node(original_receiver)
                .map(|path| path.head_symbol)
                .unwrap_or_default()
        } else {
            SymbolHandle::invalid()
        };
        let Some(selected) = static_call_target(
            &program.symbols,
            &receiver_name,
            &name,
            reference,
            lexical_head,
        ) else {
            continue;
        };
        // Declaration indexing can already have normalized this occurrence.
        // A later body batch must not dereference its now-absent receiver.
        if original_receiver.is_valid()
            && let ExpressionNode::Name(original_path) = program
                .tables
                .bodies
                .expressions
                .expression(original_receiver)
                .clone()
        {
            let mut prefix = String::new();
            for (offset, member) in receiver.iter().enumerate() {
                if !prefix.is_empty() {
                    prefix.push_str("::");
                }
                prefix.push_str(member.as_str());
                let selected_module = program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &prefix,
                        &[SymbolKind::Module],
                        reference,
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
                program
                    .tables
                    .bodies
                    .expressions
                    .set_name_path_member_symbol_at_offset(
                        original_path.member_symbols,
                        offset as u32,
                        selected_module,
                    );
            }
        }
        if let ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression_mut(*expression)
        {
            call.receiver = ExpressionHandle::invalid();
            call.target = symbol_resolved_trees::name::DiagnosticName::from_str(&name, reference);
            call.target_symbol = selected;
        }
    }
    let symbols = &program.symbols;
    let declarations = &mut program.tables.declarations;
    let members = &mut declarations.statement_path_members;
    declarations
        .state_statements
        .for_each_mut(|_, statement| match statement {
            symbol_resolved_trees::statement::Statement::Call(call) => {
                let Some((_, path)) = statement_calls
                    .iter()
                    .find(|(span, _)| *span == call.target.source_span())
                else {
                    return;
                };
                let Some((target, receiver)) = path.split_last() else {
                    return;
                };
                let receiver_name = receiver.joined_path();
                let reference = lookup::diagnostic_path_source_span(path);
                let name = format!("{receiver_name}::{}", target.as_str());
                let Some(selected) = static_call_target(
                    symbols,
                    &receiver_name,
                    &name,
                    reference,
                    call.receiver_root_symbol,
                ) else {
                    return;
                };
                call.receiver = arena::HandleSpan::empty();
                call.receiver_symbol = SymbolHandle::invalid();
                call.receiver_root_symbol = SymbolHandle::invalid();
                call.target =
                    symbol_resolved_trees::name::DiagnosticName::from_str(&name, reference);
                call.target_symbol = selected;
            }
            symbol_resolved_trees::statement::Statement::Transition(transition) => {
                normalize_terminal_namespace(symbols, members, &mut transition.target);
                if let Some(continuation) = &mut transition.continuation {
                    normalize_terminal_namespace(symbols, members, continuation);
                }
            }
            _ => {}
        });
}

/// Static qualification is settled before typed call construction. Keep the
/// original authored occurrence and source span; the runtime path has only its
/// selected callable coordinate, just as an ordinary normalized call does.
fn normalize_terminal_namespace(
    symbols: &symbols::SymbolTable,
    members: &mut arena::Arena<symbol_resolved_trees::name::DiagnosticName>,
    target: &mut symbol_resolved_trees::statement::TransitionTarget,
) {
    use symbol_resolved_trees::statement::TransitionTarget;
    use symbols::JoinedPath;
    let TransitionTarget::Named(named) = target else {
        return;
    };
    let path = members.span_or_empty(named.path);
    let Some((target, receiver)) = path
        .split_last()
        .filter(|(_, receiver)| !receiver.is_empty())
    else {
        return;
    };
    let receiver_name = receiver.joined_path();
    let reference = lookup::diagnostic_path_source_span(path);
    let name = format!("{receiver_name}::{}", target.as_str());
    let Some(selected) =
        static_call_target(symbols, &receiver_name, &name, reference, named.head_symbol)
    else {
        return;
    };
    let invocation = super::transitions::terminal_call_target(symbols, selected);
    let mut canonical = arena::HandleSpan::empty();
    members.append_to_span(
        &mut canonical,
        symbol_resolved_trees::name::DiagnosticName::from_str(&name, reference),
    );
    named.path = canonical;
    named.symbol = invocation;
    named.head_symbol = invocation;
    named.receiver_symbol = SymbolHandle::invalid();
}
