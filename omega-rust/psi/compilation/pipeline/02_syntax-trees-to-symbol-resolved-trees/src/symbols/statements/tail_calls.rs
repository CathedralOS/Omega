//! Exact recursive tail calls retain invocation admission on a constant-stack edge.
//!
//! This runs after reference resolution: a namespace is not a runtime receiver,
//! and a same-spelled machine is not the current entry. Ranking governs progress,
//! independently of this constant-stack normalization. Arguments retain their
//! existing order and handles; later arrival, ownership and ranking checks still
//! validate the edge. The machine target keeps recursive invocation contracts
//! distinct from internal state arrivals; exact authored markers remain until
//! operational checking. Calls nested in computations remain ordinary calls.

use arena::{Arena, HandleSpan};
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{
    NamedTransitionTarget, NamedTransitionTargetStorage, Statement, Transition, TransitionGuard,
    TransitionTarget,
};
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::scope::MachineScope;

pub(super) fn normalize_recursive_tail_calls(
    machine: &MachineScope<'_>,
    symbols: &SymbolTable,
    entry_symbol: SymbolHandle,
    expressions: &ExpressionTable,
    path_members: &mut Arena<DiagnosticName>,
    statements: &mut [Statement],
) {
    let last_index = statements.len().checked_sub(1);
    for (index, statement) in statements.iter_mut().enumerate() {
        match statement {
            Statement::Expression(expression) if Some(index) == last_index => {
                if let Some(target) = recursive_tail_target(
                    machine,
                    symbols,
                    entry_symbol,
                    expressions,
                    path_members,
                    *expression,
                ) {
                    let source_span = target.source_span;
                    *statement = Statement::Transition(Transition {
                        target: TransitionTarget::Named(target),
                        continuation: None,
                        guard: TransitionGuard::Always,
                        proof_selectors: Box::default(),
                        exit: Default::default(),
                        ends_dispatch: true,
                        source_span,
                    });
                }
            }
            Statement::Transition(transition) => {
                for target in std::iter::once(&mut transition.target)
                    .chain(transition.continuation.iter_mut())
                {
                    if let TransitionTarget::Value(expression) = target
                        && let Some(named) = recursive_tail_target(
                            machine,
                            symbols,
                            entry_symbol,
                            expressions,
                            path_members,
                            *expression,
                        )
                    {
                        *target = TransitionTarget::Named(named);
                    }
                }
            }
            _ => {}
        }
    }
}

fn recursive_tail_target(
    machine: &MachineScope<'_>,
    symbols: &SymbolTable,
    entry_symbol: SymbolHandle,
    expressions: &ExpressionTable,
    path_members: &mut Arena<DiagnosticName>,
    expression: ExpressionHandle,
) -> Option<NamedTransitionTarget> {
    let call = expressions.call_node(expression)?;
    if !entry_symbol.is_valid()
        || (call.target_symbol != entry_symbol && call.target_symbol != machine.symbol)
        || !call.machine_arguments.is_empty()
    {
        return None;
    }
    // A result-overload family's binding is provisional until typed destination
    // selection. Keep those calls intact instead of freezing the first member.
    if machine.attached_data_symbol.is_valid()
        && machine
            .attached_machines
            .positions_for_owner(machine.attached_data_symbol)
            .iter()
            .filter(|position| {
                crate::symbols::lookup::child_symbol_by_kinds(
                    symbols,
                    machine.attached_machines.list[**position].machine,
                    &[SymbolKind::State],
                    call.target.as_str(),
                )
                .is_valid()
            })
            .take(2)
            .count()
            != 1
    {
        return None;
    }
    // A declaration qualifier transports no value. Runtime binding dispatch
    // remains an ordinary call; a backedge carries only its explicit arguments.
    if call.receiver.is_valid()
        && !matches!(expressions.expression(call.receiver), ExpressionNode::Name(receiver)
            if machine.attached_data_symbol.is_valid()
                && receiver.symbol == machine.attached_data_symbol)
    {
        return None;
    }
    let mut path = HandleSpan::empty();
    path_members.append_to_span(&mut path, call.target.clone());
    Some(NamedTransitionTarget {
        head_symbol: machine.symbol,
        symbol: machine.symbol,
        storage: NamedTransitionTargetStorage {
            operational_acknowledgement: call.operational_acknowledgement,
            path,
            arguments: call.arguments,
            evidence_arguments: call.evidence_arguments.clone(),
            source_span: call.target.source_span(),
            authored_call_selection: None,
        },
    })
}
