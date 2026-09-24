//! Settled `CompositionMode::Independent` edges at the product boundary.
//!
//! Settlement proves each `Independent` selection is realizable: the
//! component-closure fence in `provider-planning` has already joined the edge
//! to exactly one verified component description, or rejected it. But the
//! verified component only proves the closure exists — the Terminal product
//! still defines no symbolic import/export table, no entry/leave resource
//! carrier, and no installation/replacement obligation for it, so every
//! emitted artifact is a single fused composition. Emitting one anyway would
//! silently deliver a different composition than the build selected: the
//! provider would arrive inlined into the consumer's image, never installed
//! as the component the description describes.
//!
//! Both product routes therefore replay the retained selection provenance
//! before producing anything and reject every settled `Independent` edge
//! here rather than falling back to a fused artifact — the same contract the
//! settlement fence applies when no verified component exists. Check-level
//! compilations are unaffected: settlement itself still succeeds, so
//! component descriptions can be published and verified from the checked
//! result.
//!
//! Revisit this fence when the product gains a real substrate for a deployed
//! component; until then it must reject, never degrade to Fused.

use assembled_syntax_to_checked_compilation::CheckedCompilation;
use diagnostics::Diagnostic;
use provider_planning::CompositionMode;

/// Reject production while any selected provider plan settled with an
/// `Independent` composition edge the product cannot carry. A provenance row
/// whose composition mode is incoherent rejects as well: settlement owns
/// that rejection, but the checked result is never a license to emit.
pub(crate) fn verify_selected_compositions_are_realized(
    checked: &CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = checked
        .selected_provider_provenance()
        .iter()
        .filter_map(|row| match row.selected_by.composition_mode() {
            Ok(CompositionMode::Fused) => None,
            Ok(CompositionMode::Independent) => Some(Diagnostic::error(format!(
                "selected provider plan `{}` settled with an independent composition edge to provider `{}`, but the product defines no symbolic import/export, entry/leave, or installation/replacement carrier for it; refusing to emit a fused artifact",
                row.plan.name, row.plan.provider_type,
            ))),
            Err(reason) => Some(Diagnostic::error(format!(
                "selected provider plan `{}` retained no coherent composition mode ({reason}); refusing to emit a fused artifact",
                row.plan.name,
            ))),
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
