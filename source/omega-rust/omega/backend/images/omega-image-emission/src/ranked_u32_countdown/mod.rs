//! Independent object admission for the exact unmetered ranked-`u32` body.

mod contract;
mod fuel;
mod layout;

use omega_machine_code::MachineCodePlan;
use psi_diagnostics::Diagnostic;

use crate::{ObjectError, ObjectFunction};

/// Classify ranked custody once, then join target decoding, semantic replay,
/// and exact fuel reconstruction. This grants object custody only.
pub(super) fn replay_ranked_u32_countdown(plan: &MachineCodePlan) -> Result<(), ObjectError> {
    let mut candidates = plan
        .functions
        .iter()
        .filter(|function| {
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

/// Final-image replay has not yet learned the ranked carrier. Keep that later
/// authority visibly fenced after object admission succeeds.
pub(super) fn reject_ranked_u32_countdown_final_image(
    functions: &[ObjectFunction],
) -> Result<(), Diagnostic> {
    if let Some(function) = functions
        .iter()
        .find(|function| function.ranked_u32_countdown.is_some())
    {
        return Err(Diagnostic::error(format!(
            "ranked-u32 countdown function {} has object custody but no final-image replay",
            function.machine
        )));
    }
    Ok(())
}
