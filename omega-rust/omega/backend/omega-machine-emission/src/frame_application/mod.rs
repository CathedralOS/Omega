//! Optimizer module role: executable entrance. Apply target frame bytes to fragments.
//!
//! This transformation and its independent checker operate on current data.
//! Source-fragment and target-protocol admission remain the caller's obligations;
//! successful byte projection alone is not publication or execution authority.

mod compute;
mod error;
mod reflow;
mod validation;
mod validation_branch;

use crate::TargetFrameProtocolEncodingPlan;
pub use error::FrameApplicationError;
use omega_machine_code::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentEmissionPlan,
    FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationIdentity,
};
use omega_optimization_core::FunctionFragmentEmissionManifestIdentity;
use omega_register_model::ValidatedPhysicalRegisterModel;

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
