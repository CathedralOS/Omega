//! Optimizer module role: stage group. Apply validated target frame protocol
//! bytes to already replayable ordinary function fragments.
//!
//! Every framed function receives one prologue and one epilogue at each return
//! site. Conditional-branch coordinates and bytes are replayed after insertion
//! with the target-owned encoders.

mod error;
mod model;
mod validation;

pub use error::FunctionFragmentFrameApplicationError;
pub use model::*;
pub use omega_machine_code::function_fragment_frame_application_identity;

use crate::{
    StagedOptimizedFunctionFragmentEmission, validate_optimized_function_fragment_emission,
};

pub fn stage_function_fragment_frame_application(
    source: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    let application = {
        let protocol = source
            .source()
            .frame_protocol()
            .ok_or(FunctionFragmentFrameApplicationError::SourceKindMismatch)?;
        omega_machine_emission::apply_frame_protocol_to_fragments(
            source.fragments(),
            source.manifest().record().identity,
            protocol.plan(),
            source.source().register_environment().physical(),
        )
        .map_err(FunctionFragmentFrameApplicationError::from)?
    };
    let receipt = model::seal(&application);
    let staged = StagedFunctionFragmentFrameApplication {
        source,
        application: std::sync::Arc::new(application),
        receipt,
    };
    validate_function_fragment_frame_application(&staged)?;
    Ok(staged)
}

pub fn validate_function_fragment_frame_application(
    staged: &StagedFunctionFragmentFrameApplication,
) -> Result<FunctionFragmentFrameApplicationReceipt, FunctionFragmentFrameApplicationError> {
    validation::validate(staged)
}
