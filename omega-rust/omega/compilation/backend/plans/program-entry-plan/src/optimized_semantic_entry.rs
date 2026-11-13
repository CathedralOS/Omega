/*
We validate and construct the declaration-only `OptimizedProgramStorageSemanticEntryContract`,
checking that the selected UEFI `ProgramStorageEntry::enter` slot, source
`Extent` roots, calling-plan application commitment, and paired physical entry
plan agree before minting the immutable semantic entry contract.
*/

use crate::{
    ProgramEntryExtentFieldRole, ProgramEntryPhysicalContractPlan, ProgramEntryResultSignature,
    ProgramEntrySignatureIdentity, ProgramEntryVisibleParameterSignature,
    ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole, SelectedProgramEntrySignature,
    SelectedProgramStorageEntryPlan,
};
use calling_conventions::{
    BoundaryEntryPlan, CallingPolicy, ValidatedBoundaryEntryPlan, ValuePlacement, ValueShape,
};
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
};
use language_semantics::CarryPolicy;

const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";
const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
const EXTENT_FIELD_SHAPE: ValueShape = ValueShape::integer(8, 8);

pub fn bind_optimized_program_storage_semantic_entry_contract(
    target: target::NativeTarget,
    selected: &SelectedProgramStorageEntryPlan,
    source: &SelectedProgramEntrySignature,
    semantic_application: &OptimizedProgramStorageSemanticCallingApplication<'_>,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<OptimizedProgramStorageSemanticEntryContract, ProgramStorageEntryDiagnostic> {
    let validated = validate(target, selected, source, semantic_application)?;
    construct(validated, selected.schema(), identities)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticCallingApplication<'plan> {
    application: &'plan calling_conventions::BoundaryCallingApplication,
    physical_boundary_entry_plan: &'plan ValidatedBoundaryEntryPlan,
}

impl<'plan> OptimizedProgramStorageSemanticCallingApplication<'plan> {
    pub const fn new(
        application: &'plan calling_conventions::BoundaryCallingApplication,
        physical_boundary_entry_plan: &'plan ValidatedBoundaryEntryPlan,
    ) -> Self {
        Self {
            application,
            physical_boundary_entry_plan,
        }
    }

    pub const fn boundary_entry_plan(&self) -> &'plan BoundaryEntryPlan {
        self.application.plan.plan()
    }

    const fn admitted_boundary_entry_plan(&self) -> &'plan ValidatedBoundaryEntryPlan {
        &self.application.plan
    }

    const fn admitted_physical_entry_plan(&self) -> &'plan ValidatedBoundaryEntryPlan {
        self.physical_boundary_entry_plan
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.application.report_fingerprint
    }

    pub const fn commitment(&self) -> BoundaryCallingPlanCommitment {
        BoundaryCallingPlanCommitment::from_digest(self.application.commitment)
    }

    pub const fn abi(&self) -> &semantic_vocabulary::CallingAbiIdentity {
        &self.application.abi
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStoragePhysicalEntryDisposition {
    PlannedNotInvokedV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticRoot {
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    carrier_identity: terminal_psi::TerminalIdentityIndex,
    parameter_type_identity: terminal_psi::TerminalIdentityIndex,
    domain: String,
    effective_carry: CarryPolicy,
    shape: ValueShape,
    placement: ValuePlacement,
}

impl OptimizedProgramStorageSemanticRoot {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn parameter_index(&self) -> usize {
        self.parameter_index
    }

    pub const fn carrier_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.carrier_identity
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub const fn effective_carry(&self) -> CarryPolicy {
        self.effective_carry
    }

    pub const fn shape(&self) -> ValueShape {
        self.shape
    }

    pub const fn placement(&self) -> &ValuePlacement {
        &self.placement
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticEntryContract {
    target: target::NativeTarget,
    target_slot: target::ProgramEntrySlotDeclaration,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    source_signature: SelectedProgramEntrySignature,
    source_signature_identity: ProgramEntrySignatureIdentity,
    semantic_boundary_entry_plan: BoundaryEntryPlan,
    semantic_calling_plan_report_fingerprint: u64,
    semantic_calling_application_report_fingerprint: u64,
    semantic_calling_application_commitment: BoundaryCallingPlanCommitment,
    roots: [OptimizedProgramStorageSemanticRoot; 2],
    physical_contract: ProgramEntryPhysicalContractPlan,
    physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition,
}

impl OptimizedProgramStorageSemanticEntryContract {
    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn source_signature(&self) -> &SelectedProgramEntrySignature {
        &self.source_signature
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySignatureIdentity {
        self.source_signature_identity
    }

    pub const fn semantic_boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.semantic_boundary_entry_plan
    }

    pub const fn semantic_calling_plan_report_fingerprint(&self) -> u64 {
        self.semantic_calling_plan_report_fingerprint
    }

    pub const fn semantic_calling_application_report_fingerprint(&self) -> u64 {
        self.semantic_calling_application_report_fingerprint
    }

    pub const fn semantic_calling_application_commitment(&self) -> BoundaryCallingPlanCommitment {
        self.semantic_calling_application_commitment
    }

    pub const fn roots(&self) -> &[OptimizedProgramStorageSemanticRoot; 2] {
        &self.roots
    }

    pub const fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.physical_contract
    }

    pub const fn physical_disposition(&self) -> OptimizedProgramStoragePhysicalEntryDisposition {
        self.physical_disposition
    }
}

struct ValidatedSemanticEntryInputs {
    target: target::NativeTarget,
    slot: target::ProgramEntrySlotDeclaration,
    source: SelectedProgramEntrySignature,
    boundary: ValidatedBoundaryEntryPlan,
    method: ServiceMethod,
    pub(super) requirement_identity: String,
    semantic_application_report_fingerprint: u64,
    semantic_application_commitment: BoundaryCallingPlanCommitment,
    physical_contract: ProgramEntryPhysicalContractPlan,
}

fn validate(
    target: target::NativeTarget,
    selected: &SelectedProgramStorageEntryPlan,
    source: &SelectedProgramEntrySignature,
    semantic_application: &OptimizedProgramStorageSemanticCallingApplication<'_>,
) -> Result<ValidatedSemanticEntryInputs, ProgramStorageEntryDiagnostic> {
    let slot = selected.target_slot();
    if slot != target::TargetProfile::UefiX64.program_entry_slot()
        || target != slot.owner.native_target()
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

fn selected_method(
    selected: &SelectedProgramStorageEntryPlan,
) -> Result<&ServiceMethod, ProgramStorageEntryDiagnostic> {
    let matches = selected
        .schema()
        .declaration
        .methods
        .iter()
        .filter(|method| {
            selected.schema().declaration.identities.id_at(
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
    source: [&ProgramEntryVisibleParameterSignature; 2],
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
    if semantic_application.abi().requirement_identity
        != schema.identity_text(method.requirement_identity)
        || schema
            .calling_application(semantic_application.abi())
            .map(|plan| plan.report_fingerprint)
            != Some(semantic_application.report_fingerprint())
        || schema
            .calling_application(semantic_application.abi())
            .map(|plan| plan.commitment)
            != Some(semantic_application.commitment())
    {
        return Err(ProgramStorageEntryDiagnostic(
            "selected semantic ProgramStorage requirement does not retain its exact calling-plan application"
                .into(),
        ));
    }
    for (index, parameter) in source.into_iter().enumerate() {
        if Some(
            schema
                .declaration
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
    if claim.parameter_index != parameter_index
        || schema
            .declaration
            .identities
            .text(claim.carrier_identity)
            .is_empty()
        || claim.domain.is_empty()
        || !claim.predicate_presence.is_present()
        || claim.effective_carry != CarryPolicy::STRICT
        || claim.authority_flow != ServiceEntryAuthorityFlow::Accepts
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "selected semantic ProgramStorage parameter {parameter_index} is not exact Extent in Granted: carrier={}, domain={}, predicate={:?}, carry={:?}, flow={:?}",
            schema.declaration.identities.text(claim.carrier_identity),
            claim.domain,
            claim.predicate_presence,
            claim.effective_carry,
            claim.authority_flow
        )));
    }
    Ok(())
}

fn validate_source_root(
    parameter: &ProgramEntryVisibleParameterSignature,
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

fn construct(
    validated: ValidatedSemanticEntryInputs,
    schema: &effects::provider_plan::ServiceSchema,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<OptimizedProgramStorageSemanticEntryContract, ProgramStorageEntryDiagnostic> {
    let ValidatedSemanticEntryInputs {
        target,
        slot,
        source,
        boundary,
        method,
        requirement_identity,
        semantic_application_report_fingerprint,
        semantic_application_commitment,
        physical_contract,
    } = validated;
    let [image_source, storage_source] = source.visible_parameters() else {
        unreachable!("semantic entry admission retained exactly two visible roots")
    };
    let roots = [
        semantic_root(
            image_source,
            &method.entry_claims[0],
            &boundary.plan().call.parameters[0],
            source.identities(),
            &schema.declaration.identities,
            identities,
        )?,
        semantic_root(
            storage_source,
            &method.entry_claims[1],
            &boundary.plan().call.parameters[1],
            source.identities(),
            &schema.declaration.identities,
            identities,
        )?,
    ];

    Ok(OptimizedProgramStorageSemanticEntryContract {
        target,
        target_slot: slot,
        requirement_identity: identities
            .index_of(&requirement_identity)
            .ok_or_else(|| missing_identity("requirement"))?,
        source_signature_identity: source.identity(),
        source_signature: source,
        semantic_boundary_entry_plan: boundary.plan().clone(),
        semantic_calling_plan_report_fingerprint: boundary.contract_report_fingerprint(),
        semantic_calling_application_report_fingerprint: semantic_application_report_fingerprint,
        semantic_calling_application_commitment: semantic_application_commitment,
        roots,
        physical_contract,
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
    })
}

fn semantic_root(
    source: &ProgramEntryVisibleParameterSignature,
    claim: &ServiceEntryClaim,
    placement: &ValuePlacement,
    plan_identities: &terminal_psi::TerminalIdentityTable,
    claim_identities: &effects::provider_plan::ProviderIdentityTable,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<OptimizedProgramStorageSemanticRoot, ProgramStorageEntryDiagnostic> {
    Ok(OptimizedProgramStorageSemanticRoot {
        role: source.role(),
        parameter_index: source.visible_parameter_index(),
        carrier_identity: identities
            .index_of(claim_identities.text(claim.carrier_identity))
            .ok_or_else(|| missing_identity("root carrier"))?,
        parameter_type_identity: plan_identities
            .text(source.normalized_type_identity())
            .and_then(|text| identities.index_of(text))
            .ok_or_else(|| missing_identity("root parameter"))?,
        domain: claim.domain.clone(),
        effective_carry: claim.effective_carry,
        shape: source.value_shape(),
        placement: placement.clone(),
    })
}

fn missing_identity(role: &str) -> ProgramStorageEntryDiagnostic {
    ProgramStorageEntryDiagnostic(format!(
        "selected semantic ProgramStorage {role} identity is missing from Terminal Psi"
    ))
}
