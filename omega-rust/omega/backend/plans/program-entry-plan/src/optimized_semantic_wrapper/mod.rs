//! Optimizer module role: executable entrance. Address-free semantic ProgramStorage wrapper planning.
//!
//! This entrance coordinates the canonical recipe with an independent replay:
//! construction cannot publish a wrapper plan until the retained source,
//! frame, action sequence, and symbolic relocation validate together.

mod model;
mod recipe;
mod validation;

#[cfg(test)]
mod tests;

pub use model::*;
pub use recipe::OptimizedProgramStorageSemanticReceiverLayout;

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramStorageEntryDiagnostic,
};

/// Construct the pure semantic wrapper recipe without selecting a Terminal
/// call target or emitting bytes. `receiver` supplies the referent layout the
/// emitted semantic child derived from its checked `self` parameter exactly
/// when the contract's source signature provisions a mutable receiver.
pub fn plan_optimized_program_storage_semantic_wrapper(
    source: OptimizedProgramStorageSemanticEntryContract,
    receiver: Option<OptimizedProgramStorageSemanticReceiverLayout>,
) -> Result<OptimizedProgramStorageSemanticWrapperPlan, ProgramStorageEntryDiagnostic> {
    validation::validate_contract_surface(&source)?;
    let receiver = validation::receiver_storage(receiver, &source)?;
    let source_signature_identity = source.source_signature_identity();
    let fingerprint = source.semantic_calling_plan_report_fingerprint();
    let frame_byte_count = receiver.map_or(recipe::OUTGOING_FRAME_BYTE_COUNT, |receiver| {
        receiver.outgoing_stack_byte_offset()
            + receiver.slot_byte_count()
            + recipe::RECEIVER_FRAME_TAIL_PAD
    });
    let steps = recipe::expected_steps(fingerprint, receiver);
    let call_step_index = steps
        .iter()
        .position(|step| {
            matches!(
                step,
                OptimizedProgramStorageSemanticWrapperStep::CallPrivateTerminalContinuation { .. }
            )
        })
        .expect("the canonical recipe always emits one continuation call");
    let plan = OptimizedProgramStorageSemanticWrapperPlan {
        source,
        source_signature_identity,
        shadow_byte_count: recipe::SHADOW_BYTE_COUNT,
        outgoing_frame_byte_count: frame_byte_count,
        outgoing_release_byte_count: frame_byte_count,
        pre_call_stack_alignment: recipe::PRE_CALL_STACK_ALIGNMENT,
        receiver,
        steps,
        relocation: recipe::expected_relocation(call_step_index),
        encoding_disposition:
            OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1,
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
    };
    validate_optimized_program_storage_semantic_wrapper(&plan)?;
    Ok(plan)
}

/// Independently replay the retained contract, frame geometry, action order,
/// call slot, and symbolic relocation requirement.
pub fn validate_optimized_program_storage_semantic_wrapper(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validation::validate(plan)
}
