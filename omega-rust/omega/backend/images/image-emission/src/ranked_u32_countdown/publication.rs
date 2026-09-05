//! Final-image replay for retained unmetered ranked object custody.

use diagnostics::Diagnostic;

use crate::ObjectArtifact;

use super::{contract, layout};

pub(super) fn replay_final_image(artifact: &ObjectArtifact) -> Result<(), Diagnostic> {
    let mut candidates = artifact
        .functions()
        .iter()
        .filter(|function| function.ranked_u32_countdown.is_some());
    let Some(function) = candidates.next() else {
        return Ok(());
    };
    let record = function.ranked_u32_countdown.as_ref().ok_or_else(|| {
        Diagnostic::error(format!(
            "ranked-u32 countdown function {} lost its object custody",
            function.machine
        ))
    })?;
    if candidates.next().is_some()
        || artifact.functions().len() != 1
        || artifact.entry() != function.machine
    {
        return Err(Diagnostic::error(format!(
            "ranked-u32 countdown function {} has invalid final-image ownership",
            function.machine
        )));
    }
    let bytes = function.bytes(artifact);
    layout::validate_ranked_countdown_layout(artifact.target(), bytes).ok_or_else(|| {
        Diagnostic::error(format!(
            "ranked-u32 countdown function {} failed final-image target decoding",
            function.machine
        ))
    })?;
    contract::replay_ranked_countdown_object_contract(artifact, function, record)
        .map_err(|error| Diagnostic::error(error.to_string()))?;
    Ok(())
}
