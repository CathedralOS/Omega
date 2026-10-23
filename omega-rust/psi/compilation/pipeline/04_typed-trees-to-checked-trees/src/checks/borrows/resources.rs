//! Construct the resource rows consumed by execution planning and stage 05.
//! Formation and flow own loan ancestry and lifetime events. This owner joins
//! those facts once, checks access attenuation, and builds the parent graph
//! and disposition events required by lowering. Each disposition event whose
//! boundary is a checked post-restoration mutating call carries that call's
//! exact join. No second construction replays them.

mod call_operands;
mod disposition_events;
mod lifecycle;
mod loan_resources;
mod reborrow_drafts;

use crate::checks::borrows::resources::disposition_events::{
    bind_restored_call_uses, plan_reborrow_disposition_events,
};
use crate::checks::borrows::resources::loan_resources::{
    install_borrow_resources, plan_resource_installation, reconstruct_borrow_resource_rosters,
};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

/// Build once before the checks and preserve the resulting handles for lowering.
pub(super) fn initialize_checked_direct_borrow_resources(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    let (direct, reborrows) =
        reconstruct_borrow_resource_rosters(program, &facts.borrow, &facts.flow)?;
    let installation = plan_resource_installation(&direct, &reborrows)?;
    let mut dispositions = plan_reborrow_disposition_events(
        &facts.borrow,
        &facts.flow,
        &direct,
        &reborrows,
        &installation,
    )?;
    bind_restored_call_uses(
        program,
        &facts.borrow,
        &facts.flow,
        &direct,
        &reborrows,
        &mut dispositions,
        mutation_summaries,
    )?;
    install_borrow_resources(
        &mut facts.borrow,
        direct,
        &reborrows,
        &installation,
        &dispositions,
    );
    call_operands::install(program, facts);
    Ok(())
}

pub(crate) fn reborrow_disposition_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow resource-lifecycle disposition drifted from semantic-phase replay",
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
