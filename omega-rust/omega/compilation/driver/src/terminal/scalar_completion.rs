//! Application sponsorship of canonical scalar evaluation.
//!
//! Source checking and native demand share this operation. Provider choices
//! come from the retained Build companion; evaluation never reopens source.

use backend_plan::TerminalTargetProviderProposal;
use std::sync::Arc;

pub(crate) fn complete(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: &str,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    proposals: &[TerminalTargetProviderProposal],
    accounting: &mut build_time_evaluation::SourceCompletionAccounting,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    build_time_evaluation::complete_scalar_applications(
        artifact,
        primary,
        demand,
        accounting,
        |scope| {
            let proposal = proposals
                .iter()
                .find(|proposal| proposal.target == scope)
                .ok_or_else(|| format!("scalar checking has no provider inputs for {scope}"))?;
            let providers = proposal
                .selection
                .as_ref()
                .map_err(|reasons| reasons.join("; "))?
                .admit(artifact.view(), scope)?;
            let profile = target::TargetProfile::from_canonical_target_name(scope)
                .map_err(|diagnostic| diagnostic.message)?;
            let address_model =
                target::TargetSemantics::close_address_model(profile).map(|model| {
                    terminal_installation::ScalarAddressModel {
                        target: scope.to_owned(),
                        carrier: model.carrier(),
                        semantics_version: model.semantics_version(),
                        commitment: model.commitment(),
                    }
                });
            Ok(build_time_evaluation::ScalarScopeEvaluation {
                targets: terminal_installation::admit_target_machine_selections(
                    artifact.view(),
                    scope,
                ),
                providers,
                address_model: Ok(address_model),
            })
        },
    )
}
