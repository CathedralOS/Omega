mod calls;
mod details;
mod elision;
mod escape;
mod overlap;
mod persistent;
mod resources;
mod statements;

use psi_checked_trees::{CheckFacts, FlowStateFact};
use psi_diagnostics::Diagnostic;

use self::calls::check_call_borrows;
use self::elision::check_view_return_elision;
use self::escape::check_view_return_escape;
use self::persistent::check_persistent_borrow_assignments;
use self::statements::check_statement_borrows;

pub(crate) fn check_flow_call_borrows(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let retained_diagnostics = validate_checked_borrow_compatibility_certificates(program, facts);
    if !retained_diagnostics.is_empty() {
        return Err(retained_diagnostics);
    }
    resources::replay_checked_direct_borrow_resources(program, facts)?;
    let mut diagnostics = Vec::new();
    let mut compatibility_certificates = Vec::new();
    let retained_compatibility_certificates = facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    let mut retained_compatibility_certificates_consumed =
        vec![false; retained_compatibility_certificates.len()];
    let mut state_mutation_summaries = crate::flow::StateMutationSummaryCache::default();

    check_view_return_elision(program, &mut diagnostics);
    check_view_return_escape(program, facts, &mut diagnostics);
    check_persistent_borrow_assignments(program, &mut diagnostics);

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(borrow_state) = matching_borrow_state(facts, state_flow) else {
            continue;
        };

        for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
            check_call_borrows(program, facts, state_flow, borrow_call, &mut diagnostics);
        }

        check_statement_borrows(
            program,
            facts,
            state_flow,
            &mut diagnostics,
            &mut compatibility_certificates,
            &retained_compatibility_certificates,
            &mut retained_compatibility_certificates_consumed,
            &mut state_mutation_summaries,
        );
    }

    for (certificate, consumed) in retained_compatibility_certificates
        .iter()
        .zip(&retained_compatibility_certificates_consumed)
    {
        if !consumed {
            diagnostics.push(Diagnostic::error(format!(
                "checked borrow compatibility certificate at statement {} was not consumed by its exact formation loan pair",
                certificate.formation.statement_index,
            )));
        }
    }
    for (index, certificate) in compatibility_certificates.iter().enumerate() {
        if compatibility_certificates[..index]
            .iter()
            .any(|prior| compatibility_certificate_key_matches(prior, certificate))
        {
            diagnostics.push(duplicate_compatibility_certificate_diagnostic(certificate));
            continue;
        }
        if let Err(diagnostic) =
            replay_checked_borrow_compatibility_certificate(program, facts, certificate)
        {
            diagnostics.push(diagnostic);
        }
    }

    if diagnostics.is_empty() {
        // Settlement is transactional: publish the rebuilt proof ledger only
        // after every retained formation was consumed exactly once and every
        // new row independently replayed.
        facts
            .borrow
            .compatibility_certificates
            .reset_retain_capacity();
        facts
            .borrow
            .compatibility_certificates
            .insert_many(compatibility_certificates);
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn initialize_checked_direct_borrow_resources(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    resources::initialize_checked_direct_borrow_resources(program, facts)
}

fn validate_checked_borrow_compatibility_certificates(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Vec<Diagnostic> {
    let certificates = facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    for (index, certificate) in certificates.iter().enumerate() {
        if certificates[..index]
            .iter()
            .any(|prior| compatibility_certificate_key_matches(prior, certificate))
        {
            diagnostics.push(duplicate_compatibility_certificate_diagnostic(certificate));
            continue;
        }
        if let Err(diagnostic) =
            replay_checked_borrow_compatibility_certificate(program, facts, certificate)
        {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn duplicate_compatibility_certificate_diagnostic(
    certificate: &psi_checked_trees::CheckedBorrowCompatibilityCertificate,
) -> Diagnostic {
    Diagnostic::error(format!(
        "checked borrow compatibility certificate duplicates the formation loan-pair key at statement {}",
        certificate.formation.statement_index,
    ))
}

fn compatibility_certificate_key_matches(
    left: &psi_checked_trees::CheckedBorrowCompatibilityCertificate,
    right: &psi_checked_trees::CheckedBorrowCompatibilityCertificate,
) -> bool {
    left.formation == right.formation
        && left.forming_loan == right.forming_loan
        && left.active_loan == right.active_loan
}

fn replay_checked_borrow_compatibility_certificate(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    certificate: &psi_checked_trees::CheckedBorrowCompatibilityCertificate,
) -> Result<(), Diagnostic> {
    if !facts
        .borrow
        .compatibility_certificate_matches_resources(certificate)
    {
        return Err(Diagnostic::error(
            "checked borrow compatibility certificate does not rejoin its exact state-owned loans",
        ));
    }
    if certificate.derivation != psi_checked_trees::BorrowCompatibilityDerivation::Structural {
        return Err(Diagnostic::error(
            "checked borrow compatibility certificate has a non-structural derivation",
        ));
    }

    let Some((forming_access, active_access)) = facts
        .borrow
        .compatibility_certificate_resource_accesses(certificate)
    else {
        return Err(Diagnostic::error(
            "checked borrow compatibility certificate does not rejoin its exact state-owned loans",
        ));
    };
    let forming_loan = facts.borrow.loans.get(certificate.forming_loan);
    let active_loan = facts.borrow.loans.get(certificate.active_loan);
    let Some(replayed) = overlap::borrow_loan_compatibility_from_selector_snapshot(
        program,
        facts,
        forming_loan,
        forming_access,
        active_loan,
        active_access,
        &certificate.selector_snapshot,
    ) else {
        return Err(Diagnostic::error(
            "checked borrow compatibility certificate selector snapshot drifted from its captured-place shape",
        ));
    };
    let replayed_conclusion = psi_checked_trees::BorrowCompatibilityConclusion {
        disjoint: replayed.disjoint,
        containment: replayed.containment,
        non_interfering: replayed.non_interfering,
    };
    if replayed.left != certificate.forming_place
        || replayed.right != certificate.active_place
        || replayed_conclusion != certificate.conclusion
    {
        return Err(Diagnostic::error(
            "checked borrow compatibility certificate conclusion drifted from independent structural replay",
        ));
    }
    Ok(())
}

fn matching_borrow_state<'a>(
    facts: &'a CheckFacts,
    state_flow: &FlowStateFact,
) -> Option<&'a psi_checked_trees::StateBorrowFact> {
    facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    })
}
