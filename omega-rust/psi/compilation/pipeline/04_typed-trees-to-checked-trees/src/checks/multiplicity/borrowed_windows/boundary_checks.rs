//! boundary checks slice of borrowed_windows.

use super::{StorageWalk, place_spelling};

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// The expression coordinate is checked evidence, not permission to relocate
/// an invocation outside a restoration window. Rejoin it to the authored call
/// ordinal before occurrence-ordered replay uses the row. Retired calls belong
/// to their selected replacements and are not executed again here.
pub(in crate::checks::multiplicity) fn check_call_occurrences(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    control: &checked_trees::FlowControlFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in calls {
        if control.is_retired(state.symbol, call) {
            continue;
        }
        let expected = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )
        .map(|site| match site {
            validation::CallSite::Expression { expression, .. } => expression,
            _ => ExpressionHandle::invalid(),
        });
        if expected != Some(call.authored_expression) {
            diagnostics.push(Diagnostic::error(format!(
                "borrowed-storage call occurrence does not match statement {} invocation {} in `{}`",
                call.statement_index, call.call_ordinal, machine.name.as_str(),
            )));
        }
    }
}

/// Complete invocation checks after selected cleanup closes the effect graph.
/// Source recording retained each actual open frontier, including match-arm
/// occurrences. This consumes final call effects without reapplying any move
/// or reconstructing restoration from the later, repaired storage state.
pub(in crate::checks::multiplicity) fn check_recorded_invocations(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for (_, invocation) in facts.contextual_cleanup.window_invocations.iter() {
        let Some(machine) = program.machine_by_symbol(invocation.machine) else {
            return Err(vec![Diagnostic::error(
                "borrowed-storage invocation has no owning machine",
            )]);
        };
        let Some(state) = program.state_in_machine(machine, invocation.state) else {
            return Err(vec![Diagnostic::error(
                "borrowed-storage invocation has no owning state",
            )]);
        };
        let Some(flow) = facts
            .flow
            .control
            .state(invocation.machine, invocation.state)
        else {
            return Err(vec![Diagnostic::error(
                "borrowed-storage invocation has no final call flow",
            )]);
        };
        let calls = facts.flow.control.calls.span_or_empty(flow.calls);
        let selected = |call: &&checked_trees::FlowCallFact| {
            call.statement_index == invocation.statement
                && call.authored_expression == invocation.expression
                && !facts.flow.control.is_retired(state.symbol, call)
        };
        let may_suspend = calls.iter().filter(selected).any(|call| {
            call.suspension.direct_may_suspend
                || call.suspension.transitive_may_suspend
                || call.blocking.direct_may_block
                || call.blocking.transitive_may_block
        });
        let may_enter_boundary = calls.iter().filter(selected).any(|call| {
            call_may_enter_boundary(
                program,
                &facts.service_reaches,
                &facts.flow.control,
                &facts.operators,
                state.symbol,
                call,
            )
        }) || state_has_boundary_operator(
            program,
            &facts.flow.control,
            &facts.operators,
            state.symbol,
            Some(invocation.statement),
            Some(invocation.expression),
        );
        if !may_suspend && !may_enter_boundary {
            continue;
        }
        let path = facts
            .contextual_cleanup
            .segments
            .span_or_empty(invocation.absent.segments);
        let spelling = place_spelling(program, invocation.absent.root, path);
        let extraction = invocation
            .opened_statement
            .map(|statement| format!(" at statement {statement}"))
            .unwrap_or_default();
        if may_suspend {
            diagnostics.push(Diagnostic::error(format!(
                "cannot suspend or block at statement {} while `{spelling}` is absent: restore the value moved out of borrowed storage{extraction} first",
                invocation.statement,
            )));
        }
        if may_enter_boundary {
            diagnostics.push(Diagnostic::error(format!(
                "cannot make a boundary or service call at statement {} while `{spelling}` is absent: restore the value moved out of borrowed storage{extraction} first",
                invocation.statement,
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Service reach and boundary invocation are different facts: an opaque
/// boundary may declare empty reach. Follow the retained exact call topology
/// through ordinary wrappers as well, rather than treating that empty row (or
/// an empty write frame, which says nothing about reads) as non-observation.
/// This query runs only for calls crossing an open window. Its local visited
/// set closes cycles without inventing a new effect row or expanding paths.
pub(super) fn call_may_enter_boundary(
    program: &dyn ::validation::CheckingInput,
    service_reaches: &checked_trees::ServiceReachFacts,
    control: &checked_trees::FlowControlFacts,
    operators: &checked_trees::CheckedOperatorFacts,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
) -> bool {
    let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
    // The row-table reader maps unknown IDs to an empty slice. Only the
    // canonical empty identity establishes absence of service reach.
    if !call.boundary_edges.is_empty()
        || call.service_reach.direct != empty
        || call.service_reach.transitive != empty
    {
        return true;
    }
    let Some(target) = service_reaches.for_state(state).and_then(|state| {
        service_reaches.calls_for(state).iter().find(|target| {
            target.statement_index == call.statement_index
                && target.call_ordinal == call.call_ordinal
                && target.target_state == call.target_symbol
        })
    }) else {
        return true;
    };
    let mut pending = vec![target];
    let mut visited = Vec::new();
    while let Some(call) = pending.pop() {
        if call.inferred_direct != empty || call.inferred_transitive != empty {
            return true;
        }
        // Compiler-owned builtins have no machine body. Resolve their exact
        // symbol, not the authored spelling; service-bearing assembly remains
        // a fence even if its retained reach row were missing that service.
        if let Some(builtin) = program
            .symbols
            .builtin_function_for_symbol(call.target_state)
        {
            if builtin.asm_intrinsic_service_name().is_some() {
                return true;
            }
            continue;
        }
        let target = call.target_machine;
        if visited.contains(&target) {
            continue;
        }
        visited.push(target);
        let Some(machine) = program.machine_by_symbol(target) else {
            return true;
        };
        if !machine.supply_mode.is_checked_body() || !machine.body_is_present {
            return true;
        }
        let Some(reach) = service_reaches.for_machine(target) else {
            return true;
        };
        for state in service_reaches.states_for(reach) {
            if state_has_boundary_operator(program, control, operators, state.state, None, None) {
                return true;
            }
            pending.extend(service_reaches.calls_for(state));
        }
    }
    false
}

/// Operator invocations are scheduled separately from ordinary calls. Use
/// those records for both named and spelled operators so skipped operands do
/// not become calls, and wrappers cannot hide an opaque operator invocation.
pub(in crate::checks::multiplicity) fn state_has_boundary_operator(
    program: &dyn ::validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    operators: &checked_trees::CheckedOperatorFacts,
    state: SymbolHandle,
    statement: Option<usize>,
    expression: Option<ExpressionHandle>,
) -> bool {
    control.operator_invocations.iter().any(|(_, invocation)| {
        let (origin, target, occurrence) = if invocation.named_use.is_valid() {
            let usage = operators.named_uses.get(invocation.named_use);
            (
                usage.origin,
                usage.selected_operator_symbol,
                usage.expression,
            )
        } else {
            let usage = operators.uses.get(invocation.operator_use);
            (
                usage.origin,
                usage.selected_operator_symbol,
                usage.expression,
            )
        };
        matches!(origin, checked_trees::CheckedValueOrigin::StateStatement {
            state_symbol, statement_index, ..
        } if state_symbol == state && statement.is_none_or(|expected| expected == statement_index))
            && expression.is_none_or(|expected| expected == occurrence)
            && typed_trees::operator::declaration_by_symbol(program, target)
                .is_some_and(|operator| operator.is_boundary)
    })
}

/// The shared diagnostic for a borrowed-storage transfer that cannot open or
/// complete a window.
pub(in crate::checks::multiplicity) fn borrowed_transfer_diagnostic(
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot transfer a non-copy value out of borrowed storage without replacing its \
         owner in `{}`, state `{}` (statement {})",
        machine.name.as_str(),
        state.name.as_str(),
        statement_index,
    ))
}

/// The exclusive-route half of [`BorrowedStorageWindows::open`] keyed on the
/// authored spelling: sibling arms may name the same referent storage through
/// different routes (a `&mut` local versus the owner path), so each arm's
/// route is checked on its own links even though the join opens one hole.
pub(in crate::checks::multiplicity) fn authored_route_exclusive(
    walk: &StorageWalk<'_>,
    statement_index: usize,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> bool {
    matches!(root, facts::PlaceRoot::Symbol(_))
        && walk.exclusive_storage_chain(statement_index, root, path)
}
