pub(crate) mod calls;
mod details;
mod elision;
mod escape;
mod overlap;
mod persistent;
mod resources;
mod statements;

use super::ranges::incoming_guards::IncomingGuardIndex;
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use self::elision::check_view_return_elision;
use self::escape::check_view_return_escape;
use self::persistent::check_persistent_borrow_assignments;
use self::statements::check_statement_borrows;

pub(crate) fn check_flow_call_borrows(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming_guards: &IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut retained_diagnostics =
        validate_checked_borrow_compatibility_certificates(program, facts, incoming_guards);
    retained_diagnostics.extend(validate_checked_borrow_mutation_certificates(
        program,
        facts,
        incoming_guards,
    ));
    retained_diagnostics.extend(calls::validate_compatibility(
        program,
        facts,
        incoming_guards,
        &mut diagnostics,
    ));
    if retained_diagnostics.is_empty() {
        resources::replay_checked_direct_borrow_resources(program, facts, mutation_summaries)?;
    } else {
        // The retained resource rows are an independent evidence channel that
        // certificate rejection must not silence: a tampered loan or resource
        // still has to surface through its own substrate replay. Run it on a
        // scratch copy so the transactional rebuild cannot publish rows into
        // facts this pass is already rejecting.
        let mut scratch = facts.clone();
        if let Err(mut resource_diagnostics) = resources::replay_checked_direct_borrow_resources(
            program,
            &mut scratch,
            mutation_summaries,
        ) {
            retained_diagnostics.append(&mut resource_diagnostics);
        }
        retained_diagnostics.append(&mut diagnostics);
        return Err(retained_diagnostics);
    }
    let mut compatibility_certificates = Vec::new();
    let retained_compatibility_certificates = facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    let mut retained_compatibility_certificates_consumed =
        vec![false; retained_compatibility_certificates.len()];
    let mut mutation_certificates = Vec::new();
    let retained_mutation_certificates = facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    let mut retained_mutation_certificates_consumed =
        vec![false; retained_mutation_certificates.len()];

    check_view_return_elision(program, &mut diagnostics);
    check_view_return_escape(program, facts, &mut diagnostics);
    check_persistent_borrow_assignments(program, call_frames, &mut diagnostics);

    for (_, state_flow) in facts.flow.control.states.iter() {
        if matching_borrow_state(facts, state_flow).is_none() {
            continue;
        }

        // Ordering premises come from this state's preconditions and incoming
        // guards with independently reconstructed immutable transport.
        // Premise subjects are immutable bound values, so the set
        // is stable for every judgment inside the state -- loan formations,
        // mutations, call accesses, and receivers alike.
        let stated_premises = crate::semantic_calls::find_state_in_machine(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        )
        .and_then(|state| {
            crate::lookup::machine_by_symbol(program, state_flow.machine_symbol)
                .map(|machine| (machine, state))
        })
        .map(|(machine, state)| {
            overlap::stated_ordering_premises(program, facts, machine, state, incoming_guards)
        })
        .unwrap_or_default();

        check_statement_borrows(
            program,
            facts,
            state_flow,
            &stated_premises,
            &mut diagnostics,
            &mut compatibility_certificates,
            &retained_compatibility_certificates,
            &mut retained_compatibility_certificates_consumed,
            &mut mutation_certificates,
            &retained_mutation_certificates,
            &mut retained_mutation_certificates_consumed,
            mutation_summaries,
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
        if let Err(diagnostic) = replay_checked_borrow_compatibility_certificate(
            program,
            facts,
            certificate,
            incoming_guards,
        ) {
            diagnostics.push(diagnostic);
        }
    }

    for (certificate, consumed) in retained_mutation_certificates
        .iter()
        .zip(&retained_mutation_certificates_consumed)
    {
        if !consumed {
            diagnostics.push(Diagnostic::error(format!(
                "checked borrow mutation certificate at statement {} was not consumed by its exact formation mutation pair",
                certificate.formation.statement_index,
            )));
        }
    }
    for (index, certificate) in mutation_certificates.iter().enumerate() {
        if mutation_certificates[..index]
            .iter()
            .any(|prior| mutation_certificate_key_matches(prior, certificate))
        {
            diagnostics.push(duplicate_mutation_certificate_diagnostic(certificate));
            continue;
        }
        if let Err(diagnostic) =
            replay_checked_borrow_mutation_certificate(program, facts, certificate, incoming_guards)
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
        facts.borrow.mutation_certificates.reset_retain_capacity();
        facts
            .borrow
            .mutation_certificates
            .insert_many(mutation_certificates);
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn initialize_checked_direct_borrow_resources(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    resources::initialize_checked_direct_borrow_resources(program, facts, mutation_summaries)
}

pub(super) fn initialize_checked_borrow_call_certificates(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
) {
    calls::initialize_compatibility(program, facts)
}

fn validate_checked_borrow_compatibility_certificates(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    incoming_guards: &IncomingGuardIndex,
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
        if let Err(diagnostic) = replay_checked_borrow_compatibility_certificate(
            program,
            facts,
            certificate,
            incoming_guards,
        ) {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn duplicate_compatibility_certificate_diagnostic(
    certificate: &checked_trees::CheckedBorrowCompatibilityCertificate,
) -> Diagnostic {
    Diagnostic::error(format!(
        "checked borrow compatibility certificate duplicates the formation loan-pair key at statement {}",
        certificate.formation.statement_index,
    ))
}

fn compatibility_certificate_key_matches(
    left: &checked_trees::CheckedBorrowCompatibilityCertificate,
    right: &checked_trees::CheckedBorrowCompatibilityCertificate,
) -> bool {
    left.formation == right.formation
        && left.forming_loan == right.forming_loan
        && left.active_loan == right.active_loan
}

fn replay_checked_borrow_compatibility_certificate(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    certificate: &checked_trees::CheckedBorrowCompatibilityCertificate,
    incoming_guards: &IncomingGuardIndex,
) -> Result<(), Diagnostic> {
    if !facts
        .borrow
        .compatibility_certificate_matches_resources(certificate)
    {
        return Err(Diagnostic::error(
            "checked borrow compatibility certificate does not rejoin its exact state-owned loans",
        ));
    }
    // The derivation class must agree with the recorded premise ledger: a
    // premised conclusion must name at least one exact establishment token, and a
    // structural conclusion must have consulted none.
    match certificate.derivation {
        checked_trees::BorrowCompatibilityDerivation::Structural
            if certificate.premises.is_empty() => {}
        checked_trees::BorrowCompatibilityDerivation::Premised
            if !certificate.premises.is_empty() => {}
        _ => {
            return Err(Diagnostic::error(
                "checked borrow compatibility certificate derivation drifted from its recorded premise ledger",
            ));
        }
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
    // The premise set is re-derived from the formation scope's entry
    // establishment points, not trusted from the certificate. An unresolvable formation
    // scope offers no premises, so a recorded premised token cannot replay.
    let stated_premises = crate::semantic_calls::find_state_in_machine(
        program,
        certificate.formation.machine_symbol,
        certificate.formation.state_symbol,
    )
    .and_then(|state| {
        crate::lookup::machine_by_symbol(program, certificate.formation.machine_symbol)
            .map(|machine| (machine, state))
    })
    .map(|(machine, state)| {
        overlap::stated_ordering_premises(program, facts, machine, state, incoming_guards)
    })
    .unwrap_or_default();
    let replayed = match overlap::borrow_loan_compatibility_from_selector_snapshot(
        program,
        facts,
        forming_loan,
        forming_access,
        active_loan,
        active_access,
        &certificate.selector_snapshot,
        &stated_premises,
        &certificate.premises,
    ) {
        Ok(replayed) => replayed,
        Err(overlap::CompatibilityReplayDrift::Premise) => {
            return Err(Diagnostic::error(
                "checked borrow compatibility certificate premise tokens drifted from their established scope evidence",
            ));
        }
        Err(overlap::CompatibilityReplayDrift::SelectorSnapshot) => {
            return Err(Diagnostic::error(
                "checked borrow compatibility certificate selector snapshot drifted from its captured-place shape",
            ));
        }
    };
    let replayed_conclusion = checked_trees::BorrowCompatibilityConclusion {
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
    // A pair the spatial verdict could not discharge was admitted only through
    // recorded provenance. Replay that edge from the loan rows rather than
    // trusting the ledger: without it the certificate has no replayable
    // admission basis.
    if !certificate.conclusion.non_interfering
        && !statements::carried_authority(
            forming_loan,
            certificate.active_loan,
            active_loan,
            active_access,
            certificate.conclusion.containment,
        )
    {
        return Err(Diagnostic::error(
            "checked borrow compatibility certificate records an interfering loan pair without carried authority",
        ));
    }
    Ok(())
}

fn validate_checked_borrow_mutation_certificates(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    incoming_guards: &IncomingGuardIndex,
) -> Vec<Diagnostic> {
    let certificates = facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    for (index, certificate) in certificates.iter().enumerate() {
        if certificates[..index]
            .iter()
            .any(|prior| mutation_certificate_key_matches(prior, certificate))
        {
            diagnostics.push(duplicate_mutation_certificate_diagnostic(certificate));
            continue;
        }
        if let Err(diagnostic) =
            replay_checked_borrow_mutation_certificate(program, facts, certificate, incoming_guards)
        {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn duplicate_mutation_certificate_diagnostic(
    certificate: &checked_trees::CheckedBorrowMutationCertificate,
) -> Diagnostic {
    Diagnostic::error(format!(
        "checked borrow mutation certificate duplicates the formation mutation-loan key at statement {}",
        certificate.formation.statement_index,
    ))
}

fn mutation_certificate_key_matches(
    left: &checked_trees::CheckedBorrowMutationCertificate,
    right: &checked_trees::CheckedBorrowMutationCertificate,
) -> bool {
    left.formation == right.formation && left.active_loan == right.active_loan
}

fn replay_checked_borrow_mutation_certificate(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    certificate: &checked_trees::CheckedBorrowMutationCertificate,
    incoming_guards: &IncomingGuardIndex,
) -> Result<(), Diagnostic> {
    if !facts
        .borrow
        .mutation_certificate_matches_resources(certificate)
    {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate does not rejoin its exact state-owned loan",
        ));
    }
    // The derivation class must agree with the recorded premise ledger: a
    // premised conclusion must name at least one exact establishment token, and a
    // structural conclusion must have consulted none.
    match certificate.derivation {
        checked_trees::BorrowCompatibilityDerivation::Structural
            if certificate.premises.is_empty() => {}
        checked_trees::BorrowCompatibilityDerivation::Premised
            if !certificate.premises.is_empty() => {}
        _ => {
            return Err(Diagnostic::error(
                "checked borrow mutation certificate derivation drifted from its recorded premise ledger",
            ));
        }
    }

    let Some(active_access) = facts
        .borrow
        .mutation_certificate_resource_access(certificate)
    else {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate does not rejoin its exact state-owned loan",
        ));
    };
    let active_loan = facts.borrow.loans.get(certificate.active_loan);

    // The mutated place is re-derived from the typed formation statement, not
    // trusted from the row: the certificate only stands when the statement's
    // write target re-roots to the exact place the row judges.
    let Some(state) = crate::semantic_calls::find_state_in_machine(
        program,
        certificate.formation.machine_symbol,
        certificate.formation.state_symbol,
    ) else {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate does not rejoin its exact formation state",
        ));
    };
    let Some(statement_node) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(certificate.formation.statement_index)
    else {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate does not rejoin its exact formation statement",
        ));
    };
    let Some(mutated_place) = crate::flow::statement_mutated_place(
        program,
        certificate.formation.machine_symbol,
        certificate.formation.state_symbol,
        certificate.formation.statement_index,
        statement_node,
    ) else {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate formation statement has no mutated place",
        ));
    };
    if overlap::canonical_place_for_loan(&mutated_place, active_loan)
        != Some(certificate.mutated_place.clone())
    {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate mutated place drifted from the statement's write target",
        ));
    }

    // The pair relation is replayed too: the named active loan must have been
    // live at the formation statement's own entry constraints.
    let loan_live_at_formation = facts
        .flow
        .control
        .states
        .iter()
        .find(|(_, state)| {
            state.machine_symbol == certificate.formation.machine_symbol
                && state.state_symbol == certificate.formation.state_symbol
        })
        .and_then(|(_, state)| {
            facts
                .flow
                .control
                .statements
                .span_or_empty(state.statements)
                .iter()
                .find(|statement| {
                    statement.statement_index == certificate.formation.statement_index
                })
        })
        .is_some_and(|statement| {
            facts
                .flow
                .borrow_loan_constraints(statement.entry_constraints)
                .any(|loan| loan == certificate.active_loan)
        });
    if !loan_live_at_formation {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate active loan was not live at its formation statement",
        ));
    }

    // The premise set is re-derived from the formation scope's entry
    // establishment points, not trusted from the certificate. An unresolvable formation
    // scope offers no premises, so a recorded premised token cannot replay.
    let stated_premises =
        crate::lookup::machine_by_symbol(program, certificate.formation.machine_symbol)
            .map(|machine| {
                overlap::stated_ordering_premises(program, facts, machine, state, incoming_guards)
            })
            .unwrap_or_default();
    let replayed = match overlap::captured_place_loan_compatibility_from_selector_snapshot(
        program,
        &certificate.mutated_place,
        &checked_trees::BorrowAccessKind::Mutable,
        active_loan,
        active_access,
        &facts.borrow,
        &certificate.selector_snapshot,
        &stated_premises,
        &certificate.premises,
    ) {
        Ok(replayed) => replayed,
        Err(overlap::CompatibilityReplayDrift::Premise) => {
            return Err(Diagnostic::error(
                "checked borrow mutation certificate premise tokens drifted from their established scope evidence",
            ));
        }
        Err(overlap::CompatibilityReplayDrift::SelectorSnapshot) => {
            return Err(Diagnostic::error(
                "checked borrow mutation certificate selector snapshot drifted from its captured-place shape",
            ));
        }
    };
    let replayed_conclusion = checked_trees::BorrowCompatibilityConclusion {
        disjoint: replayed.disjoint,
        containment: replayed.containment,
        non_interfering: replayed.non_interfering,
    };
    if replayed.left != certificate.mutated_place
        || replayed.right != certificate.active_place
        || replayed_conclusion != certificate.conclusion
    {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate conclusion drifted from independent structural replay",
        ));
    }
    // A mutation carries no provenance edge: the only verdict this row may
    // record is replayed non-interference.
    if !certificate.conclusion.non_interfering {
        return Err(Diagnostic::error(
            "checked borrow mutation certificate records an interfering mutation pair with no admission basis",
        ));
    }
    Ok(())
}

fn matching_borrow_state<'a>(
    facts: &'a CheckFacts,
    state_flow: &FlowStateFact,
) -> Option<&'a checked_trees::StateBorrowFact> {
    facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    })
}

#[cfg(test)]
mod tests;
