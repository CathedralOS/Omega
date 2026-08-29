//! Independent object admission for the exact unmetered ranked-`u32` body.

mod contract;
mod fuel;
mod layout;

use omega_machine_code::MachineCodePlan;
use psi_diagnostics::Diagnostic;

use crate::{ObjectArtifact, ObjectError, ObjectFunction};

/// Classify ranked custody once, then join target decoding, semantic replay,
/// and exact fuel reconstruction. This grants object custody only.
pub(super) fn replay_ranked_u32_countdown(plan: &MachineCodePlan) -> Result<(), ObjectError> {
    let mut candidates = plan.functions.iter().filter(|function| {
        function.requires_ranked_countdown_replay()
            || layout::validate_ranked_countdown_layout(plan.target, &function.bytes).is_some()
    });
    let Some(function) = candidates.next() else {
        return Ok(());
    };
    let record = function
        .ranked_u32_countdown
        .as_ref()
        .ok_or(ObjectError::MissingRankedCountdownCustody(function.machine))?;
    if candidates.next().is_some() || plan.functions.len() != 1 || plan.entry != function.machine {
        return Err(ObjectError::InvalidRankedCountdown(function.machine));
    }
    let decoded = layout::validate_ranked_countdown_layout(plan.target, &function.bytes)
        .ok_or(ObjectError::InvalidRankedCountdown(function.machine))?;
    contract::replay_ranked_countdown_contract(plan, function, record)?;
    if !fuel::replay_ranked_countdown_fuel(record, &function.fuel_attribution, decoded) {
        return Err(ObjectError::InvalidRankedCountdown(function.machine));
    }
    Ok(())
}

/// Independently replay the complete ranked carrier from the retained object
/// facts before final-image emission and again when the source-free native
/// artifact rejoins its object and image.
pub(super) fn replay_ranked_u32_countdown_final_image(
    artifact: &ObjectArtifact,
) -> Result<(), Diagnostic> {
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
    if function.ranked_u32_countdown.is_some() {
        return true;
    }
    function.provenance.operations.len() == 4
        && function.provenance.edges.len() == 5
        && fuel.len() == 9
        && fuel.iter().all(|row| row.machine == function.machine)
}
