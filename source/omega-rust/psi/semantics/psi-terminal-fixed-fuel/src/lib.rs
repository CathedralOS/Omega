#![forbid(unsafe_code)]

//! Recomputable restricted fixed-fuel certificates for terminal Psi.
//!
//! Ordinary terminal verification accepts acyclic control flow, so the checker
//! derives an exact maximum entry-to-terminal-exit cost without precondition
//! assumptions and partitions the complete reachable graph at every reachable
//! explicit edge. A separate fixed-fuel carrier admits the first exact
//! ranked unsigned countdown and derives its whole-entry ceiling without
//! widening the acyclic segment or native-lowering authorities.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{BlockId, EdgeId, IntegerValue, MachineId, OperationId, Proposition};
use psi_terminal::{
    OperationKind, TerminalAffineCleanupAction, TerminalMachine, TerminalModule, Terminator,
};
use psi_terminal_codec::{CodecError, TerminalPsiIdentity, terminal_psi_identity};
use psi_terminal_fuel::{FuelScheduleIdentity, TerminalFuelSchedule};
use psi_terminal_verifier::{VerifiedFixedFuelTerminalModule, VerifiedTerminalModule};

/// Exact restricted theorem: every path from one machine entry reaches a
/// return or crash within the published current logical-fuel ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedEntryFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    entry: MachineId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

/// Exact current-vocabulary theorem for one selected machine-local path
/// segment. The segment begins before the first operation in `start_block` and
/// includes the charged `end_edge`; its endpoint may be either a jump or a
/// return. A later safe-point classifier can select eligible endpoints without
/// changing this recomputable accounting primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedSegmentFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

/// One block-local safe-point row for the exact verified `u32` ranked
/// countdown.
///
/// This type is intentionally incompatible with [`FixedSegmentFuelCertificate`]
/// so an analysis-only ranked row cannot enter an existing acyclic installation
/// binding API. Construction remains private and complete-catalog validation is
/// required before a sealed ranked catalog can exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedCountdownSafePointFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

/// Complete canonical safe-point partition for one verified terminal machine.
///
/// This carrier is deliberately non-clonable and has no public-field
/// constructor. It preserves the ordered segment certificates as semantic
/// evidence only: neither the catalog nor any individual row is a whole-entry
/// fixed-fuel theorem or authority to replace exact per-site charging.
#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedFixedSafePointFuelSegments {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    machine: MachineId,
    certificates: Vec<FixedSegmentFuelCertificate>,
}

/// Complete canonical safe-point partition for the exact verified `u32`
/// ranked countdown.
///
/// This carrier is deliberately distinct from the acyclic segment catalog and
/// from the whole-entry ranked certificate. It is non-clonable and has no
/// public-field constructor. Its rows describe one traversal from the start of
/// each ranked block through one exact terminating edge; they grant no
/// execution, native-lowering, installation, composition, or bulk-charge
/// authority.
#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedRankedCountdownSafePointFuelSegments {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    machine: MachineId,
    certificates: Vec<RankedCountdownSafePointFuelCertificate>,
}

impl ValidatedRankedCountdownSafePointFuelSegments {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn certificates(&self) -> &[RankedCountdownSafePointFuelCertificate] {
        &self.certificates
    }
}

impl ValidatedFixedSafePointFuelSegments {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn certificates(&self) -> &[FixedSegmentFuelCertificate] {
        &self.certificates
    }
}

impl FixedSegmentFuelCertificate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn start_block(&self) -> BlockId {
        self.start_block
    }

    pub const fn end_edge(&self) -> EdgeId {
        self.end_edge
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

impl RankedCountdownSafePointFuelCertificate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn start_block(&self) -> BlockId {
        self.start_block
    }

    pub const fn end_edge(&self) -> EdgeId {
        self.end_edge
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

impl FixedEntryFuelCertificate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

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
        // Current control and operation costs do not depend on values. The
        // theorem therefore holds for every invocation admitted by the
        // machine contract and needs no additional premise subset.
        relevant_preconditions: Vec::new(),
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

fn validate_ranked_certificate_sequence(
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
        .ok_or(FixedFuelError::NotRankedCountdown(machine))?;
    derive_ranked_countdown_entry_fuel(verified, machine)?;
    let u32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32)
        .expect("the fixed unsigned 32-bit carrier is valid");
    if component.rank_type != u32_type {
        return Err(FixedFuelError::NotRankedCountdown(machine));
    }
    Ok(machine_semantics)
}

fn validate_ranked_catalog_header(
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

/// Derive an exact bound for one selected acyclic path segment. The charged
/// endpoint is part of the segment so adjacent certificates neither omit nor
/// double-charge an edge. A conditional edge can be an endpoint; crossing an
/// unresolved conditional without selecting its successor fails closed.
pub fn derive_fixed_segment_fuel(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<FixedSegmentFuelCertificate, FixedFuelError> {
    let module = verified.module();
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let machine_semantics = module
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let ceiling_units = derive_segment_bound(module, machine_semantics, start_block, end_edge)?;
    Ok(FixedSegmentFuelCertificate {
        terminal_psi,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        machine,
        start_block,
        end_edge,
        // Current operation and edge costs are value-independent.
        relevant_preconditions: Vec::new(),
        ceiling_units,
    })
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
/// unreachable. No partial call or suspension state crosses an edge, and the
/// successor block begins the next segment.
/// The returned order is canonical block order followed by terminator edge
/// order, restricted to blocks reachable from the machine entry.
pub fn derive_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
) -> Result<Vec<FixedSegmentFuelCertificate>, FixedFuelError> {
    let machine_semantics = verified
        .module()
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let blocks = machine_semantics
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let machines = verified
        .module()
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let dynamic_call_targets = dynamic_call_targets(verified.module());
    let schedule = TerminalFuelSchedule::CURRENT;
    let mut memoized_machines = BTreeMap::new();
    let mut active_machines = BTreeSet::from([machine]);
    let mut reachable = BTreeSet::new();
    let mut reachable_terminators = BTreeSet::new();
    let mut pending = vec![machine_semantics.entry];
    while let Some(current) = pending.pop() {
        if !reachable.insert(current) {
            continue;
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        let mut terminator_reachable = true;
        for operation in &block.operations {
            if let Some(callee) = operation_callee(machine, operation, &dynamic_call_targets) {
                let callee_bounds = maximum_machine_outcomes(
                    callee,
                    &machines,
                    &dynamic_call_targets,
                    schedule,
                    &mut memoized_machines,
                    &mut active_machines,
                )?;
                if callee_bounds.returned.is_none() {
                    terminator_reachable = false;
                    break;
                }
            }
        }
        if !terminator_reachable {
            continue;
        }
        reachable_terminators.insert(current);
        match &block.terminator {
            Terminator::Jump { target, .. } => pending.push(*target),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                pending.push(when_false.target);
                pending.push(when_true.target);
            }
            Terminator::StructuralCase { cases, .. } => {
                pending.extend(cases.iter().map(|case| case.target));
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => {}
        }
    }

    let mut segments = Vec::new();
    for block in &machine_semantics.blocks {
        if !reachable_terminators.contains(&block.id) {
            continue;
        }
        for edge in block.terminator.edges() {
            segments.push(derive_fixed_segment_fuel(
                verified, machine, block.id, edge,
            )?);
        }
    }
    Ok(segments)
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
    if expected != certificates {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
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
    validate_fixed_safe_point_segments(verified, machine, &certificates)?;
    let terminal_psi =
        terminal_psi_identity(verified.module()).map_err(FixedFuelError::SemanticIdentity)?;
    Ok(ValidatedFixedSafePointFuelSegments {
        terminal_psi,
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
    let certificates = derive_fixed_safe_point_segments(verified, machine)?;
    retain_validated_fixed_safe_point_segments(verified, machine, certificates)
}

/// Independently replay a retained catalog against verified terminal Psi.
pub fn validate_retained_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    catalog: &ValidatedFixedSafePointFuelSegments,
) -> Result<(), FixedFuelError> {
    let terminal_psi =
        terminal_psi_identity(verified.module()).map_err(FixedFuelError::SemanticIdentity)?;
    if catalog.terminal_psi != terminal_psi
        || catalog.schedule != TerminalFuelSchedule::CURRENT.identity()
    {
        return Err(FixedFuelError::CertificateMismatch);
    }
    validate_fixed_safe_point_segments(verified, catalog.machine, &catalog.certificates)
}

fn derive_maximum_entry_bound(
    module: &TerminalModule,
    machine: MachineId,
) -> Result<u64, FixedFuelError> {
    let schedule = TerminalFuelSchedule::CURRENT;
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let dynamic_call_targets = dynamic_call_targets(module);
    maximum_machine_outcomes(
        machine,
        &machines,
        &dynamic_call_targets,
        schedule,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?
    .maximum()
    .ok_or(FixedFuelError::NoTerminalPath(machine))
}

fn dynamic_call_targets(module: &TerminalModule) -> BTreeMap<(MachineId, OperationId), MachineId> {
    module
        .dynamic_dispatch
        .indirect_dispatches
        .iter()
        .map(|dispatch| ((dispatch.owner, dispatch.operation), dispatch.realization))
        .collect()
}

fn operation_callee(
    owner: MachineId,
    operation: &psi_terminal::Operation,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
) -> Option<MachineId> {
    match &operation.kind {
        OperationKind::Call { callee, .. }
        | OperationKind::CallUnit { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallStructural { callee, .. } => Some(*callee),
        OperationKind::CallDynamicScalar { .. } => {
            dynamic_call_targets.get(&(owner, operation.id)).copied()
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct OutcomeBounds {
    returned: Option<u64>,
    crashed: Option<u64>,
}

impl OutcomeBounds {
    fn maximum(self) -> Option<u64> {
        match (self.returned, self.crashed) {
            (Some(returned), Some(crashed)) => Some(returned.max(crashed)),
            (Some(units), None) | (None, Some(units)) => Some(units),
            (None, None) => None,
        }
    }

    fn with_prefix(self, prefix: u64) -> Result<Self, FixedFuelError> {
        Ok(Self {
            returned: checked_optional_add(self.returned, prefix)?,
            crashed: checked_optional_add(self.crashed, prefix)?,
        })
    }

    fn merge(self, other: Self) -> Self {
        Self {
            returned: maximum_optional(self.returned, other.returned),
            crashed: maximum_optional(self.crashed, other.crashed),
        }
    }
}

fn maximum_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn checked_optional_add(value: Option<u64>, added: u64) -> Result<Option<u64>, FixedFuelError> {
    value
        .map(|value| {
            value
                .checked_add(added)
                .ok_or(FixedFuelError::BoundOverflow)
        })
        .transpose()
}

fn block_units(
    block: &psi_terminal::Block,
    schedule: TerminalFuelSchedule,
) -> Result<u64, FixedFuelError> {
    block
        .operations
        .iter()
        .try_fold(0_u64, |units, operation| {
            units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)
        })?
        .checked_add(schedule.terminator_units(&block.terminator))
        .ok_or(FixedFuelError::BoundOverflow)
}

fn maximum_machine_outcomes(
    machine: MachineId,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<OutcomeBounds, FixedFuelError> {
    if let Some(bounds) = memoized_machines.get(&machine) {
        return Ok(*bounds);
    }
    if !active_machines.insert(machine) {
        return Err(FixedFuelError::CallCycle(machine));
    }
    let machine_semantics = machines
        .get(&machine)
        .copied()
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let blocks = machine_semantics
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    outcome_bounds_from(
        machine,
        machine_semantics.entry,
        &blocks,
        machines,
        dynamic_call_targets,
        schedule,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
        memoized_machines,
        active_machines,
    )
    .inspect(|bounds| {
        active_machines.remove(&machine);
        memoized_machines.insert(machine, *bounds);
    })
}

fn outcome_bounds_from(
    machine: MachineId,
    current: BlockId,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    schedule: TerminalFuelSchedule,
    memoized: &mut BTreeMap<BlockId, OutcomeBounds>,
    active: &mut BTreeSet<BlockId>,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<OutcomeBounds, FixedFuelError> {
    if let Some(bounds) = memoized.get(&current) {
        return Ok(*bounds);
    }
    if !active.insert(current) {
        return Err(FixedFuelError::ControlCycle(current));
    }
    let block = blocks
        .get(&current)
        .copied()
        .ok_or(FixedFuelError::UnknownBlock(current))?;
    let mut normal_units = Some(0_u64);
    let mut crash_units = None;
    for operation in &block.operations {
        normal_units =
            checked_optional_add(normal_units, schedule.operation_units(&operation.kind))?;
        if let Some(callee) = operation_callee(machine, operation, dynamic_call_targets) {
            let callee_bounds = maximum_machine_outcomes(
                callee,
                machines,
                dynamic_call_targets,
                schedule,
                memoized_machines,
                active_machines,
            )?;
            if let Some(prefix) = normal_units {
                crash_units = maximum_optional(
                    crash_units,
                    checked_optional_add(callee_bounds.crashed, prefix)?,
                );
                normal_units = checked_optional_add(callee_bounds.returned, prefix)?;
            }
        }
    }
    let terminator_units = schedule.terminator_units(&block.terminator);
    let continued = match (&block.terminator, normal_units) {
        (_, None) => OutcomeBounds::default(),
        (
            Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnStructural { .. },
            Some(prefix),
        ) => OutcomeBounds {
            returned: Some(
                prefix
                    .checked_add(terminator_units)
                    .ok_or(FixedFuelError::BoundOverflow)?,
            ),
            crashed: None,
        },
        (
            Terminator::Return {
                cleanup_actions, ..
            },
            Some(prefix),
        ) => compose_cleanup_outcomes(
            cleanup_actions.iter().filter_map(|action| match action {
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    Some(cleanup.cleanup_machine)
                }
                TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::DiscardResidual(_) => None,
            }),
            OutcomeBounds {
                returned: Some(
                    prefix
                        .checked_add(terminator_units)
                        .ok_or(FixedFuelError::BoundOverflow)?,
                ),
                crashed: None,
            },
            machines,
            dynamic_call_targets,
            schedule,
            memoized_machines,
            active_machines,
        )?,
        (Terminator::ReturnUnitNominalAffine { cleanups, .. }, Some(prefix)) => {
            compose_cleanup_outcomes(
                cleanups.iter().map(|cleanup| cleanup.cleanup_machine),
                OutcomeBounds {
                    returned: Some(
                        prefix
                            .checked_add(terminator_units)
                            .ok_or(FixedFuelError::BoundOverflow)?,
                    ),
                    crashed: None,
                },
                machines,
                dynamic_call_targets,
                schedule,
                memoized_machines,
                active_machines,
            )?
        }
        (Terminator::Crash { .. }, Some(prefix)) => OutcomeBounds {
            returned: None,
            crashed: Some(
                prefix
                    .checked_add(terminator_units)
                    .ok_or(FixedFuelError::BoundOverflow)?,
            ),
        },
        (Terminator::Jump { target, .. }, Some(prefix)) => outcome_bounds_from(
            machine,
            *target,
            blocks,
            machines,
            dynamic_call_targets,
            schedule,
            memoized,
            active,
            memoized_machines,
            active_machines,
        )?
        .with_prefix(
            prefix
                .checked_add(terminator_units)
                .ok_or(FixedFuelError::BoundOverflow)?,
        )?,
        (
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            },
            Some(prefix),
        ) => outcome_bounds_from(
            machine,
            when_true.target,
            blocks,
            machines,
            dynamic_call_targets,
            schedule,
            memoized,
            active,
            memoized_machines,
            active_machines,
        )?
        .merge(outcome_bounds_from(
            machine,
            when_false.target,
            blocks,
            machines,
            dynamic_call_targets,
            schedule,
            memoized,
            active,
            memoized_machines,
            active_machines,
        )?)
        .with_prefix(
            prefix
                .checked_add(terminator_units)
                .ok_or(FixedFuelError::BoundOverflow)?,
        )?,
        (Terminator::StructuralCase { cases, .. }, Some(prefix)) => {
            let mut cases = cases.iter();
            let first = cases
                .next()
                .ok_or(FixedFuelError::BranchingNotYetSupported(current))?;
            let mut bounds = outcome_bounds_from(
                machine,
                first.target,
                blocks,
                machines,
                dynamic_call_targets,
                schedule,
                memoized,
                active,
                memoized_machines,
                active_machines,
            )?;
            for case in cases {
                bounds = bounds.merge(outcome_bounds_from(
                    machine,
                    case.target,
                    blocks,
                    machines,
                    dynamic_call_targets,
                    schedule,
                    memoized,
                    active,
                    memoized_machines,
                    active_machines,
                )?);
            }
            bounds.with_prefix(
                prefix
                    .checked_add(terminator_units)
                    .ok_or(FixedFuelError::BoundOverflow)?,
            )?
        }
    };
    active.remove(&current);
    let bounds = OutcomeBounds {
        returned: continued.returned,
        crashed: maximum_optional(crash_units, continued.crashed),
    };
    memoized.insert(current, bounds);
    Ok(bounds)
}

fn compose_cleanup_outcomes(
    cleanup_machines: impl IntoIterator<Item = MachineId>,
    mut bounds: OutcomeBounds,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<OutcomeBounds, FixedFuelError> {
    for cleanup_machine in cleanup_machines {
        let Some(cleanup_prefix) = bounds.returned else {
            break;
        };
        let cleanup_bounds = maximum_machine_outcomes(
            cleanup_machine,
            machines,
            dynamic_call_targets,
            schedule,
            memoized_machines,
            active_machines,
        )?;
        bounds = OutcomeBounds {
            returned: checked_optional_add(cleanup_bounds.returned, cleanup_prefix)?,
            crashed: maximum_optional(
                bounds.crashed,
                checked_optional_add(cleanup_bounds.crashed, cleanup_prefix)?,
            ),
        };
    }
    Ok(bounds)
}

fn derive_segment_bound(
    module: &TerminalModule,
    machine: &TerminalMachine,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<u64, FixedFuelError> {
    let schedule = TerminalFuelSchedule::CURRENT;
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let dynamic_call_targets = dynamic_call_targets(module);
    let mut memoized_machines = BTreeMap::new();
    let mut active_machines = BTreeSet::from([machine.id]);
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    if !blocks.contains_key(&start_block) {
        return Err(FixedFuelError::UnknownBlock(start_block));
    }
    let mut visited = BTreeSet::new();
    let mut current = start_block;
    let mut units = 0_u64;

    loop {
        if !visited.insert(current) {
            return Err(FixedFuelError::ControlCycle(current));
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        for operation in &block.operations {
            units = units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)?;
            if let Some(callee) = operation_callee(machine.id, operation, &dynamic_call_targets) {
                let callee_bounds = maximum_machine_outcomes(
                    callee,
                    &machines,
                    &dynamic_call_targets,
                    schedule,
                    &mut memoized_machines,
                    &mut active_machines,
                )?;
                units = units
                    .checked_add(callee_bounds.returned.ok_or(
                        FixedFuelError::SegmentEndUnreachableAfterCall {
                            block: current,
                            callee,
                        },
                    )?)
                    .ok_or(FixedFuelError::BoundOverflow)?;
            }
        }
        units = units
            .checked_add(schedule.terminator_units(&block.terminator))
            .ok_or(FixedFuelError::BoundOverflow)?;
        if block.terminator.edges().any(|edge| edge == end_edge) {
            return Ok(units);
        }
        match block.terminator {
            Terminator::Jump { target, .. } => current = target,
            Terminator::Conditional { .. } | Terminator::StructuralCase { .. } => {
                return Err(FixedFuelError::BranchingNotYetSupported(current));
            }
            Terminator::Return { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnUnit { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnUnitPartialAffine { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnUnitNominalAffine { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnStructural { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::Crash { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedFuelError {
    SemanticIdentity(CodecError),
    UnknownEntry(MachineId),
    UnknownBlock(BlockId),
    ControlCycle(BlockId),
    CallCycle(MachineId),
    BranchingNotYetSupported(BlockId),
    SegmentEndNotReached {
        requested: EdgeId,
        reached_terminal: EdgeId,
    },
    SegmentEndUnreachableAfterCall {
        block: BlockId,
        callee: MachineId,
    },
    NoTerminalPath(MachineId),
    NotRankedCountdown(MachineId),
    BoundOverflow,
    CertificateMismatch,
}

impl std::fmt::Display for FixedFuelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FixedFuelError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(byte: u8) -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([byte; 32]),
        }
    }

    #[test]
    fn ranked_catalog_header_rejects_semantic_identity_and_schedule_drift() {
        let expected = identity(1);
        let mut catalog = ValidatedRankedCountdownSafePointFuelSegments {
            terminal_psi: expected,
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            machine: MachineId::new(1).expect("nonzero machine"),
            certificates: Vec::new(),
        };
        validate_ranked_catalog_header(expected, &catalog).expect("exact header matches");

        catalog.terminal_psi = identity(2);
        assert_eq!(
            validate_ranked_catalog_header(expected, &catalog),
            Err(FixedFuelError::CertificateMismatch)
        );
        catalog.terminal_psi = expected;
        catalog.schedule = FuelScheduleIdentity::new(
            TerminalFuelSchedule::CURRENT
                .identity()
                .marker()
                .checked_add(1)
                .expect("test schedule marker fits"),
        )
        .expect("test schedule marker is nonzero");
        assert_eq!(
            validate_ranked_catalog_header(expected, &catalog),
            Err(FixedFuelError::CertificateMismatch)
        );
    }

    #[test]
    fn ranked_row_comparison_binds_every_identity_endpoint_and_ceiling() {
        let expected = RankedCountdownSafePointFuelCertificate {
            terminal_psi: identity(1),
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            machine: MachineId::new(1).expect("nonzero machine"),
            start_block: BlockId::new(2).expect("nonzero block"),
            end_edge: EdgeId::new(3).expect("nonzero edge"),
            relevant_preconditions: Vec::new(),
            ceiling_units: 4,
        };
        validate_ranked_certificate_sequence(
            std::slice::from_ref(&expected),
            std::slice::from_ref(&expected),
        )
        .expect("the exact row matches");

        let mut mutations = Vec::new();
        let mut changed = expected.clone();
        changed.terminal_psi = identity(2);
        mutations.push(changed);
        let mut changed = expected.clone();
        changed.schedule = FuelScheduleIdentity::new(2).expect("nonzero schedule");
        mutations.push(changed);
        let mut changed = expected.clone();
        changed.machine = MachineId::new(2).expect("nonzero machine");
        mutations.push(changed);
        let mut changed = expected.clone();
        changed.start_block = BlockId::new(3).expect("nonzero block");
        mutations.push(changed);
        let mut changed = expected.clone();
        changed.end_edge = EdgeId::new(4).expect("nonzero edge");
        mutations.push(changed);
        let mut changed = expected.clone();
        changed.relevant_preconditions = vec![Proposition::Truth];
        mutations.push(changed);
        let mut changed = expected.clone();
        changed.ceiling_units = 5;
        mutations.push(changed);

        for mutation in mutations {
            assert_eq!(
                validate_ranked_certificate_sequence(
                    std::slice::from_ref(&expected),
                    std::slice::from_ref(&mutation),
                ),
                Err(FixedFuelError::CertificateMismatch)
            );
        }
    }
}
