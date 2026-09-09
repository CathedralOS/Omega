//! Replay source applications before consuming their ABI plans.
//!
//! A schema commits to a calling application (requirement, target, source
//! shape graph, and ABI plan), not merely to the ABI plan. Keep both selected
//! applications through this boundary; reconstructing their identity from
//! placements loses the very source custody that native settlement must check.
//! Slot spellings locate schema requirements; normalized overload identities
//! join the retained applications. Those identity domains are not interchangeable.

use calling_conventions::ValidatedBoundaryEntryPlan;
use effects::provider_plan::ServiceMethod;
use program_entry_plan::{SelectedProgramEntrySourceSignature, SelectedProgramStorageEntryPlan};
use provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;

pub(crate) fn validate_paired_calling_plans(
    source: &SelectedProgramEntrySourceSignature,
    semantic: &BoundaryCallingPlanRealization,
    physical: &BoundaryCallingPlanRealization,
    storage: &SelectedProgramStorageEntryPlan,
) -> Result<(), String> {
    let slot = source.target_slot();
    let (Some(expected_semantic), Some(expected_physical), Some(physical_requirement)) = (
        slot.semantic_calling_convention,
        slot.physical_calling_convention,
        slot.physical_arrival_requirement,
    ) else {
        return Err(
            "selected ProgramEntry has an incomplete two-surface calling declaration".into(),
        );
    };
    if storage.target_slot() != slot {
        return Err("selected ProgramEntry storage contract drifted from its target slot".into());
    }
    let semantic_method = selected_method(storage, slot.semantic_arrival_requirement)?;
    let physical_method = selected_method(storage, physical_requirement)?;
    if semantic_method.requirement_identity != storage.requirement_identity()
        || semantic_method.requirement_identity == physical_method.requirement_identity
    {
        return Err("selected ProgramEntry lost its distinct exact arrival requirements".into());
    }
    let semantic_plan = replay_application(semantic, semantic_method, slot.owner.native_target())?;
    let physical_plan = replay_application(physical, physical_method, slot.owner.native_target())?;
    let expected_policy = |convention| match convention {
        target::ProgramEntryCallingConvention::MicrosoftX64 => {
            calling_conventions::CallingPolicy::MicrosoftX64
        }
    };
    if semantic_plan.plan().call.policy != expected_policy(expected_semantic)
        || physical_plan.plan().call.policy != expected_policy(expected_physical)
    {
        return Err("selected ProgramEntry calling policies drifted from their target slot".into());
    }

    // The current freestanding bridge forwards these two roots to source.
    // Hosted entry needs its own explicit adapter mapping; two internal storage
    // inputs must never be treated as zero source-visible inputs by omission.
    if semantic_method.parameter_type_identities
        != source
            .visible_parameters()
            .iter()
            .map(|parameter| parameter.normalized_type_identity().to_owned())
            .collect::<Vec<_>>()
        || !semantic_plan
            .plan()
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .eq(source
                .visible_parameters()
                .iter()
                .map(|parameter| parameter.value_shape()))
        || semantic_method.has_result
        || semantic_method.result_type_identity.is_some()
        || semantic_plan.plan().call.result.is_some()
    {
        return Err(
            "selected ProgramEntry semantic plan is not paired with its source signature".into(),
        );
    }
    let contract = storage
        .physical_contract()
        .ok_or_else(|| "selected ProgramEntry lost its physical calling contract".to_owned())?;
    if contract.target_slot() != slot
        || contract.requirement_identity() != physical_method.requirement_identity
        || contract.parameter_type_identities() != physical_method.parameter_type_identities
        || !physical_method.has_result
        || physical_method.result_type_identity.as_deref() != Some(contract.result_type_identity())
        || contract.boundary_entry_plan() != physical_plan.plan()
        || contract.calling_plan_report_fingerprint() != physical_plan.contract_report_fingerprint()
    {
        return Err("selected ProgramEntry physical plan drifted from its target contract".into());
    }
    Ok(())
}

fn selected_method<'schema>(
    storage: &'schema SelectedProgramStorageEntryPlan,
    spelling: &str,
) -> Result<&'schema ServiceMethod, String> {
    let (owner, name) = spelling.split_once("::").ok_or_else(|| {
        "selected ProgramEntry has an invalid target requirement spelling".to_owned()
    })?;
    let mut methods = storage
        .schema()
        .methods
        .iter()
        .filter(|method| method.requirement_owner == owner && method.name == name);
    let method = methods
        .next()
        .ok_or_else(|| format!("selected ProgramEntry schema lost requirement `{spelling}`"))?;
    if methods.next().is_some() {
        return Err(format!(
            "selected ProgramEntry schema repeats requirement `{spelling}`"
        ));
    }
    Ok(method)
}

fn replay_application(
    application: &BoundaryCallingPlanRealization,
    method: &ServiceMethod,
    target: target::NativeTarget,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    let (validated, report, commitment) =
        application
            .replayed_validated_application()
            .map_err(|error| {
                format!("selected ProgramEntry calling application is invalid: {error}")
            })?;
    if application.materialized_signature().native_target() != target
        || application
            .materialized_signature()
            .owner_requirement_identity()
            != method.requirement_identity
        || application.exact_boundary_entry_plan() != validated.plan()
        || application.report_fingerprint != report
        || application.commitment != commitment
        || method.calling_plan_report_fingerprint != Some(report)
        || method.calling_plan_commitment != Some(commitment)
    {
        return Err(
            "selected ProgramEntry lost exact calling-application identity and custody".into(),
        );
    }
    Ok(validated)
}
