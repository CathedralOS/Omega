//! Insertion of target frame bytes into current function fragments.
//!
//! [`apply_frame_protocol_to_fragments`] inserts each function's prologue and
//! the epilogue before every retained return and shifts block, row and fixup
//! coordinates (`compute`), re-encodes and widens the branches the insertion
//! moved (`reflow`), and returns only after the independent checker
//! (`validation`, decoding every branch through `validation_branch`) accepts
//! the result. Source-fragment and target-protocol admission belong to the
//! frame-application stage above; successful byte insertion alone is not
//! publication or execution authority.

mod compute;
mod error;
mod reflow;
mod validation;
mod validation_branch;

use crate::frame_protocol::TargetFrameProtocolEncodingPlan;
pub use error::FrameApplicationError;
use machine_code::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentEmissionPlan,
    FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationIdentity,
};
use optimization_core::FunctionFragmentEmissionManifestIdentity;
use register_model::ValidatedPhysicalRegisterModel;

pub fn apply_frame_protocol_to_fragments(
    source: &FunctionFragmentEmissionPlan,
    source_manifest: FunctionFragmentEmissionManifestIdentity,
    protocol: &TargetFrameProtocolEncodingPlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionFragmentFrameApplication, FrameApplicationError> {
    let application = compute::apply(source, source_manifest, protocol, physical)?;
    validate_frame_protocol_application(source, source_manifest, protocol, physical, &application)?;
    Ok(application)
}

pub fn validate_frame_protocol_application(
    source: &FunctionFragmentEmissionPlan,
    source_manifest: FunctionFragmentEmissionManifestIdentity,
    protocol: &TargetFrameProtocolEncodingPlan,
    physical: &ValidatedPhysicalRegisterModel,
    application: &FunctionFragmentFrameApplication,
) -> Result<(), FrameApplicationError> {
    validation::validate(source, source_manifest, protocol, physical, application)
}
