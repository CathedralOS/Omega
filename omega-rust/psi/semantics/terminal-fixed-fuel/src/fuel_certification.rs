//! Derive, replay, and seal fixed-fuel evidence for an invocation.

use crate::{
    FixedEntryFuelCertificate, FixedFuelError, FixedSegmentFuelCertificate,
    ValidatedFixedSafePointFuelSegments,
};
use outcome_bounds::{derive_maximum_entry_bound, used_contract_premises};
use segment_partition::{PreparedFuelModule, PreparedSegments};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use std::collections::BTreeMap;
use terminal_codec::terminal_psi_identity;
use terminal_fuel::TerminalFuelSchedule;
use terminal_verifier::VerifiedTerminalModule;

mod outcome_bounds;
mod segment_partition;
#[cfg(test)]
mod tests;

/// Derive the exact current-slice bound from a verified canonical semantic
/// module. The checker performs no search and depends on no producing compiler
/// state or proof-bundle representation.
pub fn derive_fixed_entry_fuel(
    verified: &VerifiedTerminalModule<'_>,
    entry: MachineId,
) -> Result<FixedEntryFuelCertificate, FixedFuelError> {
    let module = verified.module();
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == entry)
        .ok_or(FixedFuelError::UnknownEntry(entry))?;
    let ceiling_units = derive_maximum_entry_bound(module, machine.id)?;
    Ok(FixedEntryFuelCertificate {
        terminal_psi,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        entry,
        // Control and operation costs are value-independent, so a machine
        // without a contract-tightened component entry rank binds no
        // premise. When `requires` clauses cap the rank arriving at a
        // `Natural` component's first entry — directly, or through a
        // relational chain the clauses themselves state — the visit bound
        // rests on them and the certificate binds the consulted premises.
        relevant_preconditions: used_contract_premises(machine),
        ceiling_units,
    })
}

/// Recompute and compare every public certificate field. Consumers never need
/// to trust a producing compiler's claimed ceiling.
pub fn validate_fixed_entry_fuel(
    verified: &VerifiedTerminalModule<'_>,
    certificate: &FixedEntryFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_entry_fuel(verified, certificate.entry)?;
    if expected != *certificate {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Derive an exact bound for one selected acyclic path segment. The charged
/// endpoint is part of the segment so adjacent certificates neither omit nor
/// double-charge an edge. A conditional edge can be an endpoint, and an
/// interior conditional or case composes the maximum arm: the certificate
/// bounds every walk from `start_block` that commits `end_edge`, while walks
/// leaving through another terminal edge are covered by that edge's own
/// certificate.
pub fn derive_fixed_segment_fuel(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<FixedSegmentFuelCertificate, FixedFuelError> {
    let subject = PreparedFuelModule::new(verified.module());
    subject.identity()?;
    let prepared = PreparedSegments::new(&subject, machine)?;
    prepared.segment_certificate(start_block, end_edge, &mut BTreeMap::new())
}

/// Recompute every public segment field from independently verified terminal
/// semantics.
pub fn validate_fixed_segment_fuel(
    verified: &VerifiedTerminalModule<'_>,
    certificate: &FixedSegmentFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_segment_fuel(
        verified,
        certificate.machine,
        certificate.start_block,
        certificate.end_edge,
    )?;
    if expected != *certificate {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Select the complete current-vocabulary safe-point partition for one
/// machine. Every reachable explicit jump, conditional, return, or local crash
/// edge is a semantic safe point in this slice. Calls compose the callee's
/// normal-return bound into a following caller edge and terminate separately
/// at a callee crash; an all-crash call therefore makes the caller terminator
/// unreachable. A return edge composes the nominal cleanup machines it
/// suspends into, in order, since that work is metered before control leaves
/// the machine. No partial call or suspension state crosses an edge, and the
/// successor block begins the next segment.
/// The returned order is canonical block order followed by terminator edge
/// order, restricted to blocks reachable from the machine entry.
pub fn derive_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
) -> Result<Vec<FixedSegmentFuelCertificate>, FixedFuelError> {
    let subject = PreparedFuelModule::new(verified.module());
    let prepared = PreparedSegments::new(&subject, machine)?;
    prepared.derive_catalog()
}

/// Recompute the whole ordered safe-point partition. Validating certificates
/// one at a time is insufficient because a producer could omit a reachable
/// segment or present a different order.
pub fn validate_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    certificates: &[FixedSegmentFuelCertificate],
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_safe_point_segments(verified, machine)?;
    validate_certificate_sequence(&expected, certificates)
}

/// Validate and retain one complete ordered safe-point partition.
///
/// The supplied rows are compared against a fresh derivation as one sequence,
/// rather than accepted independently. Missing, extra, duplicated, reordered,
/// or semantically stale rows therefore reject before the sealed carrier is
/// created.
pub fn retain_validated_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    certificates: Vec<FixedSegmentFuelCertificate>,
) -> Result<ValidatedFixedSafePointFuelSegments, FixedFuelError> {
    let subject = PreparedFuelModule::new(verified.module());
    let prepared = PreparedSegments::new(&subject, machine)?;
    let expected = prepared.derive_catalog()?;
    validate_certificate_sequence(&expected, &certificates)?;
    Ok(ValidatedFixedSafePointFuelSegments {
        terminal_psi: subject.identity()?,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        machine,
        certificates,
    })
}

/// Derive and seal the complete canonical safe-point partition.
pub fn derive_validated_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
) -> Result<ValidatedFixedSafePointFuelSegments, FixedFuelError> {
    let subject = PreparedFuelModule::new(verified.module());
    let prepared = PreparedSegments::new(&subject, machine)?;
    let certificates = prepared.derive_catalog()?;
    // Replay the complete roster with fresh outcome working state before sealing.
    // Only immutable subject identity and lookups survive from derivation.
    let expected = prepared.derive_catalog()?;
    validate_certificate_sequence(&expected, &certificates)?;
    Ok(ValidatedFixedSafePointFuelSegments {
        terminal_psi: subject.identity()?,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        machine,
        certificates,
    })
}

/// Independently replay a retained catalog against verified terminal Psi.
pub fn validate_retained_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    catalog: &ValidatedFixedSafePointFuelSegments,
) -> Result<(), FixedFuelError> {
    let subject = PreparedFuelModule::new(verified.module());
    if catalog.terminal_psi != subject.identity()?
        || catalog.schedule != TerminalFuelSchedule::CURRENT.identity()
    {
        return Err(FixedFuelError::CertificateMismatch);
    }
    let prepared = PreparedSegments::new(&subject, catalog.machine)?;
    let expected = prepared.derive_catalog()?;
    validate_certificate_sequence(&expected, &catalog.certificates)
}

fn validate_certificate_sequence(
    expected: &[FixedSegmentFuelCertificate],
    certificates: &[FixedSegmentFuelCertificate],
) -> Result<(), FixedFuelError> {
    if expected != certificates {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}
