//! Join evaluated Calling applications to an already prepared entry contract.
//!
//! No Source context, declaration walk or type-identity lookup is available
//! here. This consumes owned preparation in the original refusal order.

use super::SelectedProgramEntryCallingPlans;
use super::source_contract::{PreparedEntryCallingContract, PreparedEntrySurface};
use diagnostics::Diagnostic;

pub(super) fn settle(
    slot: target::ProgramEntrySlotDeclaration,
    contract: PreparedEntryCallingContract,
    realizations: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
) -> Result<SelectedProgramEntryCallingPlans, Vec<Diagnostic>> {
    let PreparedEntryCallingContract {
        schema,
        schema_name,
        semantic_signature,
        semantic_identity,
        physical_signature,
        physical_identity,
        physical_requirement,
        physical_convention,
        semantic_convention,
        surface,
        parameter_type_identities,
        result_type_identity,
        accepted_binding,
    } = contract;
    let semantic_matching = realizations
        .iter()
        .filter(|realization| {
            realization.boundary_trait == schema
                && realization.requirement_machine == semantic_signature
        })
        .collect::<Vec<_>>();
    let [semantic_realization] = semantic_matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` retains {} evaluated calling plans for semantic requirement `{}` instead of exactly one",
            semantic_matching.len(),
            slot.semantic_arrival_requirement,
        ))]);
    };
    let physical_matching = realizations
        .iter()
        .filter(|realization| {
            realization.boundary_trait == schema
                && realization.requirement_machine == physical_signature
        })
        .collect::<Vec<_>>();
    let [physical_realization] = physical_matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` retains {} evaluated calling plans for physical requirement `{physical_requirement}` instead of exactly one",
            physical_matching.len(),
        ))]);
    };
    for (role, realization) in [
        ("semantic", *semantic_realization),
        ("physical", *physical_realization),
    ] {
        let (validated, application_report_fingerprint, application_commitment) = realization
            .replayed_validated_application()
            .map_err(|error| {
            vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` retains an invalid {role} calling plan: {error}"
            ))]
        })?;
        if realization.exact_boundary_entry_plan() != validated.plan()
            || realization.report_fingerprint != application_report_fingerprint
            || realization.commitment != application_commitment
        {
            return Err(vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` does not retain exact {role} calling-plan identity and custody"
            ))]);
        }
    }
    let (validated_physical_plan, _, _) = physical_realization
        .replayed_validated_application()
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` lost its validated physical calling plan: {error}"
            ))]
        })?;
    let expected_physical =
        calling_conventions::CallingPolicy::for_program_entry_convention(physical_convention);
    if physical_realization.boundary_entry_plan.call.policy != expected_physical {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` evaluates physical requirement `{physical_requirement}` with {:?}, but `{}::{}` requires {:?}",
            physical_realization.boundary_entry_plan.call.policy,
            slot.owner.root_slot_owner_name(),
            slot.slot_name,
            expected_physical,
        ))]);
    }
    let expected_semantic =
        calling_conventions::CallingPolicy::for_program_entry_convention(semantic_convention);
    if semantic_realization.boundary_entry_plan.call.policy != expected_semantic {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` evaluates semantic requirement `{}` with {:?}, but `{}::{}` requires {:?}",
            slot.semantic_arrival_requirement,
            semantic_realization.boundary_entry_plan.call.policy,
            slot.owner.root_slot_owner_name(),
            slot.slot_name,
            expected_semantic,
        ))]);
    }
    let PreparedEntrySurface {
        declaration,
        physical_source,
    } = surface?;
    let prepared = declaration
        .surface()
        .map_err(|diagnostic| vec![diagnostic])?;
    let service_schema = crate::service_schema::attach_calling(&prepared, realizations);
    let physical_source = physical_source?
        .validate(
            &service_schema,
            &physical_identity,
            accepted_binding.as_ref(),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    let storage_entry = program_entry_plan::SelectedProgramStorageEntryPlan::from_target_slot(
        slot,
        service_schema,
        semantic_identity,
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    let result_type_identity = result_type_identity.ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "physical entry requirement `{physical_requirement}` has no result"
        ))]
    })?;
    let physical_contract = program_entry_plan::ProgramEntryPhysicalContractPlan::new(
        slot,
        physical_identity,
        physical_source.package,
        physical_source.package_source_digest,
        physical_source.non_authoritative_package_source_report_fingerprint,
        parameter_type_identities,
        result_type_identity,
        validated_physical_plan.contract_report_fingerprint(),
        physical_realization.boundary_entry_plan.clone(),
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])?;
    let storage_entry = storage_entry
        .with_physical_contract(physical_contract.clone())
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    Ok(SelectedProgramEntryCallingPlans {
        semantic_calling_application: semantic_realization
            .native_application()
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        physical_calling_application: physical_realization
            .native_application()
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        storage_entry,
    })
}
