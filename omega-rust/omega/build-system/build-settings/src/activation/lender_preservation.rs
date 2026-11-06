//! Source checks must fulfill the lender duties they use even without running
//! the program. The retained target companion supplies the selected code; the
//! canonical call captures and independent write judgment remain authoritative.

use backend_plan::TerminalTargetProviderProposal;
use terminal_codec::CanonicalTerminalArtifact;

pub fn complete(
    artifact: &CanonicalTerminalArtifact,
    primary: &str,
    proposals: &[TerminalTargetProviderProposal],
) -> Result<(), String> {
    let module = artifact.semantic_module();
    if module.boundary_lender_preservation.is_empty()
        && module.boundary_reference_forwarding.is_empty()
    {
        return Ok(());
    }
    let mut checked = false;
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.application_required || proposal.target == primary)
    {
        if proposal.terminal_psi != artifact.view().identity() {
            return Err("lender preservation inputs belong to another artifact".into());
        }
        let providers = proposal
            .selection
            .as_ref()
            .map_err(|reasons| reasons.join("; "))?
            .admit(artifact.view(), &proposal.target)?;
        let targets = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            &proposal.target,
        );
        terminal_installation::require_selected_lender_preservation(
            artifact.view(),
            &providers,
            &targets,
        )?;
        checked = true;
    }
    if !checked {
        return Err("lender preservation has no selected target provider inputs".into());
    }
    Ok(())
}
