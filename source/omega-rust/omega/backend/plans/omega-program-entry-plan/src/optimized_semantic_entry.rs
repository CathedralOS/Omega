//! Data-only contract for the clean optimizer's semantic ProgramStorage entry.
//!
//! This boundary joins the checked source declaration, selected target-owned
//! ProgramStorage requirement, and its semantic calling plan. It deliberately
//! owns no machine association, wrapper bytes, runtime root values,
//! physical bootstrap, image, installation, or publication evidence.

use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, ValidatedBoundaryEntryPlan, ValuePlacement,
    ValueShape, validate_boundary_entry_plan,
};
use omega_effects::provider_plan::{ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod};
use psi_language_semantics::CarryPolicy;

use crate::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceReceiverSignature, ProgramEntrySourceResultSignature,
    ProgramEntrySourceSignatureIdentity, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryRootRole, SelectedProgramEntrySourceSignature,
    SelectedProgramStorageEntryPlan,
};

const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";
const EXTENT_CARRIER: &str = "named(name(Extent))";
const GRANTED_DOMAIN: &str = "Extent::Granted";
const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
const EXTENT_FIELD_SHAPE: ValueShape = ValueShape::integer(8, 8);

/// Explicit status of the separately retained physical entry contract.
///
/// The semantic contract requires this plan to remain paired with the selected
/// target slot, but cannot use it to construct or invoke a bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStoragePhysicalEntryDisposition {
    PlannedNotInvokedV1,
}

/// One exact qualified semantic root and its address-free ABI placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticRoot {
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    carrier_identity: String,
    parameter_type_identity: String,
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

    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub fn parameter_type_identity(&self) -> &str {
        &self.parameter_type_identity
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

/// Validated declaration-only contract for a future clean semantic wrapper.
///
/// A higher compiler layer must separately join this contract to the exact
/// Terminal `MachineId` and private object symbol before emitting anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticEntryContract {
    target: omega_target::NativeTarget,
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    requirement_identity: String,
    source_signature: SelectedProgramEntrySourceSignature,
    source_signature_identity: ProgramEntrySourceSignatureIdentity,
    semantic_boundary_entry_plan: BoundaryEntryPlan,
    semantic_calling_plan_report_fingerprint: u64,
    roots: [OptimizedProgramStorageSemanticRoot; 2],
    physical_contract: ProgramEntryPhysicalContractPlan,
    physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition,
}

impl OptimizedProgramStorageSemanticEntryContract {
    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn target_slot(&self) -> omega_target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn source_signature(&self) -> &SelectedProgramEntrySourceSignature {
        &self.source_signature
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySourceSignatureIdentity {
        self.source_signature_identity
    }

    pub const fn semantic_boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.semantic_boundary_entry_plan
    }

    pub const fn semantic_calling_plan_report_fingerprint(&self) -> u64 {
        self.semantic_calling_plan_report_fingerprint
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

/// Bind the clean optimizer's declaration-only semantic ProgramStorage edge.
pub fn bind_optimized_program_storage_semantic_entry_contract(
    target: omega_target::NativeTarget,
    selected: &SelectedProgramStorageEntryPlan,
    source: &SelectedProgramEntrySourceSignature,
    semantic_boundary_entry_plan: &BoundaryEntryPlan,
) -> Result<OptimizedProgramStorageSemanticEntryContract, ProgramStorageEntryDiagnostic> {
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

    let roots = [
        semantic_root(
            image_source,
            &method.entry_claims[0],
            &boundary.plan().call.parameters[0],
        ),
        semantic_root(
            storage_source,
            &method.entry_claims[1],
            &boundary.plan().call.parameters[1],
        ),
    ];
    Ok(OptimizedProgramStorageSemanticEntryContract {
        target,
        target_slot: slot,
        requirement_identity: method.requirement_identity.clone(),
        source_signature: source.clone(),
        source_signature_identity: source.identity(),
        semantic_boundary_entry_plan: boundary.plan().clone(),
        semantic_calling_plan_report_fingerprint: boundary.contract_report_fingerprint(),
        roots,
        physical_contract: physical_contract.clone(),
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
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

fn semantic_root(
    source: &crate::ProgramEntrySourceVisibleParameterSignature,
    claim: &ServiceEntryClaim,
    placement: &ValuePlacement,
) -> OptimizedProgramStorageSemanticRoot {
    OptimizedProgramStorageSemanticRoot {
        role: source.role(),
        parameter_index: source.visible_parameter_index(),
        carrier_identity: claim.carrier_identity.clone(),
        parameter_type_identity: source.normalized_type_identity().to_owned(),
        domain: claim.domain.clone(),
        effective_carry: claim.effective_carry,
        shape: source.value_shape(),
        placement: placement.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, IndirectPointerLocation, ValueLocation,
        evaluate_ordinary_boundary_entry_plan,
    };
    use omega_effects::provider_plan::{ServiceEntryClaim, ServiceSchema};
    use psi_language_semantics::DomainPredicateBody;
    use psi_symbols::SymbolHandle;

    const REQUIREMENT: &str = "ProgramStorageEntry::enter#exact";
    const IMAGE_TYPE: &str = "Extent in Granted#image";
    const STORAGE_TYPE: &str = "Extent in Granted#initial-storage";

    fn semantic_plan(policy: CallingPolicy) -> ValidatedBoundaryEntryPlan {
        evaluate_ordinary_boundary_entry_plan(
            policy,
            &CallSignature {
                parameters: vec![EXTENT_SHAPE, EXTENT_SHAPE],
                result: None,
            },
        )
        .expect("semantic entry plan")
    }

    fn physical_contract(
        slot: omega_target::ProgramEntrySlotDeclaration,
    ) -> ProgramEntryPhysicalContractPlan {
        let pointer = ValueShape::integer(8, 8);
        let physical = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![pointer, pointer],
                result: Some(pointer),
            },
        )
        .expect("physical entry plan");
        ProgramEntryPhysicalContractPlan::new(
            slot,
            "UefiPhysicalEntry::enter#exact".into(),
            omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
            crate::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                b"optimized-semantic-entry-test-package-source",
            ),
            0xfeed,
            vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
            "EfiStatus".into(),
            physical.contract_report_fingerprint(),
            physical.plan().clone(),
        )
        .expect("physical contract")
    }

    fn claim(parameter_index: usize) -> ServiceEntryClaim {
        ServiceEntryClaim {
            parameter_index,
            carrier_identity: EXTENT_CARRIER.into(),
            domain: GRANTED_DOMAIN.into(),
            predicate_body: DomainPredicateBody::Present,
            effective_carry: CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        }
    }

    fn method(boundary: &ValidatedBoundaryEntryPlan) -> ServiceMethod {
        ServiceMethod {
            name: PROGRAM_STORAGE_ENTRY_METHOD.into(),
            requirement_owner: PROGRAM_STORAGE_ENTRY_OWNER.into(),
            requirement_identity: REQUIREMENT.into(),
            parameter_count: 2,
            parameter_type_identities: vec![IMAGE_TYPE.into(), STORAGE_TYPE.into()],
            entry_claims: vec![claim(0), claim(1)],
            calling_plan_report_fingerprint: Some(boundary.contract_report_fingerprint()),
            calling_plan_commitment: Some(
                omega_effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                    boundary.contract_commitment_digest(),
                ),
            ),
            ..Default::default()
        }
    }

    fn selected_with_method(
        method: ServiceMethod,
        with_physical: bool,
    ) -> SelectedProgramStorageEntryPlan {
        let slot = omega_target::TargetProfile::UefiX64.program_entry_slot();
        let selected = SelectedProgramStorageEntryPlan::from_target_slot(
            slot,
            ServiceSchema {
                trait_name: slot.boundary_schema.expect("boundary schema").into(),
                methods: vec![method],
                ..Default::default()
            },
            REQUIREMENT.into(),
        )
        .expect("selected storage entry");
        if with_physical {
            selected
                .with_physical_contract(physical_contract(slot))
                .expect("paired physical contract")
        } else {
            selected
        }
    }

    fn extent_layout(base: u32) -> crate::ProgramEntrySourceExtentValueLayout {
        crate::ProgramEntrySourceExtentValueLayout::from_checked_record(
            SymbolHandle::from_arena_index(base),
            SymbolHandle::from_arena_index(base + 1),
            0,
            EXTENT_FIELD_SHAPE,
            SymbolHandle::from_arena_index(base + 2),
            8,
            EXTENT_FIELD_SHAPE,
            EXTENT_SHAPE,
        )
        .expect("exact Extent layout")
    }

    fn source(
        receiver: ProgramEntrySourceReceiverSignature,
        roles: [ProgramStorageEntryRootRole; 2],
    ) -> SelectedProgramEntrySourceSignature {
        SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            SymbolHandle::from_arena_index(1),
            SymbolHandle::from_arena_index(2),
            "Boot::launch".into(),
            "launch".into(),
            "Boot::launch#exact".into(),
            receiver,
            vec![
                SelectedProgramEntrySourceSignature::visible_parameter(
                    roles[0],
                    0,
                    IMAGE_TYPE.into(),
                    EXTENT_SHAPE,
                    extent_layout(10),
                    false,
                    false,
                ),
                SelectedProgramEntrySourceSignature::visible_parameter(
                    roles[1],
                    1,
                    STORAGE_TYPE.into(),
                    EXTENT_SHAPE,
                    extent_layout(20),
                    false,
                    false,
                ),
            ],
        )
        .expect("checked source signature")
    }

    fn exact_inputs() -> (
        ValidatedBoundaryEntryPlan,
        SelectedProgramStorageEntryPlan,
        SelectedProgramEntrySourceSignature,
    ) {
        let semantic = semantic_plan(CallingPolicy::MicrosoftX64);
        let selected = selected_with_method(method(&semantic), true);
        let source = source(
            ProgramEntrySourceReceiverSignature::Free,
            [
                ProgramStorageEntryRootRole::Image,
                ProgramStorageEntryRootRole::InitialStorage,
            ],
        );
        (semantic, selected, source)
    }

    #[test]
    fn exact_receiver_free_uefi_contract_retains_only_semantic_planning() {
        let (semantic, selected, source) = exact_inputs();
        let contract = bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &selected,
            &source,
            semantic.plan(),
        )
        .expect("exact semantic contract");

        assert_eq!(contract.target(), omega_target::NativeTarget::uefi_x64());
        assert_eq!(contract.target_slot(), selected.target_slot());
        assert_eq!(contract.requirement_identity(), REQUIREMENT);
        assert_eq!(contract.source_signature(), &source);
        assert_eq!(contract.source_signature_identity(), source.identity());
        assert_eq!(
            contract.semantic_calling_plan_report_fingerprint(),
            semantic.contract_report_fingerprint()
        );
        assert_eq!(
            contract.physical_disposition(),
            OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
        );
        assert_eq!(
            contract.physical_contract(),
            selected.physical_contract().expect("physical contract")
        );
        let [image, storage] = contract.roots();
        assert_eq!(image.role(), ProgramStorageEntryRootRole::Image);
        assert_eq!(storage.role(), ProgramStorageEntryRootRole::InitialStorage);
        assert_eq!(image.parameter_index(), 0);
        assert_eq!(storage.parameter_index(), 1);
        assert_eq!(image.carrier_identity(), EXTENT_CARRIER);
        assert_eq!(storage.domain(), GRANTED_DOMAIN);
        assert_eq!(image.effective_carry(), CarryPolicy::STRICT);
        assert_eq!(image.shape(), EXTENT_SHAPE);
        assert_eq!(image.parameter_type_identity(), IMAGE_TYPE);
        assert_eq!(storage.parameter_type_identity(), STORAGE_TYPE);
        assert!(matches!(
            image.placement().locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(
                    omega_calling_conventions::MachineRegister::X86Rcx
                ),
                ..
            }]
        ));
        assert!(matches!(
            storage.placement().locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(
                    omega_calling_conventions::MachineRegister::X86Rdx
                ),
                ..
            }]
        ));
    }

    #[test]
    fn receiver_role_and_semantic_policy_drift_fail_closed() {
        let (semantic, selected, exact_source) = exact_inputs();
        let error = bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::linux_x64(),
            &selected,
            &exact_source,
            semantic.plan(),
        )
        .expect_err("non-UEFI target must reject");
        assert!(error.0.contains("exact UEFI x86-64"), "{error}");

        let receiver = source(
            ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity: "Boot".into(),
            },
            [
                ProgramStorageEntryRootRole::Image,
                ProgramStorageEntryRootRole::InitialStorage,
            ],
        );
        let error = bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &selected,
            &receiver,
            semantic.plan(),
        )
        .expect_err("receiver-bound source must reject");
        assert!(error.0.contains("receiver-free Unit"), "{error}");

        let swapped = source(
            ProgramEntrySourceReceiverSignature::Free,
            [
                ProgramStorageEntryRootRole::InitialStorage,
                ProgramStorageEntryRootRole::Image,
            ],
        );
        let error = bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &selected,
            &swapped,
            semantic.plan(),
        )
        .expect_err("swapped roots must reject");
        assert!(error.0.contains("exact by-value Extent"), "{error}");

        let sysv = semantic_plan(CallingPolicy::SystemVAMD64);
        let source = source(
            ProgramEntrySourceReceiverSignature::Free,
            [
                ProgramStorageEntryRootRole::Image,
                ProgramStorageEntryRootRole::InitialStorage,
            ],
        );
        let error = bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &selected,
            &source,
            sysv.plan(),
        )
        .expect_err("SysV semantic plan must reject");
        assert!(error.0.contains("Microsoft-x64"), "{error}");
    }

    #[test]
    fn exact_granted_claims_and_semantic_fingerprint_are_required() {
        let semantic = semantic_plan(CallingPolicy::MicrosoftX64);
        let source = source(
            ProgramEntrySourceReceiverSignature::Free,
            [
                ProgramStorageEntryRootRole::Image,
                ProgramStorageEntryRootRole::InitialStorage,
            ],
        );
        let mut variants = Vec::new();
        let mut wrong_carrier = method(&semantic);
        wrong_carrier.entry_claims[0].carrier_identity = "ExtentLookalike".into();
        variants.push(wrong_carrier);
        let mut wrong_domain = method(&semantic);
        wrong_domain.entry_claims[0].domain = "Extent::Observed".into();
        variants.push(wrong_domain);
        let mut bodyless = method(&semantic);
        bodyless.entry_claims[0].predicate_body = DomainPredicateBody::Bodyless;
        variants.push(bodyless);
        let mut permissive = method(&semantic);
        permissive.entry_claims[0].effective_carry = CarryPolicy::PERMISSIVE;
        variants.push(permissive);
        let mut reordered = method(&semantic);
        reordered.entry_claims.swap(0, 1);
        variants.push(reordered);
        let mut stale = method(&semantic);
        stale.calling_plan_report_fingerprint = Some(1);
        variants.push(stale);

        for method in variants {
            let selected = selected_with_method(method, true);
            bind_optimized_program_storage_semantic_entry_contract(
                omega_target::NativeTarget::uefi_x64(),
                &selected,
                &source,
                semantic.plan(),
            )
            .expect_err("semantic claim or fingerprint drift must reject");
        }
    }

    #[test]
    fn paired_physical_plan_is_required_but_never_invoked() {
        let semantic = semantic_plan(CallingPolicy::MicrosoftX64);
        let selected = selected_with_method(method(&semantic), false);
        let source = source(
            ProgramEntrySourceReceiverSignature::Free,
            [
                ProgramStorageEntryRootRole::Image,
                ProgramStorageEntryRootRole::InitialStorage,
            ],
        );
        let error = bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &selected,
            &source,
            semantic.plan(),
        )
        .expect_err("missing physical pairing must reject");
        assert!(error.0.contains("paired physical plan"), "{error}");
    }
}
