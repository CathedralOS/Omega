use psi_checked_trees::{
    BorrowFacts, CheckFacts, CheckedDirectBorrowLoanResource, CheckedDirectBorrowParentLifetime,
    CheckedDirectBorrowRestorationObligation, FlowFacts, FlowInvalidationSource,
};
use psi_diagnostics::Diagnostic;

/// Populate the checked-only direct-root resource closure before ordinary
/// checked-fact replay. This carrier is deliberately absent for reborrows and
/// borrow-carrying transfers, whose exact parent/source occurrence is not yet
/// retained by `BorrowLoanFact`.
pub(super) fn initialize_checked_direct_borrow_resources(
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let resources = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    facts.borrow.direct_loan_resources.reset_retain_capacity();
    facts.borrow.direct_loan_resources.insert_many(resources);
    Ok(())
}

/// Independently replay every retained direct-root resource from the
/// authoritative loan and flow-lifetime ledgers, then rebuild it
/// deterministically. The row itself never participates in borrow admission.
pub(super) fn replay_checked_direct_borrow_resources(
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let expected = reconstruct_direct_borrow_resources(&facts.borrow, &facts.flow)?;
    let retained = facts
        .borrow
        .direct_loan_resources
        .iter()
        .map(|(_, resource)| resource.clone())
        .collect::<Vec<_>>();
    if retained != expected {
        return Err(vec![Diagnostic::error(
            "checked direct-root borrow resource closure drifted from independent replay",
        )]);
    }

    facts.borrow.direct_loan_resources.reset_retain_capacity();
    facts.borrow.direct_loan_resources.insert_many(expected);
    Ok(())
}

fn reconstruct_direct_borrow_resources(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedDirectBorrowLoanResource>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-root borrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            // A valid symbol here means this loan was rebased or transferred
            // through another owner. The existing row does not retain an exact
            // source occurrence handle, so this first carrier must fail closed
            // by omitting it.
            if loan.source_owner_symbol.is_valid() {
                continue;
            }

            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .filter(|activation| activation.loan == loan_handle)
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .filter(|weakening| weakening.loan == loan_handle)
                .collect::<Vec<_>>();
            let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice())
            else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow resource requires exactly one activation and one weakening",
                ));
                continue;
            };
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow activation drifted from loan formation",
                ));
                continue;
            }

            let parent_lifetime = CheckedDirectBorrowParentLifetime {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                root_symbol: loan.root_symbol,
            };
            let restoration = CheckedDirectBorrowRestorationObligation {
                parent: parent_lifetime.clone(),
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
            };
            resources.push(CheckedDirectBorrowLoanResource {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: psi_checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
                    segments: borrow.loan_segments(loan).to_vec(),
                },
                access: loan.kind.clone(),
                activation_source: activation.source,
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
                parent_lifetime,
                restoration,
            });
        }
    }

    if diagnostics.is_empty() {
        Ok(resources)
    } else {
        Err(diagnostics)
    }
}
