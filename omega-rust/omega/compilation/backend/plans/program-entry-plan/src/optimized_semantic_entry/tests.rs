use super::{
    OptimizedProgramStoragePhysicalEntryDisposition,
    OptimizedProgramStorageSemanticCallingApplication, SelectedProgramEntrySourceSignature,
    SelectedProgramStorageEntryPlan, bind_optimized_program_storage_semantic_entry_contract,
};
use crate::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceReceiverSignature,
    ProgramStorageEntryRootRole,
};
use calling_conventions::{
    CallSignature, CallingPolicy, IndirectPointerLocation, ValidatedBoundaryEntryPlan,
    ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};
use language_semantics::{CarryPolicy, DomainPredicateBody};

use super::validation::{
    EXTENT_CARRIER, EXTENT_FIELD_SHAPE, EXTENT_SHAPE, GRANTED_DOMAIN, PROGRAM_STORAGE_ENTRY_METHOD,
    PROGRAM_STORAGE_ENTRY_OWNER,
};
use symbols::SymbolHandle;

const REQUIREMENT: &str = "ProgramStorageEntry::enter#exact";
const IMAGE_TYPE: &str = "Extent in Granted#image";
const STORAGE_TYPE: &str = "Extent in Granted#initial-storage";

// Test-local calling-plan application identity. Production derives this pair
// from the materialized source signature through the native binder's replayed
// `BoundaryCallingPlanRealization`; what this leg pins down is that the schema
// commits to the application pair, so it is deliberately distinct from the raw
// validated plan's own report fingerprint and commitment digest.
const APPLICATION_REPORT_FINGERPRINT: u64 = 0xA991_1CA7_10AB_1E00;
const APPLICATION_COMMITMENT_BYTES: [u8; 32] = [0xA9; 32];

fn semantic_application(
    plan: &ValidatedBoundaryEntryPlan,
) -> OptimizedProgramStorageSemanticCallingApplication<'_> {
    OptimizedProgramStorageSemanticCallingApplication::new(
        plan,
        APPLICATION_REPORT_FINGERPRINT,
        effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
            APPLICATION_COMMITMENT_BYTES,
        ),
    )
}

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
    slot: target::ProgramEntrySlotDeclaration,
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
        target::ProgramEntryPhysicalContractPackage::UefiX64,
        crate::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            target::ProgramEntryPhysicalContractPackage::UefiX64,
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

fn method(application: &OptimizedProgramStorageSemanticCallingApplication<'_>) -> ServiceMethod {
    ServiceMethod {
        name: PROGRAM_STORAGE_ENTRY_METHOD.into(),
        requirement_owner: PROGRAM_STORAGE_ENTRY_OWNER.into(),
        requirement_identity: REQUIREMENT.into(),
        parameter_count: 2,
        parameter_type_identities: vec![IMAGE_TYPE.into(), STORAGE_TYPE.into()],
        entry_claims: vec![claim(0), claim(1)],
        calling_plan_report_fingerprint: Some(application.report_fingerprint()),
        calling_plan_commitment: Some(application.commitment()),
        ..Default::default()
    }
}

fn selected_with_method(
    method: ServiceMethod,
    with_physical: bool,
) -> SelectedProgramStorageEntryPlan {
    let slot = target::TargetProfile::UefiX64.program_entry_slot();
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
        target::TargetProfile::UefiX64.program_entry_slot(),
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
    let application = semantic_application(&semantic);
    let selected = selected_with_method(method(&application), true);
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
    let application = semantic_application(&semantic);
    let contract = bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::uefi_x64(),
        &selected,
        &source,
        &application,
    )
    .expect("exact semantic contract");

    assert_eq!(contract.target(), target::NativeTarget::uefi_x64());
    assert_eq!(contract.target_slot(), selected.target_slot());
    assert_eq!(contract.requirement_identity(), REQUIREMENT);
    assert_eq!(contract.source_signature(), &source);
    assert_eq!(contract.source_signature_identity(), source.identity());
    assert_eq!(
        contract.semantic_calling_plan_report_fingerprint(),
        semantic.contract_report_fingerprint()
    );
    assert_eq!(
        contract.semantic_calling_application_report_fingerprint(),
        application.report_fingerprint()
    );
    assert_eq!(
        contract.semantic_calling_application_commitment(),
        application.commitment()
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
                calling_conventions::MachineRegister::X86Rcx
            ),
            ..
        }]
    ));
    assert!(matches!(
        storage.placement().locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(
                calling_conventions::MachineRegister::X86Rdx
            ),
            ..
        }]
    ));
}

#[test]
fn receiver_role_and_semantic_policy_drift_fail_closed() {
    let (semantic, selected, exact_source) = exact_inputs();
    let application = semantic_application(&semantic);
    let error = bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::linux_x64(),
        &selected,
        &exact_source,
        &application,
    )
    .expect_err("non-UEFI target must reject");
    assert!(error.0.contains("exact UEFI x86-64"), "{error}");

    // The canary's `Boot::launch` carries a provisioned mutable receiver: the
    // contract admits the exact selected source shape and the wrapper owns the
    // residence downstream.
    let receiver = source(
        ProgramEntrySourceReceiverSignature::ProvisionedMutable {
            normalized_type_identity: "Boot".into(),
        },
        [
            ProgramStorageEntryRootRole::Image,
            ProgramStorageEntryRootRole::InitialStorage,
        ],
    );
    bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::uefi_x64(),
        &selected,
        &receiver,
        &application,
    )
    .expect("provisioned-mutable receiver source must bind");

    let swapped = source(
        ProgramEntrySourceReceiverSignature::Free,
        [
            ProgramStorageEntryRootRole::InitialStorage,
            ProgramStorageEntryRootRole::Image,
        ],
    );
    let error = bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::uefi_x64(),
        &selected,
        &swapped,
        &application,
    )
    .expect_err("swapped roots must reject");
    assert!(error.0.contains("exact by-value Extent"), "{error}");

    let sysv = semantic_plan(CallingPolicy::SystemVAMD64);
    let sysv_application = semantic_application(&sysv);
    let source = source(
        ProgramEntrySourceReceiverSignature::Free,
        [
            ProgramStorageEntryRootRole::Image,
            ProgramStorageEntryRootRole::InitialStorage,
        ],
    );
    let error = bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::uefi_x64(),
        &selected,
        &source,
        &sysv_application,
    )
    .expect_err("SysV semantic plan must reject");
    assert!(error.0.contains("Microsoft-x64"), "{error}");
}

#[test]
fn exact_granted_claims_and_semantic_fingerprint_are_required() {
    let semantic = semantic_plan(CallingPolicy::MicrosoftX64);
    let application = semantic_application(&semantic);
    let source = source(
        ProgramEntrySourceReceiverSignature::Free,
        [
            ProgramStorageEntryRootRole::Image,
            ProgramStorageEntryRootRole::InitialStorage,
        ],
    );
    let mut variants = Vec::new();
    let mut wrong_carrier = method(&application);
    wrong_carrier.entry_claims[0].carrier_identity = "ExtentLookalike".into();
    variants.push(wrong_carrier);
    let mut wrong_domain = method(&application);
    wrong_domain.entry_claims[0].domain = "Extent::Observed".into();
    variants.push(wrong_domain);
    let mut bodyless = method(&application);
    bodyless.entry_claims[0].predicate_body = DomainPredicateBody::Bodyless;
    variants.push(bodyless);
    let mut permissive = method(&application);
    permissive.entry_claims[0].effective_carry = CarryPolicy::PERMISSIVE;
    variants.push(permissive);
    let mut reordered = method(&application);
    reordered.entry_claims.swap(0, 1);
    variants.push(reordered);
    let mut stale = method(&application);
    stale.calling_plan_report_fingerprint = Some(1);
    variants.push(stale);
    let mut stale_commitment = method(&application);
    stale_commitment.calling_plan_commitment =
        Some(effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([0x42; 32]));
    variants.push(stale_commitment);
    // The schema must commit to the calling-plan *application*, a different
    // identity namespace than the raw validated ABI plan. A schema row carrying
    // the plan's own report fingerprint and commitment digest is not exact
    // source-application custody and must reject.
    let mut raw_plan_namespace = method(&application);
    raw_plan_namespace.calling_plan_report_fingerprint =
        Some(semantic.contract_report_fingerprint());
    raw_plan_namespace.calling_plan_commitment = Some(
        effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
            semantic.contract_commitment_digest(),
        ),
    );
    variants.push(raw_plan_namespace);

    for method in variants {
        let selected = selected_with_method(method, true);
        bind_optimized_program_storage_semantic_entry_contract(
            target::NativeTarget::uefi_x64(),
            &selected,
            &source,
            &application,
        )
        .expect_err("semantic claim or fingerprint drift must reject");
    }
}

#[test]
fn paired_physical_plan_is_required_but_never_invoked() {
    let semantic = semantic_plan(CallingPolicy::MicrosoftX64);
    let application = semantic_application(&semantic);
    let selected = selected_with_method(method(&application), false);
    let source = source(
        ProgramEntrySourceReceiverSignature::Free,
        [
            ProgramStorageEntryRootRole::Image,
            ProgramStorageEntryRootRole::InitialStorage,
        ],
    );
    let error = bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::uefi_x64(),
        &selected,
        &source,
        &application,
    )
    .expect_err("missing physical pairing must reject");
    assert!(error.0.contains("paired physical plan"), "{error}");
}
