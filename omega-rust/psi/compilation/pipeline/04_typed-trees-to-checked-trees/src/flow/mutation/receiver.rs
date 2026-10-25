use validation::CallSite;
use validation::CanonicalPlace;
use validation::call_site_argument_expressions;
use validation::canonical_place_from_expression;
use validation::canonical_place_from_expression_in_state;
use validation::find_call_site;
use validation::find_state_in_machine;

use crate::flow::canonical_place_from_symbol;
use crate::proof::contract_target_from_state_symbol;

use checked_trees::{BorrowCallFact, BorrowFacts};
use symbols::SymbolHandle;

pub(crate) fn call_receiver_is_mutable(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
) -> bool {
    let Some((target_machine_symbol, target_state_symbol)) =
        contract_target_from_state_symbol(program, borrow_call.target_symbol)
    else {
        return false;
    };
    if find_state_in_machine(program, target_machine_symbol, target_state_symbol).is_none() {
        return false;
    }
    borrow_call.accesses.is_empty()
        && borrow.with_state_borrows(target_machine_symbol, target_state_symbol, |rows| {
            rows.iter()
                .any(|handle| borrow.states.get(*handle).mutable_parameter_count > 0)
        })
}

pub(crate) fn call_receiver_mutated_place(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> Option<CanonicalPlace> {
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    canonical_receiver_place_for_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        &call_site,
        borrow_call.statement_index,
    )
}

/// `statement_index` is the occurrence's recorded statement coordinate — the
/// same index `find_call_site` resolved or the caller's own traversal
/// recorded — so projected-receiver scope is derived from the recorded row,
/// never from where the call node happens to sit in this arena slice. A
/// replayed site carrying the same recorded coordinate reaches the same
/// place; see `receiver_place_for_call_site`.
pub(crate) fn canonical_receiver_place_for_call_site(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    call_site: &CallSite<'_>,
    statement_index: usize,
) -> Option<CanonicalPlace> {
    let place = receiver_place_for_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        call_site,
        statement_index,
    )?;
    Some(place)
}

fn receiver_place_for_call_site(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    call_site: &CallSite<'_>,
    statement_index: usize,
) -> Option<CanonicalPlace> {
    match call_site {
        CallSite::Statement(statement) => {
            let state = program.state_by_symbol(caller_state_symbol)?;
            let statements = program.statement_table.statements(state.statement_nodes);
            // The projected-receiver scope boundary is the occurrence's
            // recorded statement coordinate, not the address the row occupies
            // in this arena slice — pointer identity is unreplayable. The
            // coordinate is honored only when the recorded statement at that
            // index is this exact call payload, so a replayed copy of the same
            // row resolves the same boundary while a coordinate naming a
            // different statement declines to the receiver-symbol fallback.
            if let Some(typed_trees::statement::StatementNode::Call(candidate)) =
                statements.get(statement_index)
                && *candidate == **statement
                && let Some((root, segments)) = validation::projected_statement_receiver_place(
                    program,
                    state,
                    statement_index,
                    statement,
                )
            {
                return Some(CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                });
            }
            canonical_place_from_symbol(statement.receiver_symbol)
        }
        // A call names its subject operand or has none. The retired implicit
        // receiver stood here as a `StateParameter::is_self` lookup that stage
        // 01 can no longer satisfy; the explicit operand below is the whole
        // answer, and a successor's data record is matched to its argument by
        // position in `instantiate_call_relative_places`.
        CallSite::Expression { call, .. } => call
            .receiver
            .is_valid()
            .then(|| canonical_place_from_expression(program, call.receiver))
            .flatten(),
        CallSite::TransitionNamed { .. } => {
            let argument = local_jump_data_argument(program, caller_machine_symbol, call_site)?;
            canonical_place_from_expression_in_state(
                program,
                caller_state_symbol,
                statement_index,
                argument,
            )
        }
    }
}

/// A local successor's explicit data operand, at its actual positional ordinal.
/// The declaration qualifier supplies no storage: retain the actual root and
/// projection, including when a different record is supplied to the successor.
pub(in crate::flow) fn local_jump_data_argument(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    call_site: &CallSite<'_>,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let CallSite::TransitionNamed { path, .. } = call_site else {
        return None;
    };
    if path.members.count() != 1 {
        return None;
    }
    let machine = program.machine_by_symbol(caller_machine_symbol)?;
    let target = find_state_in_machine(program, caller_machine_symbol, path.head_symbol)?;
    let parameters = program.state_parameters(target);
    let subject = program.data_binding_parameter(machine, parameters)?;
    let ordinal = typed_trees::signature::state_parameter_position(parameters, subject.symbol)?;
    call_site_argument_expressions(program, call_site)
        .get(ordinal)
        .copied()
}
