//! Admission checks for the selected semantic entry and its paired physical plan.

use calling_conventions::{CallingPolicy, ValidatedBoundaryEntryPlan, ValueShape};
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
};
use language_semantics::CarryPolicy;

use crate::{
    OptimizedProgramStorageSemanticCallingApplication, ProgramEntryExtentFieldRole,
    ProgramEntryPhysicalContractPlan, ProgramEntryReceiverSignature, ProgramEntryResultSignature,
    ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole, SelectedProgramEntrySignature,
    SelectedProgramStorageEntryPlan,
};

pub(super) const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
pub(super) const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";
pub(super) const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
pub(super) const EXTENT_FIELD_SHAPE: ValueShape = ValueShape::integer(8, 8);

pub(super) struct ValidatedSemanticEntryInputs {
    pub(super) target: target::NativeTarget,
    pub(super) slot: target::ProgramEntrySlotDeclaration,
    pub(super) source: SelectedProgramEntrySignature,
    pub(super) boundary: ValidatedBoundaryEntryPlan,
    pub(super) method: ServiceMethod,
    /// Canonical text the method's interned requirement identity names in
    /// `selected.schema()`; the contract record keeps text, not the id.
    pub(super) requirement_identity: String,
    pub(super) semantic_application_report_fingerprint: u64,
    pub(super) semantic_application_commitment: BoundaryCallingPlanCommitment,
    pub(super) physical_contract: ProgramEntryPhysicalContractPlan,
}

pub(super) fn validate(
    target: target::NativeTarget,
    selected: &SelectedProgramStorageEntryPlan,
    source: &SelectedProgramEntrySignature,
    semantic_application: &OptimizedProgramStorageSemanticCallingApplication<'_>,
) -> Result<ValidatedSemanticEntryInputs, ProgramStorageEntryDiagnostic> {
    let slot = selected.target_slot();
    if target != target::NativeTarget::uefi_x64()
        || slot != target::TargetProfile::UefiX64.program_entry_slot()
        || source.target_slot() != slot
        || slot.schema != target::ProgramEntrySchema::ProgramStorageApplication
        || slot.visible_parameters != target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        || slot.semantic_calling_convention
            != Some(target::ProgramEntryCallingConvention::MicrosoftX64)
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry requires the exact UEFI x86-64 target slot"
                .into(),
        ));
    }
    // The semantic contract carries the selected source shape verbatim: a free
    // Unit source and a source whose receiver the wrapper provisions as fresh
    // mutable storage are both exact. The receiver's identity is part of the
    // checked source signature, so its custody never re-derives from bytes.
    match source.receiver() {
        ProgramEntryReceiverSignature::Free => {}
        ProgramEntryReceiverSignature::ProvisionedMutable {
            normalized_type_identity,
            ..
        } if !normalized_type_identity.is_absent() => {}
        _ => {
            return Err(ProgramStorageEntryDiagnostic(
                "optimized semantic ProgramStorage entry requires an exact receiver signature"
                    .into(),
            ));
        }
    }
    if source.result() != ProgramEntryResultSignature::Unit {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry requires one Unit source".into(),
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

    let boundary = semantic_application.admitted_boundary_entry_plan();
    if boundary.plan().call.policy != CallingPolicy::MicrosoftX64
        || boundary.plan().call.parameters.len() != 2
        || boundary.plan().call.result.is_some()
        || !boundary
            .plan()
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .eq([image_source.value_shape(), storage_source.value_shape()])
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry requires two Microsoft-x64 inputs and no result"
                .into(),
        ));
    }

    let method = selected_method(selected)?;
    validate_method(
        method,
        selected.schema(),
        semantic_application,
        [image_source, storage_source],
        source.identities(),
    )?;
    let physical_contract = selected.physical_contract().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage entry lost its paired physical plan".into(),
        )
    })?;
    let validated_physical = semantic_application.admitted_physical_entry_plan();
    if physical_contract.boundary_entry_plan() != validated_physical.plan()
        || physical_contract.target_slot() != slot
        || physical_contract
            .identities()
            .text(physical_contract.requirement_identity())
            == Some(selected.schema().identity_text(method.requirement_identity))
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
        boundary: boundary.clone(),
        method: method.clone(),
        requirement_identity: selected
            .schema()
            .identity_text(method.requirement_identity)
            .to_owned(),
        semantic_application_report_fingerprint: semantic_application.report_fingerprint(),
        semantic_application_commitment: semantic_application.commitment(),
        physical_contract: physical_contract.clone(),
    })
}

fn selected_method<'a>(
    selected: &'a SelectedProgramStorageEntryPlan,
) -> Result<&'a ServiceMethod, ProgramStorageEntryDiagnostic> {
    let matches = selected
        .schema()
        .methods
        .iter()
        .filter(|method| {
            selected.schema().identities.id_at(
                selected
                    .identities()
                    .text(selected.requirement_identity())
                    .unwrap_or_default(),
            ) == Some(method.requirement_identity)
        })
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
    schema: &effects::provider_plan::ServiceSchema,
    semantic_application: &OptimizedProgramStorageSemanticCallingApplication<'_>,
    source: [&crate::ProgramEntryVisibleParameterSignature; 2],
    plan_identities: &terminal_psi::TerminalIdentityTable,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if method.requirement_owner != PROGRAM_STORAGE_ENTRY_OWNER
        || method.name != PROGRAM_STORAGE_ENTRY_METHOD
        || method.parameter_count != 2
        || method.parameter_type_identities.len() != 2
        || method.has_result
        || method.result_type_identity.is_valid()
        || !method.result_claims.is_empty()
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
    // The schema commits to the complete calling-plan *application* (semantic
    // signature, target, opaque representations, canonical plan), which is a
    // different identity namespace than the raw validated ABI plan. Exact
    // source-application custody therefore compares against the pair the
    // entry settlement admitted for the plan above — never against the plan's own
    // `contract_report_fingerprint`/`contract_commitment_digest`, which could
    // only match a fabricated schema row.
    if method.calling_plan_report_fingerprint != Some(semantic_application.report_fingerprint())
        || method.calling_plan_commitment != Some(semantic_application.commitment())
    {
        return Err(ProgramStorageEntryDiagnostic(
            "selected semantic ProgramStorage requirement does not retain its exact calling-plan application"
                .into(),
        ));
    }
    for (index, parameter) in source.into_iter().enumerate() {
        if Some(
            schema
                .identities
                .text(method.parameter_type_identities[index]),
        ) != plan_identities.text(parameter.normalized_type_identity())
        {
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
        validate_claim(claim, schema, index)?;
    }
    Ok(())
}

fn validate_claim(
    claim: &ServiceEntryClaim,
    schema: &effects::provider_plan::ServiceSchema,
    parameter_index: usize,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    // Provider planning selected the exact core arrival declaration and emitted
    // this claim only for its authorized establishment route. Above, the source
    // parameter and calling application join that same requirement. Consume its
    // normalized identities verbatim: reconstructing a bare `Extent`/`Granted`
    // spelling loses the owner and rejects package-qualified identities.
    if claim.parameter_index != parameter_index
        || schema.identities.text(claim.carrier_identity).is_empty()
        || claim.domain.is_empty()
        || !claim.predicate_body.is_present()
        || claim.effective_carry != CarryPolicy::STRICT
        || claim.authority_flow != ServiceEntryAuthorityFlow::Accepts
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "selected semantic ProgramStorage parameter {parameter_index} is not exact Extent in Granted: carrier={}, domain={}, predicate={:?}, carry={:?}, flow={:?}",
            schema.identities.text(claim.carrier_identity),
            claim.domain,
            claim.predicate_body,
            claim.effective_carry,
            claim.authority_flow
        )));
    }
    Ok(())
}

fn validate_source_root(
    parameter: &crate::ProgramEntryVisibleParameterSignature,
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let layout = parameter.extent_value_layout();
    let [base, length] = layout.fields();
    if parameter.role() != role
        || parameter.visible_parameter_index() != parameter_index
        || parameter.value_shape() != EXTENT_SHAPE
        || layout.shape() != EXTENT_SHAPE
        || base.role() != ProgramEntryExtentFieldRole::Base
        || base.byte_offset() != 0
        || base.shape() != EXTENT_FIELD_SHAPE
        || length.role() != ProgramEntryExtentFieldRole::Length
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
