//! Optimizer module role: stage group.
//! Exact ranked-countdown contract replay.
//!
//! Admission deliberately proceeds from the carrier envelope through retained
//! proof custody and ranked semantics to target placement and ownership. Each
//! lower rung owns one rejection domain and can be audited independently.

mod calling_convention;
mod carrier;
mod proof_custody;
mod ranked_semantics;
mod structural_frontier;

use omega_machine_code::{
    MachineCodeFunction, MachineCodePlan, RankedU32CountdownMachineCodeRecord,
};

use crate::{ObjectArtifact, ObjectError, ObjectFunction};

pub(super) fn replay_ranked_countdown_contract(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidRankedCountdown(function.machine);
    carrier::validate_machine_code_carrier(plan, function, record).ok_or_else(invalid)?;
    proof_custody::replay_verifier_custody(record).ok_or_else(invalid)?;
    ranked_semantics::machine_code::validate(plan, function, record).ok_or_else(invalid)?;
    calling_convention::replay_calling_and_structural_contract(plan.target, record)
        .ok_or_else(invalid)?;
    structural_frontier::replay_structural_frontier(record).ok_or_else(invalid)?;
    Ok(())
}

pub(super) fn replay_ranked_countdown_object_contract(
    artifact: &ObjectArtifact,
    function: &ObjectFunction,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidRankedCountdown(function.machine);
    carrier::validate_object_carrier(artifact, function, record).ok_or_else(invalid)?;
    proof_custody::replay_verifier_custody(record).ok_or_else(invalid)?;
    ranked_semantics::object::validate(function, record).ok_or_else(invalid)?;
    calling_convention::replay_calling_and_structural_contract(artifact.target(), record)
        .ok_or_else(invalid)?;
    structural_frontier::replay_structural_frontier(record).ok_or_else(invalid)?;
    Ok(())
}
