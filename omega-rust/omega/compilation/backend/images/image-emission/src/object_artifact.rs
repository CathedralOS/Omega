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

pub use carriers::{
    ObjectArtifact, ObjectBoundarySettlement,
    ObjectCodeAttribution, ObjectForeignCall, ObjectFunction, ObjectScalarCallStack, ObjectUnitCallStack,
};
pub(crate) use carriers::{
    find_object_function, ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectCompilerPrivateFunction,
    ObjectPortEffect, ObjectScalarStack,
    ObjectUnitStack};
pub(crate) use construction::text_emission::emit_private_functions;
pub use construction::{
    build_object_artifact,
};
pub(crate) use construction::{
    build_admitted_x86_fma_object_artifact, build_feature_required_x86_fma_object_artifact, build_object_artifact_with_private_functions};
pub(crate) use errors::ObjectError;
pub(crate) use normalized_foreign_calls::image_foreign_calls_match_object;
pub use normalized_foreign_calls::{
    derive_normalized_foreign_call_custody, foreign_invocation_provenance_matches,
    durable_home_offsets,
    scalar_immediate_span,
    transported_span_end,
    producer_instruction, scalar_call_result_producer,
    foreign_result_operand_count, fragment_instruction_span,
    structural_result_capture, unique_block_instruction,
};
pub(crate) use normalized_foreign_calls::{ scalar_observation_producer};
pub(crate) use private_functions::validate_private_functions;
