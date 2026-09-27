//! Exact self-tail calls become the same state backedge as an authored jump.
//!
//! This runs after reference resolution: a namespace is not a runtime receiver,
//! and a same-spelled machine is not the current entry. Ranking governs progress,
//! independently of this constant-stack normalization. Arguments retain their
//! existing order and handles; later arrival, ownership and ranking checks still
//! validate the edge. Calls nested inside a computation remain ordinary calls.

use arena::{Arena, HandleSpan};
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::statement::{
    NamedTransitionTarget, NamedTransitionTargetStorage, Statement, Transition, TransitionGuard,
    TransitionTarget,
};
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::scope::MachineScope;

pub(super) fn normalize_self_tail_calls(
    machine: &MachineScope<'_>,
    symbols: &SymbolTable,
    entry_symbol: SymbolHandle,
    entry_has_self: bool,
    state_has_self: bool,
    expressions: &ExpressionTable,
    path_members: &mut Arena<DiagnosticName>,
    statements: &mut [Statement],
) {
    let last_index = statements.len().checked_sub(1);
    for (index, statement) in statements.iter_mut().enumerate() {
        match statement {
            Statement::Expression(expression) if Some(index) == last_index => {
                if let Some(target) = self_tail_target(
                    machine,
                    symbols,
                    entry_symbol,
                    entry_has_self,
                    state_has_self,
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
                        source_span,
                    });
                }
            }
            Statement::Transition(transition) => {
                for target in std::iter::once(&mut transition.target)
                    .chain(transition.continuation.iter_mut())
                {
                    if let TransitionTarget::Value(expression) = target
                        && let Some(named) = self_tail_target(
                            machine,
                            symbols,
                            entry_symbol,
                            entry_has_self,
                            state_has_self,
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

fn self_tail_target(
    machine: &MachineScope<'_>,
    symbols: &SymbolTable,
    entry_symbol: SymbolHandle,
    entry_has_self: bool,
    state_has_self: bool,
    expressions: &ExpressionTable,
    path_members: &mut Arena<DiagnosticName>,
    expression: ExpressionHandle,
) -> Option<NamedTransitionTarget> {
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return None;
    };
    if !entry_symbol.is_valid()
        || (call.target_symbol != entry_symbol && call.target_symbol != machine.symbol)
        || !call.machine_arguments.is_empty()
        || call.operational_acknowledgement.acknowledges_suspend
        || call.operational_acknowledgement.acknowledges_block
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
    // A declaration-qualified static call has no receiver value to transport.
    // An actual receiver must be this activation's exact self; never turn a
    // call on another instance into a jump that silently retains this one.
    let receiver_matches = if !call.receiver.is_valid() {
        !entry_has_self
    } else if let ExpressionNode::Name(receiver) = expressions.expression(call.receiver) {
        if entry_has_self {
            state_has_self && receiver.is_self_value && receiver.symbol == machine.symbol
        } else {
            machine.attached_data_symbol.is_valid()
                && receiver.symbol == machine.attached_data_symbol
        }
    } else {
        false
    };
    if !receiver_matches {
        return None;
    }
    let mut path = HandleSpan::empty();
    path_members.append_to_span(&mut path, call.target.clone());
    Some(NamedTransitionTarget {
        head_symbol: entry_symbol,
        symbol: entry_symbol,
        storage: NamedTransitionTargetStorage {
            path,
            path_starts_at_self: false,
            arguments: call.arguments,
            evidence_arguments: call.evidence_arguments.clone(),
            source_span: call.target.source_span(),
            authored_call_selection: None,
        },
    })
}
