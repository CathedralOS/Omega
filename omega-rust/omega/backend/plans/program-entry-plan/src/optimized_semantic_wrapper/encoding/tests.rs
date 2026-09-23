//! The encoding reuses the wrapper plan's fixture contract: the receiver-free
//! and provisioned-receiver plans are the two shapes the projection admits.

use super::super::tests::{BOOT_LAYOUT, contract, receiver_contract};
use super::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    select_optimized_program_storage_semantic_wrapper_encoding,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};
use crate::{
    OptimizedProgramStorageSemanticWrapperPlan, plan_optimized_program_storage_semantic_wrapper,
};
use isa_x86_64::{
    X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT,
    X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET, X86_64SemanticUnitWrapperEncodingPolicy,
};

fn wrapper() -> OptimizedProgramStorageSemanticWrapperPlan {
    plan_optimized_program_storage_semantic_wrapper(contract(), None).unwrap()
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

#[test]
fn provisioned_receiver_plan_projects_its_frame_slot_into_the_request() {
    let plan =
        plan_optimized_program_storage_semantic_wrapper(receiver_contract(), Some(BOOT_LAYOUT))
            .unwrap();
    let receiver = plan.receiver().unwrap();
    let staged = select_optimized_program_storage_semantic_wrapper_encoding(plan).unwrap();
    validate_optimized_program_storage_semantic_wrapper_encoding(&staged).unwrap();
    let slot = staged.request().receiver.unwrap();
    assert_eq!(
        slot.outgoing_stack_byte_offset,
        receiver.outgoing_stack_byte_offset()
    );
    assert_eq!(slot.slot_byte_count, receiver.slot_byte_count());
    assert_eq!(slot.byte_count, receiver.byte_count());
    assert_eq!(slot.alignment, receiver.alignment());
}
