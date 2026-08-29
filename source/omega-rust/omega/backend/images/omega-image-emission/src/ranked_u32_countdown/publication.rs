//! Final-image replay for retained unmetered ranked object custody.

use psi_diagnostics::Diagnostic;

use crate::{ObjectArtifact, ObjectFunction};

use super::{contract, fuel, layout};

pub(super) fn replay_final_image(artifact: &ObjectArtifact) -> Result<(), Diagnostic> {
    let mut candidates = artifact.functions().iter().filter(|function| {
        function.ranked_u32_countdown.is_some()
            || requires_ranked_countdown_replay(function, artifact.fuel_attribution())
    });
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
    let decoded =
        layout::validate_ranked_countdown_layout(artifact.target(), bytes).ok_or_else(|| {
            Diagnostic::error(format!(
                "ranked-u32 countdown function {} failed final-image target decoding",
                function.machine
            ))
        })?;
    contract::replay_ranked_countdown_object_contract(artifact, function, record)
        .map_err(|error| Diagnostic::error(error.to_string()))?;
    if !fuel::replay_ranked_countdown_object_fuel(
        record,
        artifact.fuel_attribution(),
        function,
        decoded,
    ) {
        return Err(Diagnostic::error(format!(
            "ranked-u32 countdown function {} failed final-image fuel replay",
            function.machine
        )));
    }
    Ok(())
}

fn requires_ranked_countdown_replay(
    function: &ObjectFunction,
    fuel: &[crate::ObjectFuelAttribution],
) -> bool {
    function.ranked_u32_countdown.is_some()
        || (function.provenance.operations.len() == 4
            && function.provenance.edges.len() == 5
            && fuel.len() == 9
            && fuel.iter().all(|row| row.machine == function.machine))
}
