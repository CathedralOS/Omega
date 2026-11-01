//! Discharge retained scalar obligations before executing a pure calling policy.
//!
//! Publishing the policy's exact application roots retains their conditions in
//! Terminal Psi. Being compiler-run code does not discharge those conditions.
//! Use Psi's normal completion engine with this exact entry's demand and empty
//! provider authority; asking provider settlement to supply itself would cycle.

use std::sync::Arc;

use build_time_evaluation::{ScalarScopeEvaluation, SourceCompletionAccounting};
use evaluation_accounting::EvaluationInvocationAccount;
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_installation::AdmittedScalarApplications;
use terminal_interpreter::{TerminalProgram, TerminalRuntimeSupplies};

pub(super) fn complete(
    artifact: &Arc<CanonicalTerminalArtifact>,
    program: &TerminalProgram<'_>,
    installation: &TerminalRuntimeSupplies,
    target: TargetProfile,
    account: &EvaluationInvocationAccount,
) -> Result<AdmittedScalarApplications, String> {
    let requirements = program
        .execution_requirements(Some(installation))
        .map_err(|error| format!("calling policy scalar demands: {error:?}"))?;
    let providers = terminal_installation::admit_provider_installation(artifact.view(), &[], &[])
        .map_err(|error| format!("calling policy scalar providers: {error:?}"))?;
    // The same activation sponsors completion and execution, so scalar jobs
    // consume its actual fuel/storage allowance rather than running for free.
    let mut accounting = SourceCompletionAccounting::default();
    build_time_evaluation::complete_scalar_applications(
        artifact,
        target.target_name(),
        None,
        &requirements.scalar_applications,
        Some(account),
        &mut accounting,
        |scope| {
            let profile = TargetProfile::from_canonical_target_name(scope)
                .map_err(|diagnostic| diagnostic.message)?;
            Ok(ScalarScopeEvaluation {
                application_roots: None,
                targets: terminal_installation::admit_target_machine_selections(
                    artifact.view(),
                    scope,
                ),
                providers: providers.clone(),
                layout_target: profile
                    .layout_target_facts()
                    .ok_or("calling policy scalar target has no layout facts")?,
                address_model: Ok(target::TargetSemantics::close_address_model(profile).map(
                    |model| terminal_installation::ScalarAddressModel {
                        target: scope.to_owned(),
                        carrier: model.carrier(),
                        semantics_version: model.semantics_version(),
                        commitment: model.commitment(),
                    },
                )),
                carrier_layout: profile
                    .native_realization()
                    .map(runtime_abi::target_carrier_layout),
            })
        },
    )
    .map_err(|reason| format!("calling policy scalar completion: {reason}"))
}
