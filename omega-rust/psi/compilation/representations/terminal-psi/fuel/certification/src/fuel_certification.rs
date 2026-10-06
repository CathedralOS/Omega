//! Derive, replay, and seal fixed-fuel evidence for an invocation.

use crate::{
    FixedEntryFuelCertificate, FixedFuelError, FixedSegmentFuelCertificate,
    ValidatedFixedSafePointFuelSegments,
};
use outcome_bounds::{derive_maximum_entry_bound, used_premises};
use segment_partition::{PreparedFuelModule, PreparedSegments};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use std::collections::BTreeMap;
use terminal_codec::terminal_psi_identity;
use terminal_fuel::TerminalFuelSchedule;

mod outcome_bounds;
mod segment_partition;

/// Derive the exact current-slice bound from a verified canonical semantic
/// module. The checker performs no search and depends on no producing compiler
/// state or proof-bundle representation.
pub fn derive_fixed_entry_fuel(
    module: &terminal_psi::TerminalModule,
    entry: MachineId,
) -> Result<FixedEntryFuelCertificate, FixedFuelError> {
    let module = module;
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
        // whose component entry ranks no premise channel tightens binds no
        // premise. When a `requires` clause, a verified header-block
        // invariant, or a reconstructed edge-guard fact caps the rank
        // arriving at a `Natural` component's first entry — directly,
        // through a relational chain the rows themselves state, through an
        // `IntegerMath*` linear bound solved over unbounded integers, or
        // through a conditional row whose disjunctive arms all bound the
        // rank or whose implication premise the ambient rows discharge —
        // the visit bound rests on them and the certificate binds the
        // consulted premises replayed from their own channel.
        relevant_preconditions: used_premises(module, machine),
        ceiling_units,
    })
}

/// Recompute and compare every public certificate field. Consumers never need
/// to trust a producing compiler's claimed ceiling.
pub fn validate_fixed_entry_fuel(
    module: &terminal_psi::TerminalModule,
    certificate: &FixedEntryFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_entry_fuel(module, certificate.entry)?;
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
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<FixedSegmentFuelCertificate, FixedFuelError> {
    let subject = PreparedFuelModule::new(module);
    subject.identity()?;
    let prepared = PreparedSegments::new(&subject, machine)?;
    prepared.segment_certificate(start_block, end_edge, &mut BTreeMap::new())
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
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
) -> Result<Vec<FixedSegmentFuelCertificate>, FixedFuelError> {
    let subject = PreparedFuelModule::new(module);
    let prepared = PreparedSegments::new(&subject, machine)?;
    prepared.derive_catalog()
}


/// Validate and retain one complete ordered safe-point partition.
///
/// The supplied rows are compared against a fresh derivation as one sequence,
/// rather than accepted independently. Missing, extra, duplicated, reordered,
/// or semantically stale rows therefore reject before the sealed carrier is
/// created.
pub fn retain_validated_fixed_safe_point_segments(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    certificates: Vec<FixedSegmentFuelCertificate>,
) -> Result<ValidatedFixedSafePointFuelSegments, FixedFuelError> {
    let subject = PreparedFuelModule::new(module);
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
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
) -> Result<ValidatedFixedSafePointFuelSegments, FixedFuelError> {
    let subject = PreparedFuelModule::new(module);
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


fn validate_certificate_sequence(
    expected: &[FixedSegmentFuelCertificate],
    certificates: &[FixedSegmentFuelCertificate],
) -> Result<(), FixedFuelError> {
    if expected != certificates {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}
