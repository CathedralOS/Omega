//! Object-artifact construction: the sealed carrier types, the public
//! builder entry points, and the single validating construction pass that
//! replays every retained machine-code record against final bytes before
//! sealing text, data, symbols, and relocations.
//!
//! `carriers.rs` holds the sealed types, `construction.rs` the builders and
//! the validating pass, and `errors.rs` the error; `call_sites`,
//! `private_functions`, `replay` and `stack_demand` serve the pass.

mod call_sites;
mod carriers;
pub(crate) mod construction;
mod errors;
#[cfg(test)]
mod installed_provider_scalar_call_custody_tests;
mod normalized_foreign_calls;
mod private_functions;
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
pub use construction::build_object_artifact;
pub(crate) use errors::ObjectError;
pub(crate) use normalized_foreign_calls::image_foreign_calls_match_object;
pub use normalized_foreign_calls::{
    derive_normalized_foreign_call_custody, durable_home_offsets,
    foreign_invocation_provenance_matches, foreign_result_operand_count, fragment_instruction_span,
    producer_instruction, scalar_call_result_producer, scalar_immediate_span,
    structural_result_capture, transported_span_end, unique_block_instruction,
};
