//! Independent cycle admission over every installed canonical instance.
//!
//! Source-only checking may reject conservatively before pruning. Receiving
//! admission cannot trust that earlier pass or an asserted call summary: it
//! reads the retained bodies, including uncalled closed instances. An inert
//! source template has no installed body and grants no executable edge.

use effects::{SelectedProviderPlanFacts, provider_plan::ProviderPlan};
use terminal_codec::TerminalArtifactView;

use super::{TerminalTargetProviderPlan, selected_checked_candidates};
use crate::synchronous_invocations::{
    SynchronousBoundaryTarget, validate_selected_synchronous_invocation_cycles,
};

pub(super) fn validate(
    artifact: TerminalArtifactView<'_>,
    target: &str,
    selected: &SelectedProviderPlanFacts,
    declarations: &[TerminalTargetProviderPlan],
) -> Result<(), String> {
    let module = artifact.module();
    let candidates = selected_checked_candidates(declarations, module)?;
    let calls =
        terminal_installation::SynchronousCallGraph::new(module, None)?;
    validate_selected_synchronous_invocation_cycles(selected.plans(), |plan, row| {
        let failure = |reason: String| {
            diagnostics::Diagnostic::error(format!(
                "selected provider `{}` synchronous invocation admission failed: {reason}",
                plan.name,
            ))
        };
        let matched = candidates
            .iter()
            .find(|candidate| &candidate.selected.plan == plan && candidate.row == row)
            .ok_or_else(|| {
                failure("checked row lost its canonical selection association".into())
            })?;
        let mut targets = Vec::new();
        for candidate in &matched.candidates {
            let reached = calls.from_root(candidate.candidate).map_err(&failure)?;
            for boundary in reached.boundaries {
                let declaration = module.boundary_machine(boundary).ok_or_else(|| {
                    failure("invoked boundary has no canonical declaration".into())
                })?;
                let requirement = module
                    .identities
                    .text(declaration.identity)
                    .ok_or_else(|| failure("invoked boundary has no canonical identity".into()))?;
                let supplied = selected
                    .assumed_supplier_identity(requirement)
                    .map_err(&failure)?;
                let mut slots = candidates
                    .iter()
                    .filter(|matched| {
                        matched.candidates.iter().any(|candidate| {
                            module.identities.text(candidate.requirement_identity) == Some(supplied)
                        })
                    })
                    .map(|matched| &matched.selected.plan)
                    .collect::<Vec<_>>();
                if let Some((plan, _)) = selected.selected_requirement_row(supplied) {
                    slots.push(plan);
                }
                slots.dedup();
                // Ordinary supply admission reports a missing provider.
                // Its absence never manufactures a synchronous edge.
                match slots.as_slice() {
                    [] => {}
                    [slot] => targets.push(slot_target(slot)),
                    _ => {
                        return Err(failure(format!(
                            "invoked requirement `{requirement}` has competing selected slots"
                        )));
                    }
                }
            }
        }
        Ok(targets)
    })
    .map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn slot_target(plan: &ProviderPlan) -> SynchronousBoundaryTarget {
    SynchronousBoundaryTarget::Selected {
        plan: plan.identity_digest(),
        label: plan.name.clone(),
    }
}
