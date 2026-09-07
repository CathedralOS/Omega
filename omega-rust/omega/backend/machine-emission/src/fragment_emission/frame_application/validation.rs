//! Replay publication custody, then delegate current frame data to its backend checker.

use super::{
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationReceipt,
    StagedFunctionFragmentFrameApplication,
};
use crate::validate_optimized_function_fragment_emission;

pub(super) fn validate(
    staged: &StagedFunctionFragmentFrameApplication,
) -> Result<FunctionFragmentFrameApplicationReceipt, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&staged.source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    let protocol = staged.source.source().frame_protocol();
    if staged.application.frame_protocol
        != machine_code::target_frame_protocol_encoding_identity(protocol)
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    crate::validate_frame_protocol_application(
        staged.source.fragments(),
        staged.source.manifest().record().identity,
        protocol,
        staged.source.source().register_environment().physical(),
        &staged.application,
    )?;
    let receipt = super::model::seal(&staged.application);
    if staged.receipt != receipt {
        return Err(FunctionFragmentFrameApplicationError::ReceiptMismatch);
    }
    Ok(receipt)
}
