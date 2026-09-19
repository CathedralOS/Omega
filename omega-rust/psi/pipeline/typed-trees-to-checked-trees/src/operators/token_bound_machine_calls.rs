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
//! Binary and indexed spellings at expression occurrences bind here. `[]`
//! (and `[..]` with both authored bounds and an exclusive end) supplies the
//! collection place as the call receiver when the entry state's first
//! parameter is `self`, matching the loan a named `collection.at(index)`
//! forms; a token binding whose first operand is an ordinary parameter keeps
//! the operands as ordinary arguments. Open or inclusive range uses, and any
//! other resolved selection of a token-bearing machine -- a `==` folded into
//! match-arm equality, say -- reject at this pass: leaving the selection fact
//! without a body binding would let a later consumer treat the operand
//! primitives as builtin arithmetic, which the contract forbids.

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
        let bindable = operator_use.occurrence == CheckedOperatorOccurrence::Expression
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Binary(_) | ExpressionNode::Indexed(_)
            );
        if !bindable {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` was selected for its fixed operator token `{}` in a position whose body supply is not implemented; only binary and indexed expression uses bind the declaration's own body",
                machine_name(program, selected),
                operator_use.spelling.symbol(),
            )));
            continue;
        }
        bindings.push((expression, selected));
    }

    for (expression, machine_symbol) in bindings {
        let operands = match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
            ExpressionNode::Indexed(indexed) => {
                let mut operands = vec![indexed.collection];
                match program.expression_table.expression(indexed.index) {
                    // `a[..]`/`a[start..]`/`a[..=end]` leave the token binding
                    // without every bound it declared; inclusive ends need the
                    // `end + 1` normalization the spec assigns to `[..]`,
                    // which is not yet formed here.
                    ExpressionNode::Range(range)
                        if range.end_inclusive
                            || !range.start.is_valid()
                            || !range.end.is_valid() =>
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "`{}` was selected for its fixed operator token `[..]` but the range use is open or inclusive; only `start..end` uses bind the declaration's own body",
                            machine_name(program, machine_symbol),
                        )));
                        continue;
                    }
                    ExpressionNode::Range(range) => {
                        operands.push(range.start);
                        operands.push(range.end);
                    }
                    _ => operands.push(indexed.index),
                }
                operands
            }
            _ => unreachable!("binding candidates are binary or indexed expressions"),
        };
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
        let first_is_self = program
            .state_parameters(entry)
            .first()
            .is_some_and(|parameter| parameter.is_self);
        // A `self` first operand is the receiver: the collection place takes
        // the same loan a named `collection.at(index)` call forms. Otherwise
        // operand zero is an ordinary argument like any binary operand.
        let (receiver, argument_handles) = if first_is_self {
            (operands[0], &operands[1..])
        } else {
            (ExpressionHandle::invalid(), &operands[..])
        };
        let arguments = program
            .expression_table
            .insert_expression_handles(argument_handles.iter().copied());
        *program.expression_table.expression_mut(expression) =
            ExpressionNode::Call(TableCallExpression {
                receiver,
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
