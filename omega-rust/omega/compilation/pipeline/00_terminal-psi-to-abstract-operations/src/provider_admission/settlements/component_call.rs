//! Physical component transport rejoins an already checked cross-artifact call.
//! No ForeignBinding row, callback body composition or address authority is
//! inferred from this locator. Actual image and installed-era custody stay with
//! native production and the runtime root.

use diagnostics::Diagnostic;

pub(super) fn rejoin_component_call(
    call: &crate::AdmittedNativeComponentCall<'_>,
    authority: &super::super::AdmittedNativeCallableAuthority,
    locator: &target::NormalizedComponentLocator,
    same_stack: &task_plans::AdmittedSameStackContribution,
    selected_plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
    identities: &terminal_psi::TerminalIdentityTable,
    receiver: terminal_psi::TerminalPsiIdentity,
    profile: target::TargetProfile,
) -> Result<target_operations::NormalizedForeignCallBinding, Vec<Diagnostic>> {
    let selection = call.selection();
    let application = call.application();
    let selected_commitment = task_plans::SameStackProviderPlanCommitment::from_digest(
        *selected_plan.identity_digest().as_bytes(),
    );
    if authority.subject() != selection.provider()
        || authority.entry() != selection.candidate()
        || authority.review().terminal_artifact_identity()
            != *call.component().artifact().manifest().identity().as_bytes()
        || authority.review().target() != profile.native_target()
        || selection.receiver() != &terminal_codec::TerminalExecutionIdentity::portable(receiver)
        || selection.provider() != call.component().subject()
        || call.plan_digest() != &selected_plan.identity_digest()
        || application.requirement_identity != requirement
        || application.target != profile.native_target()
        || locator.target() != profile
        || locator.slot_commitment()
            != &target::component_export_slot(
                selection.provider().vocabulary_marker.get(),
                selection.provider().program_fingerprint.as_bytes(),
                &application.commitment,
            )
        || application.recomputed_commitment().ok() != Some(application.commitment)
        || same_stack.provider_plan_report_identity() != selected_plan.report_fingerprint()
        || same_stack.provider_plan_commitment() != selected_commitment
        || identities.index_of(requirement) != Some(same_stack.requirement_identity())
    {
        return Err(vec![Diagnostic::error(format!(
            "component realization for `{requirement}` lost its exact artifact, selected plan, ABI application or same-stack custody",
        ))]);
    }
    Ok(target_operations::NormalizedForeignCallBinding {
        boundary_entry_plan: application.plan.plan().clone(),
        mechanism: target_operations::NormalizedForeignCallMechanism::Component {
            locator: locator.clone(),
            provider: selection.provider(),
            candidate: selection.candidate(),
            application_commitment: application.commitment,
            same_stack_contribution: same_stack.clone(),
        },
    })
}
