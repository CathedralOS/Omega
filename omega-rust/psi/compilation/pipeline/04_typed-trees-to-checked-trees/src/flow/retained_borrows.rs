//! Normal handback of typed storage held by an owned lifetime carrier.
//! A last use only bounds invalidation. Positive return evidence additionally
//! requires this exact invocation to receive the holder, return none of its
//! declared debt, and close the complete overlapping retained cohort.

use checked_trees::{BorrowCallFact, BorrowFacts, BorrowLoanFact, BorrowLoanLineage};
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

pub(super) struct RetainedBorrowHandback {
    pub place: CanonicalPlace,
    pub reference: TypeReferenceHandle,
}

pub(super) fn handbacks(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    argument_sources: Option<&super::call_phases::ArgumentSources<'_, '_>>,
) -> Vec<RetainedBorrowHandback> {
    let Some(recorded) = borrow
        .call_at(
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )
        .map(|handle| borrow.calls.get(handle))
        .filter(|recorded| {
            recorded.target_symbol == call.target_symbol
                && recorded.authored_expression == call.authored_expression
        })
    else {
        return Vec::new();
    };
    if validation::terminal_callable_target(program, machine, recorded.target_symbol).is_none()
        || validation::boundary_requirement_completion(program, recorded.target_symbol)
            != semantic_vocabulary::BoundaryCompletion::NormalReturn
    {
        return Vec::new();
    }
    let Some(site) = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        recorded.statement_index,
        recorded.call_ordinal,
    ) else {
        return Vec::new();
    };
    let Some(parameters) = site.parameters(program, machine.symbol) else {
        return Vec::new();
    };
    let arguments = validation::call_site_argument_expressions(program, &site);
    if std::env::var_os("OMEGA_DEBUG_HANDBACK").is_some() {
        eprintln!("HANDBACK {:?} {} {} {:?}", recorded.authored_expression, recorded.statement_index, recorded.call_ordinal, program.symbols.display_path(recorded.target_symbol, "::"));
        for arg in arguments { eprintln!("ARG {:?} {:?}", arg, validation::canonical_place_from_expression_in_state(program, state.symbol, recorded.statement_index, *arg)); }
        for (_, row) in borrow.states.iter().filter(|(_, row)| row.state_symbol == state.symbol) {
            for (h, loan) in borrow.state_loans(row) { eprintln!("LOAN {:?} owner {:?} path {:?} root {:?} {:?} form {:?} at {} last {} {:?}", h, loan.storage_owner, borrow.loan_owner_path(loan), loan.root, borrow.loan_segments(loan), loan.formation_expression, loan.statement_index, loan.last_use_statement_index, loan.lineage); }
        }
    }
    if parameters.len() != arguments.len() {
        return Vec::new();
    }
    let returned = match &site {
        validation::CallSite::Expression { call, .. } => {
            let Some(sources) =
                crate::borrow::call_reference_lifetime_sources(program, state.symbol, call)
            else {
                return Vec::new();
            };
            sources
                .into_iter()
                .map(|source| source.parameter_index)
                .collect::<Vec<_>>()
        }
        _ => {
            let Some(reference) =
                validation::call_target_return_type(program, recorded.target_symbol)
            else {
                return Vec::new();
            };
            if validation::reference_shell(program, reference).is_some()
                || validation::borrow_contracts::is_borrow_carrying_data(program, reference)
            {
                return Vec::new();
            }
            Vec::new()
        }
    };
    let mut holders = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if !argument_sources.is_some_and(|sources| sources.observation_survives(position, &[]))
            || returned.contains(&position)
            || validation::reference_shell(program, parameter.type_reference).is_some()
            || !validation::borrow_contracts::is_borrow_carrying_data(
                program,
                parameter.type_reference,
            )
        {
            continue;
        }
        if let Some(place) = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            recorded.statement_index,
            arguments[position],
        ) {
            holders.push(place);
        }
    }
    if holders.is_empty() {
        return Vec::new();
    }
    let Some((_, row)) = borrow
        .states
        .iter()
        .find(|(_, row)| row.state_symbol == state.symbol)
    else {
        return Vec::new();
    };
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let held_here = |loan: &BorrowLoanFact| {
        holders.iter().any(|holder| {
            holder.root == loan.storage_owner
                && crate::borrow::place_path_covers_owner(
                    program,
                    &holder.segments,
                    borrow.loan_owner_path(loan),
                )
        })
    };
    let before = |loan: &BorrowLoanFact| {
        order.precedes(loan.statement_index, recorded.statement_index)
            || (loan.statement_index == recorded.statement_index
                && loan.formation_expression.is_valid()
                && arguments.iter().any(|argument| {
                    validation::expression_owners::expression_contains(
                        program,
                        *argument,
                        loan.formation_expression,
                    )
                }))
    };
    let mut output = Vec::new();
    for (_, loan) in borrow.state_loans(row) {
        if loan.lineage != BorrowLoanLineage::LifetimeDependency
            || !loan.kind.is_exclusive()
            || loan.last_use_statement_index != recorded.statement_index
            || !before(loan)
            || !held_here(loan)
        {
            continue;
        }
        let place = CanonicalPlace {
            root: loan.root,
            segments: borrow.loan_segments(loan).to_vec(),
        };
        if !place.root.is_valid()
            || !place
                .segments
                .iter()
                .all(crate::flow::place_segment_has_static_identity)
        {
            continue;
        }
        let still_held = borrow.state_loans(row).any(|(_, other)| {
            if other.lineage != BorrowLoanLineage::LifetimeDependency
                || other.root != place.root
                || !crate::flow::canonical_place_segments_may_overlap(
                    program,
                    &place.segments,
                    borrow.loan_segments(other),
                )
            {
                return false;
            }
            // A returned/output generation is still a holder, including a
            // transient result that has no later source statement.
            let produced_here = recorded.authored_expression.is_valid()
                && other.formation_expression == recorded.authored_expression;
            produced_here
                || (before(other)
                    && (order.precedes(recorded.statement_index, other.last_use_statement_index)
                        || (other.last_use_statement_index == recorded.statement_index
                            && !held_here(other))))
        });
        if still_held {
            continue;
        }
        let Some(mut reference) = validation::canonical_place_type_reference(
            program,
            state.symbol,
            recorded.statement_index,
            &place,
        ) else {
            continue;
        };
        let referent = crate::checks::contracts::exclusive_referent_type(program, reference);
        if referent.is_valid() {
            reference = referent;
        }
        if !output
            .iter()
            .any(|existing: &RetainedBorrowHandback| existing.place == place)
        {
            output.push(RetainedBorrowHandback { place, reference });
        }
    }
    output
}
