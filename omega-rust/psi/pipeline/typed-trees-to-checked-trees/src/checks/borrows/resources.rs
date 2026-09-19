//! Checked direct-borrow resources: the closures over direct roots and
//! direct reborrows that the borrow checks replay and retain.
//!
//! This file owns the two entry points. `reborrow_drafts.rs` carries the
//! draft rows, `lifecycle.rs` the lifecycle phases and boundaries,
//! `certificate_planning.rs` plans containment and restored-call-use
//! certificates, `disposition_events.rs` plans and resolves disposition
//! events, `retained_validation.rs` validates retained rows against replay,
//! `resource_reconstruction.rs` rebuilds the resource arenas and
//! `lineage_replay.rs` replays reborrow lineages.

mod certificate_planning;
mod disposition_events;
mod lifecycle;
mod lineage_replay;
mod reborrow_drafts;
mod resource_reconstruction;
mod retained_validation;

use crate::checks::borrows::resources::certificate_planning::{
    plan_reborrow_containment_certificates, plan_reborrow_restored_call_uses,
    plan_resource_installation,
};
use crate::checks::borrows::resources::disposition_events::plan_reborrow_disposition_events;
use crate::checks::borrows::resources::lineage_replay::replay_checked_direct_reborrow_lineage;
use crate::checks::borrows::resources::reborrow_drafts::validate_retained_reborrow_resources;
use crate::checks::borrows::resources::resource_reconstruction::{
    install_borrow_resources, reconstruct_direct_borrow_resources,
    reconstruct_reborrow_resource_drafts,
};
use crate::checks::borrows::resources::retained_validation::{
    validate_retained_containment_certificates, validate_retained_disposition_events,
    validate_retained_restored_call_uses,
};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

// The transient-call-argument check in `calls::writability` shares the
// lattice's directed attenuation diagnostic for reference-binding reborrows.
pub(super) use retained_validation::invalid_reborrow_attenuation_diagnostic;

/// Populate the checked-only direct-root and direct-reborrow resource closures
/// before ordinary checked-fact replay.
pub(super) fn initialize_checked_direct_borrow_resources(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let direct = reconstruct_direct_borrow_resources(program, &facts.borrow, &facts.flow)?;
    let reborrows = reconstruct_reborrow_resource_drafts(&facts.borrow, &facts.flow)?;
    let installation = plan_resource_installation(&direct, &reborrows)?;
    let dispositions =
        plan_reborrow_disposition_events(&facts.flow, &direct, &reborrows, &installation)?;
    let containments = plan_reborrow_containment_certificates(&direct, &reborrows, &installation)?;
    let restored_uses = plan_reborrow_restored_call_uses(
        program,
        &facts.borrow,
        &facts.flow,
        &direct,
        &reborrows,
        &installation,
        &dispositions,
        &containments,
        mutation_summaries,
    )?;
    install_borrow_resources(
        &mut facts.borrow,
        direct,
        &reborrows,
        &installation,
        &dispositions,
        &containments,
        &restored_uses,
    );
    Ok(())
}

/// Independently replay every retained resource from the authoritative loan
/// and flow-lifetime ledgers, then transactionally rebuild both arenas with
/// remapped typed parent handles. The rows never participate in admission.
pub(super) fn replay_checked_direct_borrow_resources(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    replay_checked_direct_reborrow_lineage(program, &facts.borrow)?;
    let expected_direct = reconstruct_direct_borrow_resources(program, &facts.borrow, &facts.flow)?;
    let expected_reborrows = reconstruct_reborrow_resource_drafts(&facts.borrow, &facts.flow)?;
    let retained = facts
        .borrow
        .direct_loan_resources
        .iter()
        .map(|(_, resource)| resource.clone())
        .collect::<Vec<_>>();
    if retained != expected_direct {
        return Err(vec![Diagnostic::error(
            "checked direct-root borrow resource closure drifted from independent replay",
        )]);
    }
    validate_retained_reborrow_resources(&facts.borrow, &expected_reborrows)?;
    let installation = plan_resource_installation(&expected_direct, &expected_reborrows)?;
    let dispositions = plan_reborrow_disposition_events(
        &facts.flow,
        &expected_direct,
        &expected_reborrows,
        &installation,
    )?;
    validate_retained_disposition_events(
        &facts.borrow,
        &expected_direct,
        &expected_reborrows,
        &dispositions,
    )?;
    let containments = plan_reborrow_containment_certificates(
        &expected_direct,
        &expected_reborrows,
        &installation,
    )?;
    validate_retained_containment_certificates(
        &facts.borrow,
        &expected_direct,
        &expected_reborrows,
        &containments,
    )?;
    let restored_uses = plan_reborrow_restored_call_uses(
        program,
        &facts.borrow,
        &facts.flow,
        &expected_direct,
        &expected_reborrows,
        &installation,
        &dispositions,
        &containments,
        mutation_summaries,
    )?;
    validate_retained_restored_call_uses(
        &facts.borrow,
        &expected_direct,
        &expected_reborrows,
        &dispositions,
        &containments,
        &restored_uses,
    )?;

    install_borrow_resources(
        &mut facts.borrow,
        expected_direct,
        &expected_reborrows,
        &installation,
        &dispositions,
        &containments,
        &restored_uses,
    );
    Ok(())
}
