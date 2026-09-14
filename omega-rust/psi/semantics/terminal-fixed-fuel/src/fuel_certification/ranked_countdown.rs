//! Separate exact-countdown derivation and analysis-only catalog authority.

use super::outcome_bounds::block_units;
use crate::{
    FixedEntryFuelCertificate, FixedFuelError, RankedCountdownSafePointFuelCertificate,
    ValidatedRankedCountdownSafePointFuelSegments,
};
use semantic_vocabulary::{IntegerValue, MachineId};
use std::collections::BTreeMap;
use terminal_codec::{TerminalPsiIdentity, terminal_psi_identity};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi::{TerminalMachine, Terminator};
use terminal_verifier::VerifiedFixedFuelTerminalModule;

/// Derive the all-input ceiling for the exact ranked unsigned-countdown slice
/// admitted by the fixed-fuel verifier carrier.
///
/// The retained rank upper bound supplies the maximum number of covered
/// backedge traversals. Costs are replayed from the actual preheader, header,
/// decrement, exit, and return blocks under the current fuel schedule:
/// `preheader + (upper_bound - lower_bound) * cycle + exit`.
pub fn derive_ranked_countdown_entry_fuel(
    verified: &VerifiedFixedFuelTerminalModule<'_>,
    entry: MachineId,
) -> Result<FixedEntryFuelCertificate, FixedFuelError> {
    let module = verified.module();
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == entry)
        .ok_or(FixedFuelError::UnknownEntry(entry))?;
    let component = machine
        .ranked_scc
        .as_ref()
        .and_then(|ranked| ranked.as_unsigned_countdown())
        .ok_or(FixedFuelError::NotRankedCountdown(entry))?;
    let [covered] = component.covered_cyclic_edges.as_slice() else {
        return Err(FixedFuelError::NotRankedCountdown(entry));
    };
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let preheader = blocks
        .get(&machine.entry)
        .copied()
        .ok_or(FixedFuelError::UnknownBlock(machine.entry))?;
    let header = blocks
        .get(&component.header)
        .copied()
        .ok_or(FixedFuelError::UnknownBlock(component.header))?;
    let decrement = blocks
        .get(&covered.source)
        .copied()
        .ok_or(FixedFuelError::UnknownBlock(covered.source))?;
    let Terminator::Conditional { when_false, .. } = &header.terminator else {
        return Err(FixedFuelError::NotRankedCountdown(entry));
    };
    let done = blocks
        .get(&when_false.target)
        .copied()
        .ok_or(FixedFuelError::UnknownBlock(when_false.target))?;

    let schedule = TerminalFuelSchedule::CURRENT;
    let preheader_units = block_units(preheader, schedule)?;
    let header_units = block_units(header, schedule)?;
    let cycle_units = header_units
        .checked_add(block_units(decrement, schedule)?)
        .ok_or(FixedFuelError::BoundOverflow)?;
    let exit_units = header_units
        .checked_add(block_units(done, schedule)?)
        .ok_or(FixedFuelError::BoundOverflow)?;
    let maximum_iterations = match (component.lower_bound, component.upper_bound) {
        (IntegerValue::Unsigned(lower), IntegerValue::Unsigned(upper)) => upper
            .checked_sub(lower)
            .ok_or(FixedFuelError::NotRankedCountdown(entry))?,
        _ => return Err(FixedFuelError::NotRankedCountdown(entry)),
    };
    let ceiling_units = u128::from(cycle_units)
        .checked_mul(maximum_iterations)
        .and_then(|units| units.checked_add(u128::from(preheader_units)))
        .and_then(|units| units.checked_add(u128::from(exit_units)))
        .ok_or(FixedFuelError::BoundOverflow)?;
    let ceiling_units = u64::try_from(ceiling_units).map_err(|_| FixedFuelError::BoundOverflow)?;

    Ok(FixedEntryFuelCertificate {
        terminal_psi,
        schedule: schedule.identity(),
        entry,
        relevant_preconditions: Vec::new(),
        ceiling_units,
    })
}

/// Independently replay an exact ranked-countdown certificate from the
/// independently proof-checked fixed-fuel carrier.
pub fn validate_ranked_countdown_entry_fuel(
    verified: &VerifiedFixedFuelTerminalModule<'_>,
    certificate: &FixedEntryFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_ranked_countdown_entry_fuel(verified, certificate.entry)?;
    if expected != *certificate {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Derive the complete canonical per-edge safe-point partition for the exact
/// verified `u32` ranked-countdown slice.
///
/// Every row starts before one ranked block's first operation and ends after
/// one of that block's terminating edges. Conditional arms therefore have
/// separate rows with the same block-local cost, while the covered backedge is
/// an ordinary per-traversal row rather than authority to charge the whole
/// loop. The existing whole-entry derivation is replayed first so wider or
/// otherwise unsupported shapes cannot acquire segment evidence merely because
/// one traversal fits in `u64`.
pub fn derive_ranked_countdown_safe_point_segments(
    verified: &VerifiedFixedFuelTerminalModule<'_>,
    machine: MachineId,
) -> Result<Vec<RankedCountdownSafePointFuelCertificate>, FixedFuelError> {
    let module = verified.module();
    let machine_semantics = exact_ranked_u32_machine(verified, machine)?;
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let schedule = TerminalFuelSchedule::CURRENT;
    let mut certificates = Vec::new();
    for block in &machine_semantics.blocks {
        let ceiling_units = block_units(block, schedule)?;
        for end_edge in block.terminator.edges() {
            certificates.push(RankedCountdownSafePointFuelCertificate {
                terminal_psi,
                schedule: schedule.identity(),
                machine,
                start_block: block.id,
                end_edge,
                relevant_preconditions: Vec::new(),
                ceiling_units,
            });
        }
    }
    Ok(certificates)
}

/// Recompute and compare the complete ranked safe-point partition as one
/// sequence. Missing, extra, duplicated, reordered, or stale rows reject.
pub fn validate_ranked_countdown_safe_point_segments(
    verified: &VerifiedFixedFuelTerminalModule<'_>,
    machine: MachineId,
    certificates: &[RankedCountdownSafePointFuelCertificate],
) -> Result<(), FixedFuelError> {
    let expected = derive_ranked_countdown_safe_point_segments(verified, machine)?;
    validate_ranked_certificate_sequence(&expected, certificates)
}

pub(super) fn validate_ranked_certificate_sequence(
    expected: &[RankedCountdownSafePointFuelCertificate],
    certificates: &[RankedCountdownSafePointFuelCertificate],
) -> Result<(), FixedFuelError> {
    if expected != certificates {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Validate and retain one complete ranked-countdown safe-point partition.
pub fn retain_validated_ranked_countdown_safe_point_segments(
    verified: &VerifiedFixedFuelTerminalModule<'_>,
    machine: MachineId,
    certificates: Vec<RankedCountdownSafePointFuelCertificate>,
) -> Result<ValidatedRankedCountdownSafePointFuelSegments, FixedFuelError> {
    validate_ranked_countdown_safe_point_segments(verified, machine, &certificates)?;
    let terminal_psi =
        terminal_psi_identity(verified.module()).map_err(FixedFuelError::SemanticIdentity)?;
    Ok(ValidatedRankedCountdownSafePointFuelSegments {
        terminal_psi,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        machine,
        certificates,
    })
}

/// Derive and seal the complete ranked-countdown safe-point partition.
pub fn derive_validated_ranked_countdown_safe_point_segments(
    verified: &VerifiedFixedFuelTerminalModule<'_>,
    machine: MachineId,
) -> Result<ValidatedRankedCountdownSafePointFuelSegments, FixedFuelError> {
    let certificates = derive_ranked_countdown_safe_point_segments(verified, machine)?;
    retain_validated_ranked_countdown_safe_point_segments(verified, machine, certificates)
}

/// Independently replay a retained ranked safe-point catalog against the exact
/// fixed-fuel verifier subject.
pub fn validate_retained_ranked_countdown_safe_point_segments(
    verified: &VerifiedFixedFuelTerminalModule<'_>,
    catalog: &ValidatedRankedCountdownSafePointFuelSegments,
) -> Result<(), FixedFuelError> {
    let terminal_psi =
        terminal_psi_identity(verified.module()).map_err(FixedFuelError::SemanticIdentity)?;
    validate_ranked_catalog_header(terminal_psi, catalog)?;
    validate_ranked_countdown_safe_point_segments(verified, catalog.machine, &catalog.certificates)
}

fn exact_ranked_u32_machine<'module>(
    verified: &'module VerifiedFixedFuelTerminalModule<'_>,
    machine: MachineId,
) -> Result<&'module TerminalMachine, FixedFuelError> {
    let machine_semantics = verified
        .module()
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let component = machine_semantics
        .ranked_scc
        .as_ref()
        .and_then(|ranked| ranked.as_unsigned_countdown())
        .ok_or(FixedFuelError::NotRankedCountdown(machine))?;
    derive_ranked_countdown_entry_fuel(verified, machine)?;
    let u32_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
            .expect("the fixed unsigned 32-bit carrier is valid");
    if component.rank_type != u32_type {
        return Err(FixedFuelError::NotRankedCountdown(machine));
    }
    Ok(machine_semantics)
}

pub(super) fn validate_ranked_catalog_header(
    terminal_psi: TerminalPsiIdentity,
    catalog: &ValidatedRankedCountdownSafePointFuelSegments,
) -> Result<(), FixedFuelError> {
    if catalog.terminal_psi != terminal_psi
        || catalog.schedule != TerminalFuelSchedule::CURRENT.identity()
    {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}
