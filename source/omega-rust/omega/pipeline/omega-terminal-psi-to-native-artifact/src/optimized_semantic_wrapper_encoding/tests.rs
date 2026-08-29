use super::*;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValidatedBoundaryEntryPlan, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};
use omega_effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};
use omega_isa_x86_64::{
    X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT,
    X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET, X86_64SemanticUnitWrapperEncodingPolicy,
};
use omega_program_entry_plan::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentValueLayout,
    ProgramEntrySourceReceiverSignature, ProgramStorageEntryRootRole,
    SelectedProgramEntrySourceSignature, SelectedProgramStorageEntryPlan,
    bind_optimized_program_storage_semantic_entry_contract,
    plan_optimized_program_storage_semantic_wrapper,
};
use psi_language_semantics::{CarryPolicy, DomainPredicateBody};
use psi_symbols::SymbolHandle;

const REQUIREMENT: &str = "ProgramStorageEntry::enter#encoding";
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

fn wrapper() -> OptimizedProgramStorageSemanticWrapperPlan {
    let slot = omega_target::TargetProfile::UefiX64.program_entry_slot();
    let semantic = semantic();
    let claim = |parameter_index| ServiceEntryClaim {
        parameter_index,
        carrier_identity: "named(name(Extent))".into(),
        domain: "Extent::Granted".into(),
        predicate_body: DomainPredicateBody::Present,
        effective_carry: CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    };
    let storage = SelectedProgramStorageEntryPlan::from_target_slot(
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
                    omega_effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
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
    let storage = storage
        .with_physical_contract(
            ProgramEntryPhysicalContractPlan::new(
                slot,
                "UefiPhysicalEntry::enter#encoding".into(),
                omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                omega_program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                    omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                    b"optimized-semantic-wrapper-encoding-test-package-source",
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
        "Boot::launch#encoding".into(),
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
    let contract = bind_optimized_program_storage_semantic_entry_contract(
        omega_target::NativeTarget::uefi_x64(),
        &storage,
        &source,
        semantic.plan(),
    )
    .unwrap();
    plan_optimized_program_storage_semantic_wrapper(contract).unwrap()
}

#[test]
fn semantic_plan_selects_the_explicit_compact_target_encoding() {
    let staged = select_optimized_program_storage_semantic_wrapper_encoding(wrapper()).unwrap();
    validate_optimized_program_storage_semantic_wrapper_encoding(&staged).unwrap();
    assert_eq!(
        staged.request().policy,
        X86_64SemanticUnitWrapperEncodingPolicy::MicrosoftX64CallerSavedOnlyNoControlStateMutationV1
    );
    assert_eq!(
        staged.template().bytes().len(),
        X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT
    );
    assert_eq!(
        staged.template().relocation().opcode_function_byte_offset,
        X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET
    );
    assert_eq!(
        staged.template().relocation().field_function_byte_offset,
        X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET
    );
    assert_eq!(
        staged
            .template()
            .relocation()
            .next_instruction_function_byte_offset,
        X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET
    );
    assert_ne!(
        u32::from(X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET),
        113
    );
}

#[test]
fn retained_target_request_drift_fails_closed() {
    let mut staged = select_optimized_program_storage_semantic_wrapper_encoding(wrapper()).unwrap();
    staged.request.outgoing_frame_byte_count = 88;
    assert_eq!(
        validate_optimized_program_storage_semantic_wrapper_encoding(&staged),
        Err(OptimizedProgramStorageSemanticWrapperEncodingError::TemplateMismatch)
    );
}
