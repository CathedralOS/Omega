//! Typed return duties follow completed lifetime inputs through owned carriers.
//! Lifetime arcs transport obligations, never descriptor geometry or permission.
//! A normal consuming call closes its exact actuals only after overlapping
//! retained holders have all ended. Old scalar observations stay invalidated.

use checked_trees::{
    BorrowCallFact, BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowControlFacts,
};
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RetainedBorrowHandback {
    pub place: CanonicalPlace,
    pub reference: TypeReferenceHandle,
}

mod captured_inputs;
pub(in crate::flow) use captured_inputs::{RetainedSources, captured_sources, completed_calls};

fn ending_inputs(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    control: &FlowControlFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    sources: &super::call_phases::ArgumentSources<'_, '_>,
) -> Option<RetainedSources> {
    let recorded = borrow.call_at(
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    if borrow.calls.get(recorded) != call
        || validation::terminal_callable_target(program, machine, call.target_symbol).is_none()
        || validation::boundary_requirement_completion(program, call.target_symbol)
            != semantic_vocabulary::BoundaryCompletion::NormalReturn
    {
        return None;
    }
    let site = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameters = site.parameters(program, machine.symbol)?;
    let returned = match &site {
        validation::CallSite::Expression { call, .. } => {
            crate::borrow::call_reference_lifetime_sources(program, state.symbol, call)?
                .into_iter()
                .map(|source| source.parameter_index)
                .collect::<Vec<_>>()
        }
        _ => {
            let reference = validation::call_target_return_type(program, call.target_symbol)?;
            if validation::reference_shell(program, reference).is_some()
                || validation::borrow_contracts::is_borrow_carrying_data(program, reference)
            {
                return None;
            }
            Vec::new()
        }
    };
    let mut result = RetainedSources::default();
    for (position, parameter) in parameters.iter().enumerate() {
        if returned.contains(&position)
            || program.type_multiplicity(parameter.type_reference)
                == language_semantics::Multiplicity::Unrestricted
            || validation::reference_shell(program, parameter.type_reference).is_some()
            || !validation::borrow_contracts::is_borrow_carrying_data(
                program,
                parameter.type_reference,
            )
        {
            continue;
        }
        result.append(sources.retained_sources(control, position)?);
    }
    Some(result)
}

pub(super) fn handbacks(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    control: &FlowControlFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    argument_sources: Option<&super::call_phases::ArgumentSources<'_, '_>>,
) -> Vec<RetainedBorrowHandback> {
    let Some(sources) = argument_sources else {
        return Vec::new();
    };
    let Some(ending) = ending_inputs(program, borrow, control, machine, state, call, sources)
    else {
        return Vec::new();
    };

    if ending.borrows.is_empty() {
        return Vec::new();
    }
    let Some((_, row)) = borrow.first_state_borrow(machine.symbol, state.symbol) else {
        return Vec::new();
    };
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let covers = |holders: &[CanonicalPlace], loan: &BorrowLoanFact| {
        holders.iter().any(|holder| {
            holder.root == loan.storage_owner
                && crate::borrow::place_path_covers_owner(
                    program,
                    &holder.segments,
                    borrow.loan_owner_path(loan),
                )
        })
    };
    let formed_before = |loan: &BorrowLoanFact, invocation: &BorrowCallFact| {
        order.precedes(loan.statement_index, invocation.statement_index)
            || (loan.statement_index == invocation.statement_index
                && loan.formation_expression.is_valid()
                && validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    invocation.statement_index,
                    invocation.call_ordinal,
                )
                .is_some_and(|site| {
                    validation::call_site_argument_expressions(program, &site)
                        .iter()
                        .any(|argument| {
                            validation::expression_owners::expression_contains(
                                program,
                                *argument,
                                loan.formation_expression,
                            )
                        })
                }))
    };
    let completed = sources.completed_calls(control);
    let mut previous = Vec::new();
    for (_, performed) in control.calls.iter().filter(|(_, performed)| {
        performed.statement_index == call.statement_index
            && completed.contains(&performed.authored_expression)
    }) {
        let Some(recorded) = borrow
            .call_at(
                machine.symbol,
                state.symbol,
                performed.statement_index,
                performed.call_ordinal,
            )
            .map(|handle| borrow.calls.get(handle))
            .filter(|recorded| {
                recorded.authored_expression == performed.authored_expression
                    && recorded.target_symbol == performed.target_symbol
            })
        else {
            continue;
        };
        let Some(inputs) = control.operands.span(performed.operands) else {
            continue;
        };
        let Some(captured) = super::call_phases::ArgumentSources::new(
            program,
            machine.symbol,
            state.symbol,
            recorded,
            Some(&[]),
            control,
            inputs,
        ) else {
            continue;
        };
        if let Some(ended) = ending_inputs(
            program, borrow, control, machine, state, recorded, &captured,
        ) {
            previous.push((recorded, ended.holders));
        }
    }
    ending
        .borrows
        .into_iter()
        .filter(|returned| {
            returned.place.root.is_valid()
                && returned
                    .place
                    .segments
                    .iter()
                    .all(super::place_segment_has_static_identity)
                && !borrow.state_loans(row).any(|(_, other)| {
                    if other.lineage != BorrowLoanLineage::LifetimeDependency
                        || other.root != returned.place.root
                        || !super::canonical_place_segments_may_overlap(
                            program,
                            &returned.place.segments,
                            borrow.loan_segments(other),
                        )
                    {
                        return false;
                    }
                    let produced_here = call.authored_expression.is_valid()
                        && other.formation_expression == call.authored_expression;
                    let ended_before = previous.iter().any(|(performed, holders)| {
                        formed_before(other, performed) && covers(holders, other)
                    });
                    produced_here
                        || (formed_before(other, call)
                            && !ended_before
                            && (order
                                .precedes(call.statement_index, other.last_use_statement_index)
                                || (other.last_use_statement_index == call.statement_index
                                    && !covers(&ending.holders, other))))
                })
        })
        .collect()
}
