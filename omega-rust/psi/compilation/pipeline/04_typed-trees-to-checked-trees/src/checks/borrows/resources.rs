//! Construct the resource rows consumed by execution planning and stage 05.
//! Formation and flow own loan ancestry and lifetime events. This owner joins
//! those facts once, checks access attenuation, and builds the parent graph,
//! dispositions, containment and restored-call records required by lowering.
//! These latter records still have live consumers; they are not the removed
//! checker-only compatibility ledgers. No second construction replays them.

mod certificate_planning;
mod disposition_events;
mod lifecycle;
pub(crate) use lifecycle::{LifecyclePhase, weakening_event_boundary};
mod reborrow_drafts;
mod resource_reconstruction;

use crate::checks::borrows::resources::certificate_planning::{
    plan_reborrow_containment_certificates, plan_reborrow_restored_call_uses,
    plan_resource_installation,
};
use crate::checks::borrows::resources::disposition_events::plan_reborrow_disposition_events;
use crate::checks::borrows::resources::resource_reconstruction::{
    install_borrow_resources, reconstruct_direct_borrow_resources,
    reconstruct_reborrow_resource_drafts,
};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

/// Build once before the checks and preserve the resulting handles for lowering.
pub(super) fn initialize_checked_direct_borrow_resources(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
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

pub(crate) fn reborrow_disposition_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow resource-lifecycle disposition drifted from semantic-phase replay",
    )]
}

pub(crate) fn reborrow_containment_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow suspension/freeze-containment evidence drifted from exact lifecycle replay",
    )]
}

pub(crate) fn reborrow_restored_call_use_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow restored mutating-call use drifted from exact lifecycle and call replay",
    )]
}

pub(crate) fn reborrow_resource_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked direct-reborrow resource closure drifted from independent topological replay",
    )]
}

pub(crate) fn invalid_reborrow_attenuation_diagnostic(
    parent: &checked_trees::BorrowAccessKind,
    child: &checked_trees::BorrowAccessKind,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot derive {} reborrow authority from an exact {} parent loan; allowed direct reborrow access pairs are Read->Read, Mutable->Read, Mutable->Mutable, Mutable->WriteOnly, and WriteOnly->WriteOnly",
        borrow_access_name(child),
        borrow_access_name(parent),
    ))
}

fn borrow_access_name(access: &checked_trees::BorrowAccessKind) -> &'static str {
    match access {
        checked_trees::BorrowAccessKind::Read => "Read",
        checked_trees::BorrowAccessKind::Mutable => "Mutable",
        checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
    }
}
