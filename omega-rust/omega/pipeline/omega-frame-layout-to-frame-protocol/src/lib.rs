#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Packed target frame-protocol encoding.
//!
//! Target-owned encoders turn validated frame geometry into one packed byte
//! arena. Per-function spans point into that arena; no tiny byte vectors are
//! retained. The protocol still excludes the selected return instruction.

mod compute;
mod error;
mod identity;
mod model;
mod validation;

pub use error::*;
pub use identity::target_frame_protocol_encoding_identity;
pub use model::*;
pub use validation::validate_target_frame_protocol_encoding;

use omega_post_allocation_machine_to_frame_layout::{
    ReturnAddressFrameCustody, TargetFrameLayoutIdentity, ValidatedTargetFrameLayout,
};
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

pub fn stage_target_frame_protocol_encoding(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameProtocolEncodingPolicy,
) -> Result<ValidatedTargetFrameProtocolEncoding, TargetFrameProtocolEncodingError> {
    let plan = compute::derive(frame, environment, policy)?;
    validate_target_frame_protocol_encoding(frame, environment, plan)
}
