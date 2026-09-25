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
use provider_planning::calling_policy_plans::{
    BoundaryCallingPlanRealization, BoundaryValueClass, MaterializedBoundarySignature,
};

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
        target::ProgramEntryCallingConvention::Aapcs64 => {
            calling_conventions::CallingPolicy::Aapcs64
        }
        target::ProgramEntryCallingConvention::SystemVAMD64 => {
            calling_conventions::CallingPolicy::SystemVAMD64
        }
    };
    if semantic_plan.plan().call.policy != expected_policy(expected_semantic)
        || physical_plan.plan().call.policy != expected_policy(expected_physical)
    {
        return Err("selected ProgramEntry calling policies drifted from their target slot".into());
    }

    // The freestanding bridge forwards its two roots to source. Hosted entry
    // instead provisions two bridge-internal Extent roots under its own
    // adapter: the source signature retains zero visible parameters while the
    // semantic requirement retains exactly two Extent inputs. Internal roots
    // must never be treated as zero source-visible inputs by omission.
    let semantic_shapes = semantic_plan
        .plan()
        .call
        .parameters
        .iter()
        .map(|placement| placement.shape)
        .collect::<Vec<_>>();
    let semantic_source_pairing = match slot.schema {
        target::ProgramEntrySchema::ProgramStorageApplication => {
            semantic_method.parameter_type_identities
                == source
                    .visible_parameters()
                    .iter()
                    .map(|parameter| parameter.normalized_type_identity().to_owned())
                    .collect::<Vec<_>>()
                && semantic_shapes
                    == source
                        .visible_parameters()
                        .iter()
                        .map(|parameter| parameter.value_shape())
                        .collect::<Vec<_>>()
        }
        target::ProgramEntrySchema::HostedApplication => {
            source.visible_parameters().is_empty()
                && semantic_method.parameter_count == 2
                && semantic_method.parameter_type_identities.len() == 2
                && semantic_method
                    .parameter_type_identities
                    .iter()
                    .all(|identity| identity == &semantic_method.parameter_type_identities[0])
                && semantic_shapes == [calling_conventions::ValueShape::integer(16, 8); 2]
                && has_internal_storage_signature(semantic.materialized_signature())
        }
    };
    if !semantic_source_pairing
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

// ABI-sized scalars cannot substitute for the adapter's internal Extent records.
// Exact accepted schema/application custody above supplies semantic identity;
// this independent graph check establishes geometry, not storage authority.
fn has_internal_storage_signature(signature: &MaterializedBoundarySignature) -> bool {
    signature.parameters().len() == 2
        && signature.result().is_none()
        && signature.parameters().iter().all(|root| {
            let Some(shape) = signature.shapes().get(usize::from(*root)) else {
                return false;
            };
            let BoundaryValueClass::Record {
                first_field,
                field_count: 2,
            } = shape.class()
            else {
                return false;
            };
            if shape.byte_size() != 16 || shape.alignment() != 8 {
                return false;
            }
            let Some(fields) = signature
                .fields()
                .get(usize::from(first_field)..)
                .and_then(|fields| fields.get(..2))
            else {
                return false;
            };
            fields.iter().zip([0, 8]).all(|(field, offset)| {
                field.byte_offset() == offset
                    && signature
                        .shapes()
                        .get(usize::from(field.shape()))
                        .is_some_and(|word| {
                            word.class() == BoundaryValueClass::Integer
                                && word.byte_size() == 8
                                && word.alignment() == 8
                        })
            })
        })
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
