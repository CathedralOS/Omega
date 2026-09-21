use super::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperRelocationRequirement,
    OptimizedProgramStorageSemanticWrapperStep, plan_optimized_program_storage_semantic_wrapper,
    validate_optimized_program_storage_semantic_wrapper,
};
use crate::{
    OptimizedProgramStorageSemanticCallingApplication, ProgramEntryPhysicalContractPlan,
    ProgramEntrySourceExtentFieldRole, ProgramEntrySourceExtentValueLayout,
    ProgramEntrySourceReceiverSignature, ProgramStorageEntryRootRole,
    SelectedProgramEntrySourceSignature, SelectedProgramStorageEntryPlan,
    bind_optimized_program_storage_semantic_entry_contract,
};
use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValidatedBoundaryEntryPlan, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};

use super::recipe::{
    OptimizedProgramStorageSemanticReceiverLayout, copy, expected_relocation, expected_steps,
};

const RECEIVER_FREE_CALL_STEP_INDEX: usize = 8;
use effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};
use language_semantics::{CarryPolicy, DomainPredicateBody};
use symbols::SymbolHandle;

const REQUIREMENT: &str = "ProgramStorageEntry::enter#recipe";
const EXTENT_CARRIER: &str = "named(name(Extent))";
const GRANTED_DOMAIN: &str = "Extent::Granted";
const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
const WORD_SHAPE: ValueShape = ValueShape::integer(8, 8);

fn extent_layout(base: u32) -> ProgramEntrySourceExtentValueLayout {
    ProgramEntrySourceExtentValueLayout::from_checked_record(
        SymbolHandle::from_arena_index(base),
        SymbolHandle::from_arena_index(base + 1),
        0,
        WORD_SHAPE,
        SymbolHandle::from_arena_index(base + 2),
        8,
        WORD_SHAPE,
        EXTENT_SHAPE,
    )
    .unwrap()
}

fn semantic() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![EXTENT_SHAPE, EXTENT_SHAPE],
            result: None,
        },
    )
    .unwrap()
}

fn contract_with(
    receiver: ProgramEntrySourceReceiverSignature,
) -> OptimizedProgramStorageSemanticEntryContract {
    let slot = target::TargetProfile::UefiX64.program_entry_slot();
    let semantic = semantic();
    // Test-local calling-plan application identity, distinct from the raw
    // validated plan's own report fingerprint and commitment digest.
    let application = OptimizedProgramStorageSemanticCallingApplication::new(
        &semantic,
        0xB99A_CC11_1901_A002,
        effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([0xB2; 32]),
    );
    let claim = |parameter_index| ServiceEntryClaim {
        parameter_index,
        carrier_identity: EXTENT_CARRIER.into(),
        domain: GRANTED_DOMAIN.into(),
        predicate_body: DomainPredicateBody::Present,
        effective_carry: CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    };
    let selected = SelectedProgramStorageEntryPlan::from_target_slot(
        slot,
        ServiceSchema {
            trait_name: slot.boundary_schema.unwrap().into(),
            methods: vec![ServiceMethod {
                name: "enter".into(),
                requirement_owner: "ProgramStorageEntry".into(),
                requirement_identity: REQUIREMENT.into(),
                parameter_count: 2,
                parameter_type_identities: vec!["ImageExtent".into(), "StorageExtent".into()],
                entry_claims: vec![claim(0), claim(1)],
                calling_plan_report_fingerprint: Some(application.report_fingerprint()),
                calling_plan_commitment: Some(application.commitment()),
                ..Default::default()
            }],
            ..Default::default()
        },
        REQUIREMENT.into(),
    )
    .unwrap();
    let pointer = ValueShape::integer(8, 8);
    let physical = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![pointer, pointer],
            result: Some(pointer),
        },
    )
    .unwrap();
    let selected = selected
        .with_physical_contract(
            ProgramEntryPhysicalContractPlan::new(
                slot,
                "UefiPhysicalEntry::enter#recipe".into(),
                target::ProgramEntryPhysicalContractPackage::UefiX64,
                crate::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                    target::ProgramEntryPhysicalContractPackage::UefiX64,
                    b"optimized-semantic-wrapper-test-package-source",
                ),
                1,
                vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
                "EfiStatus".into(),
                physical.contract_report_fingerprint(),
                physical.plan().clone(),
            )
            .unwrap(),
        )
        .unwrap();
    let source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        slot,
        SymbolHandle::from_arena_index(1),
        SymbolHandle::from_arena_index(2),
        "Boot::launch".into(),
        "launch".into(),
        "Boot::launch#recipe".into(),
        receiver,
        vec![
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::Image,
                0,
                "ImageExtent".into(),
                EXTENT_SHAPE,
                extent_layout(10),
                false,
                false,
            ),
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::InitialStorage,
                1,
                "StorageExtent".into(),
                EXTENT_SHAPE,
                extent_layout(20),
                false,
                false,
            ),
        ],
    )
    .unwrap();
    bind_optimized_program_storage_semantic_entry_contract(
        target::NativeTarget::uefi_x64(),
        &selected,
        &source,
        &application,
    )
    .unwrap()
}

fn contract() -> OptimizedProgramStorageSemanticEntryContract {
    contract_with(ProgramEntrySourceReceiverSignature::Free)
}

fn receiver_contract() -> OptimizedProgramStorageSemanticEntryContract {
    contract_with(ProgramEntrySourceReceiverSignature::ProvisionedMutable {
        normalized_type_identity: "ref-mut(named(name(Boot::launch)))".into(),
    })
}

const BOOT_LAYOUT: OptimizedProgramStorageSemanticReceiverLayout =
    OptimizedProgramStorageSemanticReceiverLayout::new(8, 8);

#[test]
fn exact_semantic_wrapper_recipe_is_address_free_and_balanced() {
    let contract = contract();
    let fingerprint = contract.semantic_calling_plan_report_fingerprint();
    let source_identity = contract.source_signature_identity();
    let plan = plan_optimized_program_storage_semantic_wrapper(contract, None).unwrap();

    validate_optimized_program_storage_semantic_wrapper(&plan).unwrap();
    assert_eq!(plan.source_signature_identity(), source_identity);
    assert_eq!(plan.shadow_byte_count(), 32);
    assert_eq!(plan.outgoing_frame_byte_count(), 72);
    assert_eq!(plan.outgoing_release_byte_count(), 72);
    assert_eq!(plan.pre_call_stack_alignment(), 16);
    assert_eq!(plan.receiver(), None);
    assert_eq!(plan.steps(), expected_steps(fingerprint, None).as_slice());
    assert_eq!(plan.relocation(), &expected_relocation(8));
    assert_eq!(plan.relocation().call_step_index(), 8);
    assert_eq!(
        plan.encoding_disposition(),
        OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1
    );
    assert_eq!(plan.relocation().byte_width(), 4);
    assert_eq!(plan.relocation().addend(), 0);
    assert_eq!(
        plan.physical_disposition(),
        OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
    );
}

#[test]
fn step_order_root_register_and_frame_corruption_fail_closed() {
    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract(), None).unwrap();
    plan.steps.swap(2, 4);
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract(), None).unwrap();
    plan.steps[2] = copy(
        ProgramStorageEntryRootRole::Image,
        0,
        ProgramEntrySourceExtentFieldRole::Base,
        MachineRegister::X86Rdx,
        0,
        32,
    );
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract(), None).unwrap();
    plan.outgoing_release_byte_count = 88;
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract(), None).unwrap();
    plan.steps[9] =
        OptimizedProgramStorageSemanticWrapperStep::ReleaseOutgoingStackFrame { byte_count: 56 };
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());
}

#[test]
fn receiver_source_provisions_one_zeroed_frame_slot_and_reorders_arguments() {
    let plan =
        plan_optimized_program_storage_semantic_wrapper(receiver_contract(), Some(BOOT_LAYOUT))
            .unwrap();

    validate_optimized_program_storage_semantic_wrapper(&plan).unwrap();
    let receiver = plan.receiver().unwrap();
    assert_eq!(receiver.byte_count(), 8);
    assert_eq!(receiver.alignment(), 8);
    assert_eq!(receiver.slot_byte_count(), 16);
    assert_eq!(receiver.outgoing_stack_byte_offset(), 64);
    assert_eq!(plan.outgoing_frame_byte_count(), 88);
    assert_eq!(plan.outgoing_release_byte_count(), 88);
    assert_eq!(plan.pre_call_stack_alignment(), 16);
    assert_eq!(plan.steps().len(), 13);
    assert_eq!(plan.relocation().call_step_index(), 10);
    use OptimizedProgramStorageSemanticWrapperStep as Step;
    assert!(matches!(
        plan.steps()[6],
        Step::ProvisionReceiverMutableStorage {
            outgoing_stack_byte_offset: 64,
            slot_byte_count: 16,
        }
    ));
    assert!(matches!(
        plan.steps()[7],
        Step::BindOutgoingReceiverAddress {
            register: MachineRegister::X86Rcx,
            outgoing_stack_byte_offset: 64,
            byte_count: 8,
            alignment: 8,
        }
    ));
    assert!(matches!(
        plan.steps()[8],
        Step::BindOutgoingExtentCopyAddress {
            register: MachineRegister::X86Rdx,
            ..
        }
    ));
    assert!(matches!(
        plan.steps()[9],
        Step::BindOutgoingExtentCopyAddress {
            register: MachineRegister::X86R8,
            ..
        }
    ));
    assert!(matches!(
        plan.steps()[10],
        Step::CallPrivateTerminalContinuation { .. }
    ));

    // A drifted residence, a missing receiver, or a receiver on the wrong
    // signature all fail closed before any bytes exist.
    let mut drift = plan.clone();
    drift.receiver = None;
    assert!(validate_optimized_program_storage_semantic_wrapper(&drift).is_err());
    let mut drift = plan.clone();
    drift.receiver = Some(super::OptimizedProgramStorageSemanticReceiverStorage {
        byte_count: 8,
        alignment: 8,
        slot_byte_count: 16,
        outgoing_stack_byte_offset: 65,
    });
    assert!(validate_optimized_program_storage_semantic_wrapper(&drift).is_err());

    assert!(
        plan_optimized_program_storage_semantic_wrapper(contract(), Some(BOOT_LAYOUT)).is_err()
    );
    assert!(plan_optimized_program_storage_semantic_wrapper(receiver_contract(), None).is_err());
    assert!(
        plan_optimized_program_storage_semantic_wrapper(
            receiver_contract(),
            Some(OptimizedProgramStorageSemanticReceiverLayout::new(0, 8)),
        )
        .is_err()
    );
    assert!(
        plan_optimized_program_storage_semantic_wrapper(
            receiver_contract(),
            Some(OptimizedProgramStorageSemanticReceiverLayout::new(8, 3)),
        )
        .is_err()
    );
}

#[test]
fn private_call_fingerprint_and_relocation_corruption_fail_closed() {
    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract(), None).unwrap();
    plan.steps[RECEIVER_FREE_CALL_STEP_INDEX] =
            OptimizedProgramStorageSemanticWrapperStep::CallPrivateTerminalContinuation {
                calling_policy: CallingPolicy::MicrosoftX64,
                semantic_calling_plan_report_fingerprint: 0,
                disposition: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
            };
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

    for corrupt in [
        |relocation: &mut OptimizedProgramStorageSemanticWrapperRelocationRequirement| {
            relocation.call_step_index = 7;
        },
        |relocation: &mut OptimizedProgramStorageSemanticWrapperRelocationRequirement| {
            relocation.byte_width = 8;
        },
        |relocation: &mut OptimizedProgramStorageSemanticWrapperRelocationRequirement| {
            relocation.addend = -4;
        },
    ] {
        let mut plan = plan_optimized_program_storage_semantic_wrapper(contract(), None).unwrap();
        corrupt(&mut plan.relocation);
        assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());
    }
}
