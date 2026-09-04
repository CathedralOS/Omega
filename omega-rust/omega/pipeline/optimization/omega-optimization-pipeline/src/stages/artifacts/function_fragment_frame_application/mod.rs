//! Optimizer module role: stage group. Apply validated target frame protocol
//! bytes to already replayable ordinary function fragments.
//!
//! Every framed function receives one prologue and one epilogue at each return
//! site. Conditional-branch coordinates and bytes are replayed after insertion
//! with the target-owned encoders.

mod compute;
mod error;
mod identity;
mod model;
mod reflow;
mod validation;
mod validation_branch;

pub use error::FunctionFragmentFrameApplicationError;
pub use identity::function_fragment_frame_application_identity;
pub use model::*;

use crate::{
    StagedOptimizedFunctionFragmentEmission, validate_optimized_function_fragment_emission,
};

pub fn stage_function_fragment_frame_application(
    source: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    let application = {
        let realization = source
            .source()
            .fixed_frame_realization()
            .ok_or(FunctionFragmentFrameApplicationError::SourceKindMismatch)?;
        compute::apply(
            source.fragments(),
            source.manifest().record().identity,
            realization.protocol().plan(),
            source.source().register_environment().physical(),
        )?
    };
    let receipt = model::seal(&application);
    let staged = StagedFunctionFragmentFrameApplication {
        source,
        application,
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
