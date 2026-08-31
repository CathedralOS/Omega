//! Admission checks for the selected semantic entry and its paired physical plan.

use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, ValidatedBoundaryEntryPlan, ValueShape,
    validate_boundary_entry_plan,
};
use omega_effects::provider_plan::{ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod};
use psi_language_semantics::CarryPolicy;

use crate::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceReceiverSignature, ProgramEntrySourceResultSignature,
    ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole,
    SelectedProgramEntrySourceSignature, SelectedProgramStorageEntryPlan,
};

pub(super) const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
pub(super) const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";
pub(super) const EXTENT_CARRIER: &str = "named(name(Extent))";
pub(super) const GRANTED_DOMAIN: &str = "Extent::Granted";
pub(super) const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
pub(super) const EXTENT_FIELD_SHAPE: ValueShape = ValueShape::integer(8, 8);

pub(super) struct ValidatedSemanticEntryInputs {
    pub(super) target: omega_target::NativeTarget,
    pub(super) slot: omega_target::ProgramEntrySlotDeclaration,
    pub(super) source: SelectedProgramEntrySourceSignature,
    pub(super) boundary: ValidatedBoundaryEntryPlan,
    pub(super) method: ServiceMethod,
    pub(super) physical_contract: ProgramEntryPhysicalContractPlan,
}

pub(super) fn validate(
    target: omega_target::NativeTarget,
    selected: &SelectedProgramStorageEntryPlan,
    source: &SelectedProgramEntrySourceSignature,
    semantic_boundary_entry_plan: &BoundaryEntryPlan,
) -> Result<ValidatedSemanticEntryInputs, ProgramStorageEntryDiagnostic> {
    let slot = selected.target_slot();
    if target != omega_target::NativeTarget::uefi_x64()
        || slot != omega_target::TargetProfile::UefiX64.program_entry_slot()
        || source.target_slot() != slot
        || slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
        || slot.visible_parameters
            != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        || slot.semantic_calling_convention
            != Some(omega_target::ProgramEntryCallingConvention::MicrosoftX64)
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry requires the exact UEFI x86-64 target slot"
                .into(),
        ));
    }
    if source.receiver() != &ProgramEntrySourceReceiverSignature::Free
        || source.result() != ProgramEntrySourceResultSignature::Unit
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry requires one receiver-free Unit source".into(),
        ));
    }

    let [image_source, storage_source] = source.visible_parameters() else {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry requires Image then InitialStorage".into(),
        ));
    };
    validate_source_root(image_source, ProgramStorageEntryRootRole::Image, 0)?;
    validate_source_root(
        storage_source,
        ProgramStorageEntryRootRole::InitialStorage,
        1,
    )?;

    let signature = CallSignature {
        parameters: vec![image_source.value_shape(), storage_source.value_shape()],
        result: None,
    };
    let boundary = validate_boundary_entry_plan(semantic_boundary_entry_plan.clone(), &signature)
        .map_err(|diagnostic| {
        ProgramStorageEntryDiagnostic(format!(
            "optimized semantic ProgramStorage calling plan is invalid: {diagnostic}"
        ))
    })?;
    if boundary.plan().call.policy != CallingPolicy::MicrosoftX64
        || boundary.plan().call.parameters.len() != 2
        || boundary.plan().call.result.is_some()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry requires two Microsoft-x64 inputs and no result"
                .into(),
        ));
    }

    let method = selected_method(selected)?;
    validate_method(method, &boundary, [image_source, storage_source])?;
    let physical_contract = selected.physical_contract().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry lost its paired physical plan".into(),
        )
    })?;
    let physical_signature = CallSignature {
        parameters: physical_contract
            .boundary_entry_plan()
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: physical_contract
            .boundary_entry_plan()
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let validated_physical = validate_boundary_entry_plan(
        physical_contract.boundary_entry_plan().clone(),
        &physical_signature,
    )
    .map_err(|diagnostic| {
        ProgramStorageEntryDiagnostic(format!(
            "optimized semantic ProgramStorage physical plan is invalid: {diagnostic}"
        ))
    })?;
    if physical_contract.target_slot() != slot
        || physical_contract.requirement_identity() == method.requirement_identity
        || physical_contract.calling_plan_report_fingerprint()
            != validated_physical.contract_report_fingerprint()
        || physical_contract.calling_plan_report_fingerprint()
            == boundary.contract_report_fingerprint()
        || physical_contract
            .boundary_entry_plan()
            .call
            .result
            .is_none()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry conflates its semantic and physical plans"
                .into(),
        ));
    }

    Ok(ValidatedSemanticEntryInputs {
        target,
        slot,
        source: source.clone(),
        boundary,
        method: method.clone(),
        physical_contract: physical_contract.clone(),
    })
}

fn selected_method(
    selected: &SelectedProgramStorageEntryPlan,
) -> Result<&ServiceMethod, ProgramStorageEntryDiagnostic> {
    let matches = selected
        .schema()
        .methods
        .iter()
        .filter(|method| method.requirement_identity == selected.requirement_identity())
        .collect::<Vec<_>>();
    let [method] = matches.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "selected semantic ProgramStorage schema retains {} exact requirement rows",
            matches.len()
        )));
    };
    Ok(method)
}

fn validate_method(
    method: &ServiceMethod,
    boundary: &ValidatedBoundaryEntryPlan,
    source: [&crate::ProgramEntrySourceVisibleParameterSignature; 2],
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if method.requirement_owner != PROGRAM_STORAGE_ENTRY_OWNER
        || method.name != PROGRAM_STORAGE_ENTRY_METHOD
        || method.parameter_count != 2
        || method.parameter_type_identities.len() != 2
        || method.has_result
        || method.result_type_identity.is_some()
        || !method.result_claims.is_empty()
        || method.calling_plan_report_fingerprint != Some(boundary.contract_report_fingerprint())
        || method.calling_plan_commitment.map(|value| value.as_bytes())
            != Some(boundary.contract_commitment_digest())
        || !matches!(
            method.entry_claims.as_slice(),
            [image, storage] if image.parameter_index == 0 && storage.parameter_index == 1
        )
    {
        return Err(ProgramStorageEntryDiagnostic(
            "selected semantic requirement is not exact ProgramStorageEntry::enter(Image, InitialStorage) -> Unit"
                .into(),
        ));
    }
    for (index, parameter) in source.into_iter().enumerate() {
        if method.parameter_type_identities[index] != parameter.normalized_type_identity() {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "selected semantic ProgramStorage parameter {index} drifted from its checked source type"
            )));
        }
        let claims = method
            .entry_claims
            .iter()
            .filter(|claim| claim.parameter_index == index)
            .collect::<Vec<_>>();
        let [claim] = claims.as_slice() else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "selected semantic ProgramStorage parameter {index} retains {} exact root claims",
                claims.len()
            )));
        };
        validate_claim(claim, index)?;
    }
    Ok(())
}

fn validate_claim(
    claim: &ServiceEntryClaim,
    parameter_index: usize,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if claim.parameter_index != parameter_index
        || claim.carrier_identity != EXTENT_CARRIER
        || claim.domain != GRANTED_DOMAIN
        || !claim.predicate_body.is_present()
        || claim.effective_carry != CarryPolicy::STRICT
        || claim.authority_flow != ServiceEntryAuthorityFlow::Accepts
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "selected semantic ProgramStorage parameter {parameter_index} is not exact Extent in Granted"
        )));
    }
    Ok(())
}

fn validate_source_root(
    parameter: &crate::ProgramEntrySourceVisibleParameterSignature,
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let layout = parameter.extent_value_layout();
    let [base, length] = layout.fields();
    if parameter.role() != role
        || parameter.visible_parameter_index() != parameter_index
        || parameter.value_shape() != EXTENT_SHAPE
        || layout.shape() != EXTENT_SHAPE
        || base.role() != ProgramEntrySourceExtentFieldRole::Base
        || base.byte_offset() != 0
        || base.shape() != EXTENT_FIELD_SHAPE
        || length.role() != ProgramEntrySourceExtentFieldRole::Length
        || length.byte_offset() != 8
        || length.shape() != EXTENT_FIELD_SHAPE
        || parameter.is_const()
        || parameter.is_mutable()
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "optimized semantic ProgramStorage {:?} parameter is not the exact by-value Extent declaration",
            role
        )));
    }
    Ok(())
}
