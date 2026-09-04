use super::*;
use crate::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceReceiverSignature,
    ProgramStorageEntryRootRole,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, IndirectPointerLocation, ValidatedBoundaryEntryPlan,
    ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use omega_effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};
use psi_language_semantics::{CarryPolicy, DomainPredicateBody};

use super::validation::{
    EXTENT_CARRIER, EXTENT_FIELD_SHAPE, EXTENT_SHAPE, GRANTED_DOMAIN, PROGRAM_STORAGE_ENTRY_METHOD,
    PROGRAM_STORAGE_ENTRY_OWNER,
};
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
