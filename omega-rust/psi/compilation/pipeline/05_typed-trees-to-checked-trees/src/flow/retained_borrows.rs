//! Typed return duties follow completed lifetime inputs through owned carriers.
//! Lifetime arcs transport obligations, never descriptor geometry or permission.
//! A normal consuming call closes its exact actuals only after overlapping
//! retained holders have all ended. Old scalar observations stay invalidated.

use crate::borrow::call_reference_lifetime_sources;
use crate::borrow::place_path_covers_owner;
use crate::shared::collections::push_unique;
use checked_trees::{
    BorrowCallFact, BorrowFacts, BorrowLoanFact, BorrowLoanLineage, FlowControlFacts,
};
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

#[derive(Debug, Clone)]
pub(super) struct RetainedBorrowHandback {
    pub place: CanonicalPlace,
    pub reference: TypeReferenceHandle,
    /// Every possible origin must supply the declared duty. These are
    /// conjunctive proof suppliers, not part of the returned subject's identity.
    pub supplies: Vec<RetainedBorrowSupply>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RetainedBorrowSupply {
    pub operand: checked_trees::FlowOperandFact,
    pub constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
}

impl RetainedBorrowHandback {
    fn same_subject(&self, other: &Self) -> bool {
        self.place == other.place && self.reference == other.reference
    }

    fn require_supplies(&mut self, other: &Self) {
        for supply in &other.supplies {
            push_unique(&mut self.supplies, supply.clone());
        }
    }
}

mod qualifications;
pub(in crate::flow) use qualifications::declared_duties;

mod captured_inputs;
mod output_debts;
pub(in crate::flow) use captured_inputs::{RetainedSources, captured_sources, completed_calls};
pub(in crate::flow) use output_debts::sources as output_debt_sources;

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
            call_reference_lifetime_sources(program, state.symbol, call)?
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
        result.append(sources.retained_sources(borrow, call, control, position)?);
    }
    Some(result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handbacks(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    contexts: &checked_trees::FlowContextFacts,
    lifetimes: &checked_trees::FlowBorrowLifetimeFacts,
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
                && place_path_covers_owner(program, &holder.segments, borrow.loan_owner_path(loan))
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
    let Some(calls) = control.calls.span(sources.call_scope().calls) else {
        return Vec::new();
    };
    for performed in calls.iter().filter(|performed| {
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
        let Some(captured) = super::call_phases::ArgumentSources::new(
            program,
            borrow,
            machine.symbol,
            state.symbol,
            recorded,
            Some(&[]),
            control,
            sources.call_scope().calls,
            performed.operands,
            performed.invocation_storage,
        ) else {
            continue;
        };
        if let Some(ended) = ending_inputs(
            program, borrow, control, machine, state, recorded, &captured,
        ) {
            previous.push((recorded, ended.holders));
        }
    }
    // Consuming a record ends that generation, not every saved reference
    // derived from it. Start with the descriptors captured at this call's
    // entry, and keep each still-live holder's complete original obligations.
    // A replacement at the same name cannot revive its discarded generation.
    let held = (|| -> Result<Vec<_>, &'static str> {
        let entry = call
            .input_reference_loans
            .and_then(|span| borrow.loan_suppliers.span(span))
            .ok_or("the consuming call has no complete entry obligation roster")?;
        borrow
            .loans
            .span(call.completed_reference_loans)
            .ok_or("the consuming call has no complete produced obligation roster")?;
        let produced = call.completed_reference_loans.handles().collect::<Vec<_>>();
        let mut roots = entry.to_vec();
        for root in &produced {
            push_unique(&mut roots, *root);
        }
        let mut held = Vec::new();
        for root in roots {
            if !borrow.state_owns_loan(row, root) {
                return Err("the consuming call names a foreign or unavailable obligation holder");
            }
            let holder = borrow.loans.get(root);
            let produced_here = produced.contains(&root);
            let ended_before = previous.iter().any(|(performed, holders)| {
                formed_before(holder, performed) && covers(holders, holder)
            });
            let live = produced_here
                || (formed_before(holder, call)
                    && !ended_before
                    && (order.precedes(call.statement_index, holder.last_use_statement_index)
                        || (holder.last_use_statement_index == call.statement_index
                            && !covers(&ending.holders, holder))));
            if live {
                let obligations = crate::borrow::retained_loan_closure(
                    borrow,
                    row,
                    root,
                    &crate::borrow::RetainedLoanSourceScope {
                        program,
                        contexts,
                        lifetimes,
                        control,
                        calls: sources.call_scope(),
                    },
                )?;
                held.push((root, obligations));
            }
        }
        Ok(held)
    })();
    let held = match held {
        Ok(held) => held,
        Err(reason) => {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE retained-handback-holders-unavailable state={:?} statement={} call={} reason={reason}",
                    state.symbol, call.statement_index, call.call_ordinal
                );
            }
            return Vec::new();
        }
    };
    ending
        .borrows
        .into_iter()
        .filter(|returned| {
            returned.place.root.is_valid()
                && returned.place.segments.iter().all(super::place_segment_has_static_identity)
                && !held.iter().any(|(holder, obligations)| {
                    obligations.iter().any(|handle| {
                        let other = borrow.loans.get(*handle);
                        let blocked = other.lineage == BorrowLoanLineage::LifetimeDependency
                            && other.root == returned.place.root
                            && super::canonical_place_segments_may_overlap(
                                program, &returned.place.segments, borrow.loan_segments(other),
                            );
                        if blocked && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                            let owner = borrow.loans.get(*holder);
                            eprintln!("CHECKSITE retained-handback-still-held state={:?} statement={} call={} subject={:?} holder={holder:?} holder_owner={:?} holder_last_use={} loan={handle:?} owner={:?} owner_path={:?} ending_holders={:?}",
                                state.symbol, call.statement_index, call.call_ordinal, returned.place,
                                owner.storage_owner, owner.last_use_statement_index, other.storage_owner,
                                borrow.loan_owner_path(other), ending.holders);
                        }
                        blocked
                    })
                })
        })
        .collect()
}
