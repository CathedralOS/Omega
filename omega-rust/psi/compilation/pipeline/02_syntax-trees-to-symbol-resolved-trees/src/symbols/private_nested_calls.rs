//! Refuse a qualified call into another machine's nested helper.
//!
//! A nested machine is unavailable outside the machine whose scope declares it,
//! and qualification cannot bypass that rule
//! (`wiki/spec/language/machines.md#nested-machines-and-identity`). Call
//! resolution already leaves `outer::helper(..)` unselected: a machine receiver
//! names no methods. Left alone, that absence surfaced wherever a later stage
//! first needed the callee (a dispatch-subject storage type, a generic static
//! argument, a receiver-method hint), never naming the rule. This scan runs once
//! the bodies' symbols are assigned and reports the rule at the call, for every
//! carrier: expression calls, statement calls and terminal transition targets.
//! The expression and statement arenas are flat, so no body walk is needed.

use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{Statement, TransitionTarget};
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

pub(super) fn reject_private_nested_calls(
    program: &SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let symbols = &program.symbols;
    let expressions = &program.tables.bodies.expressions;
    let declarations = &program.tables.declarations;
    let mut diagnostics = Vec::new();
    for (_, node) in expressions.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if call.target_symbol.is_valid() || !call.receiver.is_valid() {
            continue;
        }
        let ExpressionNode::Name(path) = expressions.expression(call.receiver) else {
            continue;
        };
        let receiver = if path.symbol.is_valid() {
            path.symbol
        } else {
            path.head_symbol
        };
        report(symbols, receiver, &call.target, &mut diagnostics);
    }
    for (_, statement) in declarations.state_statements.iter() {
        match statement {
            Statement::Call(call) if !call.target_symbol.is_valid() => {
                report(
                    symbols,
                    call.receiver_symbol,
                    &call.target,
                    &mut diagnostics,
                );
            }
            Statement::Transition(transition) => {
                for target in
                    std::iter::once(&transition.target).chain(transition.continuation.as_ref())
                {
                    let TransitionTarget::Named(named) = target else {
                        continue;
                    };
                    let path = declarations
                        .statement_path_members
                        .span_or_empty(named.path);
                    if let [_, .., leaf] = path {
                        report(symbols, named.receiver_symbol, leaf, &mut diagnostics);
                    }
                }
            }
            _ => {}
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn report(
    symbols: &SymbolTable,
    receiver: SymbolHandle,
    target: &DiagnosticName,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !receiver.is_valid()
        || symbols.get(receiver).kind != SymbolKind::Machine
        || symbols
            .find_child_by_name_and_kind(receiver, target.as_str(), SymbolKind::Machine)
            .is_none()
    {
        return;
    }
    let owner = symbols.name(receiver);
    diagnostics.push(
        Diagnostic::error(format!(
            "call `{owner}::{}` names a nested machine private to `{owner}`; a nested machine is \
             called by its bare name inside its enclosing machine and is unavailable outside it",
            target.as_str(),
        ))
        .with_source_span(target.source_span()),
    );
}
