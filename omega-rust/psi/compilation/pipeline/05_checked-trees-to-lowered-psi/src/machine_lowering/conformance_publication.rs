//! Closed conformance application publication for a selected module.
//!
//! The producer records in [`ConformancePublication`] how the module's
//! conformance applications must reach Terminal Psi: dynamic dispatch keeps
//! only the root-scoped applications the exact source owners attest, a
//! rebound root admits one rebound application beside the selected root, a
//! joined module publishes its bounded roster directly, and an ordinary
//! selection reconstructs every application from its checked closure.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;

use crate::lowering_error::{LoweringError, unsupported};
use crate::producer_result::ConformancePublication;
use crate::retention::conformance_applications::{self, lower_closed_conformance_applications};

/// Publish the closed conformance applications the selected module's
/// publication route requires, or refuse a route whose emitted roster cannot
/// carry the dispatch.
pub(crate) fn publish_selected_conformance_applications(
    checked: &CheckedTrees,
    entry: SymbolHandle,
    publication: &ConformancePublication,
    has_exact_source_owners: bool,
    projection_sources: &[(SymbolHandle, MachineId)],
    source_machines: &[SymbolHandle],
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    let dynamic_root_application_count = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .filter(|application| {
            !has_exact_source_owners || application.owner == lowered.semantic_module.entry
        })
        .count();
    match publication {
        ConformancePublication::ExactRoot if dynamic_root_application_count != 1 => {
            return unsupported(
                "direct dynamic dispatch must publish exactly one closed conformance application",
            );
        }
        ConformancePublication::BoundedRoot
            if !(1..=2).contains(&dynamic_root_application_count) =>
        {
            return unsupported(
                "rebound dynamic dispatch must publish one or two closed conformance applications",
            );
        }
        ConformancePublication::BoundedModule
            if !(1..=2).contains(
                &lowered
                    .semantic_module
                    .closed_conformance_applications
                    .len(),
            ) =>
        {
            return unsupported(
                "joined dynamic dispatch must publish one or two closed conformance applications",
            );
        }
        ConformancePublication::Reconstruct => {
            lower_closed_conformance_applications(
                checked,
                source_machines,
                &mut lowered.semantic_module,
            )?;
        }
        _ => {}
    }
    if has_exact_source_owners
        && matches!(
            publication,
            ConformancePublication::ExactRoot | ConformancePublication::BoundedRoot
        )
    {
        conformance_applications::append_closed_conformance_applications_excluding(
            checked,
            projection_sources,
            entry,
            &mut lowered.semantic_module,
        )?;
    }
    Ok(())
}
