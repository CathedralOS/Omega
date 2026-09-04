//! Optimizer module role: stage group. Apply validated target frame protocol
//! bytes to already replayable ordinary function fragments.
//!
//! V1 deliberately admits only a single selected block with one final return
//! for each framed function. This makes prologue/epilogue placement exact
//! without pretending that branch displacement replay has already been
//! extended across inserted frame bytes.

mod compute;
mod error;
mod identity;
mod model;

pub use error::FunctionFragmentFrameApplicationError;
pub use identity::function_fragment_frame_application_identity;
pub use model::*;

use crate::{
    StagedOptimizedFunctionFragmentEmission, ValidatedTargetFrameLayout,
    ValidatedTargetFrameProtocolEncoding, validate_optimized_function_fragment_emission,
};

pub fn stage_function_fragment_frame_application(
    source: StagedOptimizedFunctionFragmentEmission,
    frame: ValidatedTargetFrameLayout,
    protocol: ValidatedTargetFrameProtocolEncoding,
) -> Result<StagedFunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    validate_roots(&source, &frame, &protocol)?;
    let application = compute::apply(
        source.fragments(),
        source.manifest().record().identity,
        protocol.plan(),
    )?;
    let receipt = model::seal(&application);
    let staged = StagedFunctionFragmentFrameApplication {
        source,
        frame,
        protocol,
        application,
        receipt,
    };
    validate_function_fragment_frame_application(&staged)?;
    Ok(staged)
}

pub fn validate_function_fragment_frame_application(
    staged: &StagedFunctionFragmentFrameApplication,
) -> Result<FunctionFragmentFrameApplicationReceipt, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&staged.source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    validate_roots(&staged.source, &staged.frame, &staged.protocol)?;
    let expected = compute::apply(
        staged.source.fragments(),
        staged.source.manifest().record().identity,
        staged.protocol.plan(),
    )?;
    if staged.application != expected
        || staged.application.fragments.recomputed_identity()
            != staged.application.fragments.identity
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    let receipt = model::seal(&expected);
    if staged.receipt != receipt {
        return Err(FunctionFragmentFrameApplicationError::ReceiptMismatch);
    }
    Ok(receipt)
}

fn validate_roots(
    source: &StagedOptimizedFunctionFragmentEmission,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
) -> Result<(), FunctionFragmentFrameApplicationError> {
    let source_manifest = source.manifest().record();
    if source_manifest.post_allocation_machine != frame.receipt().post_allocation_machine()
        || frame.receipt().identity() != protocol.receipt().frame_layout()
        || source.fragments().target != frame.receipt().target()
        || source.fragments().target != protocol.receipt().target()
    {
        return Err(FunctionFragmentFrameApplicationError::RootMismatch);
    }
    Ok(())
}
