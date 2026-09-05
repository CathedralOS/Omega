use crate::{ValidatedTargetFrameLayout, ValidatedTargetRegisterEnvironment};

use super::{
    TargetFrameProtocolEncodingError, TargetFrameProtocolEncodingPlan,
    ValidatedTargetFrameProtocolEncoding, compute, seal,
};

pub fn validate_target_frame_protocol_encoding(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: TargetFrameProtocolEncodingPlan,
) -> Result<ValidatedTargetFrameProtocolEncoding, TargetFrameProtocolEncodingError> {
    if candidate.frame_layout != frame.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    let replayed = compute::derive(frame, environment, candidate.policy)?;
    if candidate != replayed {
        return Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding);
    }
    let receipt = seal(&candidate);
    Ok(ValidatedTargetFrameProtocolEncoding {
        plan: std::sync::Arc::new(candidate),
        receipt,
    })
}
