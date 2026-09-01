use omega_target::Architecture;

use super::super::{Aarch64SameViewCopyElisionError, SameViewCopyInputs};

pub(super) fn validate(
    inputs: &SameViewCopyInputs<'_>,
) -> Result<(), Aarch64SameViewCopyElisionError> {
    if inputs.selected.target.architecture != Architecture::Aarch64
        || inputs.source.target.architecture != Architecture::Aarch64
        || inputs.physical.model().architecture != Architecture::Aarch64
    {
        return Err(Aarch64SameViewCopyElisionError::UnsupportedTarget(
            inputs.source.target,
        ));
    }
    if inputs.source.identity != inputs.source_identity
        || inputs.source.selected != inputs.selected_identity
        || inputs.selected.target != inputs.source.target
        || inputs.liveness.selected != inputs.selected_identity
        || inputs.liveness.target != inputs.source.target
        || inputs.source.physical_register_model != inputs.physical.identity()
        || inputs.selected.functions.len() != inputs.source.functions.len()
        || inputs.selected.functions.len() != inputs.liveness.functions.len()
    {
        return Err(Aarch64SameViewCopyElisionError::RootMismatch);
    }
    Ok(())
}
