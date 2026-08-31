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

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramStorageEntryDiagnostic,
};

/// Construct the pure semantic wrapper recipe without selecting a Terminal
/// call target or emitting bytes.
pub fn plan_optimized_program_storage_semantic_wrapper(
    source: OptimizedProgramStorageSemanticEntryContract,
) -> Result<OptimizedProgramStorageSemanticWrapperPlan, ProgramStorageEntryDiagnostic> {
    validation::validate_contract_surface(&source)?;
    let source_signature_identity = source.source_signature_identity();
    let fingerprint = source.semantic_calling_plan_report_fingerprint();
    let plan = OptimizedProgramStorageSemanticWrapperPlan {
        source,
        source_signature_identity,
        shadow_byte_count: recipe::SHADOW_BYTE_COUNT,
        outgoing_frame_byte_count: recipe::OUTGOING_FRAME_BYTE_COUNT,
        outgoing_release_byte_count: recipe::OUTGOING_FRAME_BYTE_COUNT,
        pre_call_stack_alignment: recipe::PRE_CALL_STACK_ALIGNMENT,
        steps: recipe::expected_steps(fingerprint),
        relocation: recipe::expected_relocation(),
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
