//! Bind a selected token-bearing machine to its own checked body.
//!
//! The executable-supply contract in wiki/spec/language/expressions.md says an
//! ordinary direct `machine + Name(...) { ... }` is supplied by its own checked
//! body, through ordinary call machinery, with no operator interpreter and no
//! satisfier search. Operand-directed selection (`resolve_spelling_for_operands`
//! over the typed `machine_token_bindings` views) settles which declaration a
//! spelled use means; this pass turns that settled meaning into the ordinary
//! call the author could have written by hand -- `left + right` becomes a
//! `Call` on `Wrapped::add`'s entry state with the operands as its arguments,
//! in operand order, evaluated once each.
//!
//! It runs at the checked stage rather than at typing because operand types
//! are only settled by the checked value facts; typing rewrites `==` to a
//! written `equals` from declared data alone, which is not enough to choose
//! between overloaded token bindings. It runs before program validation so
//! ownership, effects, termination, contracts, and every executing consumer
//! (interpreter, lowering, Terminal, native) see a plain call edge to the
//! declaration -- the same shape build-time evaluation forms for selected
//! boundary providers (`build-time-evaluation/.../selected_operators.rs`). The
//! rewritten node keeps its authored `Operator` selection occurrence, which
//! finalization settles to the machine symbol
//! (`authored_selections/finalization.rs`), so the checked artifact retains
//! the authored token occurrence together with the exact declaration.
//!
//! Only binary spellings at expression occurrences bind here. Any other
//! resolved selection of a token-bearing machine -- `[]`, `[..]`, or a `==`
//! folded into match-arm equality -- rejects at this pass: leaving the
//! selection fact without a body binding would let a later consumer treat the
//! operand primitives as builtin arithmetic, which the contract forbids.

use checked_trees::{CheckedOperatorOccurrence, CheckedOperatorResolutionStatus};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};

/// Rewrite every resolved binary use of a token-bearing machine into an
/// ordinary call on that machine's entry state.
pub(crate) fn bind_token_bound_machine_calls(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    if program.machine_token_bindings().is_empty() {
        return Ok(());
    }
    let mut facts = crate::derive_pre_flow_operator_selections(program);
    // A domain-homed binding (`machine + Quantity::Additive::add`) is a
    // `DomainPending` candidate until binding-site selection settles it. That
    // selection reads only static operand qualifications, mints, and
    // signature `requires` -- never flow facts -- so it can run here, before
    // validation, and an unselected domain meaning keeps the builtin surface
    // exactly as a domain-homed `operator` declaration did.
    crate::operators::select_pending_domain_operator_meanings(program, &mut facts);
    let mut diagnostics = Vec::new();
    let mut bindings: Vec<(ExpressionHandle, SymbolHandle)> = Vec::new();
    for operator_use in facts.uses_with_status(CheckedOperatorResolutionStatus::Resolved) {
        let selected = operator_use.selected_operator_symbol;
        if !program
            .machine_token_bindings()
            .iter()
            .any(|view| view.symbol == selected)
        {
            continue;
        }
        let expression = operator_use.expression;
        // One expression can surface under several value origins (a shared
        // template body, for example); its body binding happens once.
        if bindings.iter().any(|(bound, _)| *bound == expression) {
            continue;
        }
        let is_binary_expression = operator_use.occurrence == CheckedOperatorOccurrence::Expression
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Binary(_)
            );
        if !is_binary_expression {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` was selected for its fixed operator token `{}` in a position whose body supply is not implemented; only binary expression uses bind the declaration's own body",
                machine_name(program, selected),
                operator_use.spelling.symbol(),
            )));
            continue;
        }
        bindings.push((expression, selected));
    }

    for (expression, machine_symbol) in bindings {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            unreachable!("binding candidates are binary expressions");
        };
        let operands = [binary.left, binary.right];
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
        else {
            diagnostics.push(Diagnostic::error(
                "a selected token-bearing machine lost its typed declaration before body binding",
            ));
            continue;
        };
        let Some(entry) = program.machine_states(machine).first() else {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` was selected for its fixed operator token but declares no entry state",
                machine.name
            )));
            continue;
        };
        let (entry_symbol, target) = (entry.symbol, machine.name.clone());
        let arguments = program.expression_table.insert_expression_handles(operands);
        *program.expression_table.expression_mut(expression) =
            ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: entry_symbol,
                static_machine_parameter: SymbolHandle::invalid(),
                target,
                static_requirement_dispatch: None,
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: language_semantics::CallOperationalAcknowledgement {
                    origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                    ..Default::default()
                },
            });
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The token-bearing machine whose entry state a compiler-synthesized call
/// targets, if the call is one bound by [`bind_token_bound_machine_calls`].
/// Finalization uses it to settle the retained `Operator` occurrence on the
/// rewritten node to the exact declaration.
pub(crate) fn token_bound_machine_call_target(
    program: &TypedTrees,
    call: &TableCallExpression,
) -> Option<SymbolHandle> {
    if call.operational_acknowledgement.origin
        != language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
        || !call.target_symbol.is_valid()
    {
        return None;
    }
    program
        .machines()
        .iter()
        .filter(|machine| machine.spelling.is_some())
        .find(|machine| {
            program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
        })
        .map(|machine| machine.symbol)
}

fn machine_name(program: &TypedTrees, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map_or_else(
            || "<unknown machine>".to_owned(),
            |machine| machine.name.to_string(),
        )
}
