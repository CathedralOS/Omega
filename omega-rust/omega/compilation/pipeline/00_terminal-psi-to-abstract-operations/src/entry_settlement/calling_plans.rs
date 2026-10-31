//! Join completed calling applications to the selected entry contract.
//!
//! A schema commits to a calling application (requirement, target, source
//! shape graph, and ABI plan), not merely to the ABI plan. Keep both produced
//! applications through this boundary; reconstructing their identity from
//! placements loses the contract identity that native settlement must join.
//! Calling-policy evaluation validates the plan once. This consumer checks
//! its relationship to the selected entry, not the producer's work again.
//! Slot spellings locate schema requirements; normalized overload identities
//! join the retained applications. Those identity domains are not interchangeable.

use super::model::ValidatedNativeProgramEntryCallingPlans;
use calling_conventions::ValidatedBoundaryEntryPlan;
use calling_conventions::{
    BoundaryCallingApplication, BoundarySignatureGeometry, BoundaryValueClass,
};
use effects::provider_plan::{ServiceMethod, ServiceSchema};
use program_entry_plan::{SelectedProgramEntrySignature, SelectedProgramStorageEntryPlan};

pub(crate) fn validate_paired_calling_plans(
    source: &SelectedProgramEntrySignature,
    semantic: &BoundaryCallingApplication,
    physical: &BoundaryCallingApplication,
    storage: &SelectedProgramStorageEntryPlan,
) -> Result<ValidatedNativeProgramEntryCallingPlans, String> {
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
    if Some(
        storage
            .schema()
            .identity_text(semantic_method.requirement_identity),
    ) != storage.identities().text(storage.requirement_identity())
        || semantic_method.requirement_identity == physical_method.requirement_identity
    {
        return Err("selected ProgramEntry lost its distinct exact arrival requirements".into());
    }
    let semantic_plan = join_application(
        storage.schema(),
        semantic,
        semantic_method,
        slot.owner.native_target(),
    )?;
    let physical_plan = join_application(
        storage.schema(),
        physical,
        physical_method,
        slot.owner.native_target(),
    )?;
    if semantic_plan.plan().call.policy
        != calling_conventions::CallingPolicy::for_program_entry_convention(expected_semantic)
        || physical_plan.plan().call.policy
            != calling_conventions::CallingPolicy::for_program_entry_convention(expected_physical)
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
            semantic_method
                .parameter_type_identities
                .iter()
                .map(|identity| storage.schema().declaration.identities.text(*identity))
                .eq(source.visible_parameters().iter().map(|parameter| {
                    source
                        .identities()
                        .text(parameter.normalized_type_identity())
                        .unwrap_or_default()
                }))
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
                && has_internal_storage_signature(&semantic.signature)
        }
    };
    if !semantic_source_pairing
        || semantic_method.has_result
        || semantic_method.result_type_identity.is_valid()
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
        || contract.identities().text(contract.requirement_identity())
            != Some(
                storage
                    .schema()
                    .identity_text(physical_method.requirement_identity),
            )
        || !contract
            .parameter_type_identities()
            .iter()
            .zip(physical_method.parameter_type_identities.iter())
            .all(|(left, right)| {
                contract.identities().text(*left)
                    == Some(storage.schema().declaration.identities.text(*right))
            })
        || contract.parameter_type_identities().len()
            != physical_method.parameter_type_identities.len()
        || !physical_method.has_result
        || Some(
            storage
                .schema()
                .declaration
                .identities
                .text(physical_method.result_type_identity),
        ) != contract.identities().text(contract.result_type_identity())
        || contract.boundary_entry_plan() != physical_plan.plan()
        || contract.calling_plan_report_fingerprint() != physical_plan.contract_report_fingerprint()
    {
        return Err("selected ProgramEntry physical plan drifted from its target contract".into());
    }
    Ok(ValidatedNativeProgramEntryCallingPlans {
        semantic: semantic_plan.clone(),
        physical: physical_plan.clone(),
    })
}

// ABI-sized scalars cannot substitute for the adapter's internal Extent records.
// Exact accepted schema/application custody above supplies semantic identity;
// this independent graph check establishes geometry, not storage authority.
fn has_internal_storage_signature(signature: &BoundarySignatureGeometry) -> bool {
    signature.parameters.len() == 2
        && signature.result.is_none()
        && signature.parameters.iter().all(|root| {
            let Some(shape) = signature.shapes.get(usize::from(*root)) else {
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
                .fields
                .get(usize::from(first_field)..)
                .and_then(|fields| fields.get(..2))
            else {
                return false;
            };
            fields.iter().zip([0, 8]).all(|(field, offset)| {
                field.byte_offset() == offset
                    && signature
                        .shapes
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
        .declaration
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

fn join_application<'application>(
    schema: &ServiceSchema,
    application: &'application BoundaryCallingApplication,
    method: &ServiceMethod,
    target: target::NativeTarget,
) -> Result<&'application ValidatedBoundaryEntryPlan, String> {
    if application.target != target
        || application.requirement_identity != schema.identity_text(method.requirement_identity)
        || schema
            .calling_plan(method.requirement_identity)
            .map(|plan| plan.report_fingerprint)
            != Some(application.report_fingerprint)
        || schema
            .calling_plan(method.requirement_identity)
            .map(|plan| plan.commitment)
            .map(|commitment| commitment.as_bytes())
            != Some(application.commitment)
    {
        return Err(
            "selected ProgramEntry lost exact calling-application identity and custody".into(),
        );
    }
    Ok(&application.plan)
}
