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

    // Checked recording is deliberately idempotent: each run rebuilds this
    // proof ledger from the unchanged resource/control facts.
    facts
        .borrow
        .compatibility_certificates
        .reset_retain_capacity();

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
        );
    }

    facts
        .borrow
        .compatibility_certificates
        .insert_many(compatibility_certificates);
    diagnostics.extend(validate_checked_borrow_compatibility_certificates(
        program, facts,
    ));

    if diagnostics.is_empty() {
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
    facts
        .borrow
        .compatibility_certificates
        .iter()
        .filter_map(|(_, certificate)| {
            replay_checked_borrow_compatibility_certificate(program, facts, certificate).err()
        })
        .collect()
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
    let replayed = overlap::captured_place_compatibility(
        program,
        &certificate.forming_place,
        forming_access,
        &certificate.active_place,
        active_access,
    );
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
