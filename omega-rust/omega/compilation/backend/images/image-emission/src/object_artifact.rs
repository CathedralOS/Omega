//! The sealed object-artifact carrier. `function_fragments` is its only
//! builder: it projects admitted current fragments into this carrier once.
//!
//! `carriers.rs` holds the sealed types and `errors.rs` the error;
//! `normalized_foreign_calls`, `replay` and `stack_demand` hold the byte-level
//! helpers that projection and retained-artifact readers share.

mod carriers;
mod errors;
mod normalized_foreign_calls;
pub(crate) mod replay;
pub(crate) mod stack_demand;
#[cfg(test)]
pub(crate) mod test_support;

pub use carriers::{
    ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectForeignCall,
    ObjectFunction, ObjectPrivateFunctionEntry, ObjectScalarCallStack, ObjectUnitCallStack,
    PrivateFunctionObject, PrivateObjectSymbolBinding,
};
pub(crate) use carriers::{
    ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectCompilerPrivateFunction,
    ObjectPortEffect, ObjectScalarStack, ObjectUnitStack, find_object_function,
};
pub(crate) use errors::ObjectError;
pub(crate) use normalized_foreign_calls::image_foreign_calls_match_object;
pub use normalized_foreign_calls::{
    derive_normalized_foreign_call_custody, durable_home_offsets,
    foreign_invocation_provenance_matches, foreign_result_operand_count, fragment_instruction_span,
    producer_instruction, scalar_call_result_producer, scalar_immediate_span,
    structural_result_capture, transported_span_end, unique_block_instruction,
};
