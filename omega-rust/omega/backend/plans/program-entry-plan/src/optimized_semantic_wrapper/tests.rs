use super::*;
use crate::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceExtentValueLayout, ProgramEntrySourceReceiverSignature,
    ProgramStorageEntryRootRole, SelectedProgramEntrySourceSignature,
    SelectedProgramStorageEntryPlan, bind_optimized_program_storage_semantic_entry_contract,
};
use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValidatedBoundaryEntryPlan, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};

use super::recipe::{CALL_STEP_INDEX, copy, expected_relocation, expected_steps};
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

fn contract() -> OptimizedProgramStorageSemanticEntryContract {
    let slot = target::TargetProfile::UefiX64.program_entry_slot();
    let semantic = semantic();
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
                calling_plan_report_fingerprint: Some(semantic.contract_report_fingerprint()),
                calling_plan_commitment: Some(
                    effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                        semantic.contract_commitment_digest(),
                    ),
                ),
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
        ProgramEntrySourceReceiverSignature::Free,
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
        semantic.plan(),
    )
    .unwrap()
}

#[test]
fn exact_semantic_wrapper_recipe_is_address_free_and_balanced() {
    let contract = contract();
    let fingerprint = contract.semantic_calling_plan_report_fingerprint();
    let source_identity = contract.source_signature_identity();
    let plan = plan_optimized_program_storage_semantic_wrapper(contract).unwrap();

    validate_optimized_program_storage_semantic_wrapper(&plan).unwrap();
    assert_eq!(plan.source_signature_identity(), source_identity);
    assert_eq!(plan.shadow_byte_count(), 32);
    assert_eq!(plan.outgoing_frame_byte_count(), 72);
    assert_eq!(plan.outgoing_release_byte_count(), 72);
    assert_eq!(plan.pre_call_stack_alignment(), 16);
    assert_eq!(plan.steps(), &expected_steps(fingerprint));
    assert_eq!(plan.relocation(), &expected_relocation());
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
    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
    plan.steps.swap(2, 4);
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
    plan.steps[2] = copy(
        ProgramStorageEntryRootRole::Image,
        0,
        ProgramEntrySourceExtentFieldRole::Base,
        MachineRegister::X86Rdx,
        0,
        32,
    );
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
    plan.outgoing_release_byte_count = 88;
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
    plan.steps[9] =
        OptimizedProgramStorageSemanticWrapperStep::ReleaseOutgoingStackFrame { byte_count: 56 };
    assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());
}

#[test]
fn private_call_fingerprint_and_relocation_corruption_fail_closed() {
    let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
    plan.steps[CALL_STEP_INDEX] =
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
        let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
        corrupt(&mut plan.relocation);
        assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());
    }
}
