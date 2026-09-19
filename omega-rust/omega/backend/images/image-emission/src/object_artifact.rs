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
mod construction;
mod errors;
mod normalized_foreign_calls;
mod private_functions;
pub(crate) mod replay;
pub(crate) mod stack_demand;

pub use carriers::{
    ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectCompilerPrivateFunction,
    ObjectDynamicConformanceSlot, ObjectDynamicConformanceTable, ObjectForeignCall,
    ObjectForwardedDynamicDescriptorAdapter, ObjectForwardedDynamicDescriptorSlot,
    ObjectForwardedDynamicDescriptorTable, ObjectFunction, ObjectPortEffect, ObjectScalarCallStack,
    ObjectScalarStack, ObjectUnitCallStack, ObjectUnitStack,
};
pub(crate) use construction::same_dynamic_table_application;
pub use construction::{
    build_admitted_x86_fma_object_artifact, build_feature_required_x86_fma_object_artifact,
    build_object_artifact, build_object_artifact_with_private_functions,
};
pub use errors::ObjectError;
pub use normalized_foreign_calls::derive_normalized_foreign_call_custody;
pub(crate) use normalized_foreign_calls::image_foreign_calls_match_object;
