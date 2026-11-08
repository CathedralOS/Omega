//! Provider supply for the boundary requirements a program entry reaches.
//!
//! Selection chooses among declared candidates, and a missing candidate
//! rejects (`wiki/spec/build/provider_selection.md`).
//! A requirement the entry calls directly, `Requirement::operation(..)`, has
//! no source carrier that could name its boundary early, so its demand is read
//! here from this target's realized plan. Psi's invocation facts union every
//! member of a target family; the realized plan reaches only the member this
//! target selected, so another target's raw family is never demanded.
//!
//! The walk is the closure review's own (`reachability`). This check names the
//! entry's unselected requirements before settlement reports the same omission
//! as a missing import, mechanism or provider row. A selected provider's
//! nested calls are closed by provider planning; the review still refuses any
//! reachable requirement without a row.

use ::diagnostics::Diagnostic;
use language_semantics::type_identity::callable_display_path as requirement_path;

use super::diagnostics::realization_error;
use super::terminal_authority_review::reachability::AuthorityGraph;

pub(super) fn require_reached_providers(
    input: &crate::AdmissionSource<'_>,
    target_profile: target::TargetProfile,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let plan = input.plan();
    let reached = AuthorityGraph::new(input, target_profile)
        .and_then(|graph| graph.reachable_authority_edges(plan.entry))
        .map_err(|error| realization_error("provider supply", error))?;
    let diagnostics = reached
        .boundaries
        .iter()
        .filter_map(|boundary| {
            // An undeclared boundary is the closure review's refusal.
            let declaration = plan
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == *boundary)?;
            if declaration.declaration.as_ref().is_some_and(|row|
                row.execution == terminal_psi::CheckedBoundaryExecution::EmptyRunRelease)
            {
                return None;
            }
            // Private memory operations are not supplied by provider selection.
            // Their operation owner joins the installed profile call and keeps
            // the native-execution fence until backing realization exists.
            if declaration.declaration.as_ref()
                .and_then(|declaration| declaration.execution.placement_operation())
                .is_some() {
                return None;
            }
            let requirement = plan.identities.text(declaration.identity)?;
            (selected
                .selected_requirement_row(requirement)
                .is_none()
                && !selected.assumed_bindings().iter().any(|row| row.required == requirement)
            ).then(|| {
                    Diagnostic::error(format!(
                        "no provider is selected for boundary requirement `{}`, which the program entry reaches on target `{}`",
                        requirement_path(requirement),
                        target_profile.target_name(),
                    ))
                })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
