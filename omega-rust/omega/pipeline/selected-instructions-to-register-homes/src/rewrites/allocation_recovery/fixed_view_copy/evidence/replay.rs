//! Independently keyed reconstruction of segment-home boundary authority.

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::OptimizationWorkUsage;
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::MachineId;

use crate::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomePolicy,
    FixedPrecoloredSourceSegmentHome, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPolicy, FixedViewCopyError,
    FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
    ValidatedAllocationLegality, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges,
};

use super::{AuthenticatedFixedViewBoundary, FixedViewBoundaryEvidence};

pub(crate) fn reconstruct(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
) -> Result<FixedViewBoundaryEvidence, FixedViewCopyError> {
    replay_roots(ranges, legality, fixed, requirements, homes)?;
    let mut work = ReplayWork::new();
    let mut boundaries = replay_roster(
        &requirements.plan().functions,
        &homes.plan().functions,
        false,
        &mut work,
    )?;
    let structural = replay_roster(
        &requirements.plan().structural_unit_functions,
        &homes.plan().structural_unit_functions,
        true,
        &mut work,
    )?;
    if !structural.is_empty() {
        return Err(FixedViewCopyError::UnsupportedSegmentBoundarySet { function: 0 });
    }
    boundaries.sort_by_key(|boundary| {
        (
            boundary.function,
            boundary.virtual_register.0,
            boundary.block.0,
            boundary.destination_segment.0,
        )
    });
    Ok(FixedViewBoundaryEvidence {
        boundaries,
        usage: work.finish()?,
    })
}

fn replay_roots(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
) -> Result<(), FixedViewCopyError> {
    let requirements_receipt = requirements.receipt();
    let homes_receipt = homes.receipt();
    if fixed.receipt().ranges() != ranges.receipt().identity()
        || fixed.receipt().legality() != legality.receipt().identity()
        || fixed.receipt().policy()
            != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
        || requirements_receipt.fixed_intervals() != fixed.receipt().identity()
        || requirements_receipt.ranges() != ranges.receipt().identity()
        || requirements_receipt.legality() != legality.receipt().identity()
        || requirements_receipt.policy()
            != FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1
        || homes_receipt.fixed_intervals() != fixed.receipt().identity()
        || homes_receipt.split_requirements() != requirements_receipt.identity()
        || homes_receipt.ranges() != ranges.receipt().identity()
        || homes_receipt.legality() != legality.receipt().identity()
        || homes_receipt.register_environment() != legality.receipt().register_environment()
        || homes_receipt.allocator_availability() != legality.receipt().allocator_availability()
        || homes_receipt.optimization_unit() != ranges.receipt().optimization_unit()
        || homes_receipt.fuel_schedule() != ranges.receipt().fuel_schedule()
        || homes_receipt.target() != ranges.plan().target
        || homes_receipt.policy()
            != FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1
    {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    Ok(())
}

fn replay_roster(
    requirements: &[FunctionFixedPrecoloredSplitRequirements],
    homes: &[FunctionFixedPrecoloredSegmentHomes],
    structural: bool,
    work: &mut ReplayWork,
) -> Result<Vec<AuthenticatedFixedViewBoundary>, FixedViewCopyError> {
    let mut home_functions = BTreeMap::<MachineId, &FunctionFixedPrecoloredSegmentHomes>::new();
    for function in homes {
        if home_functions.insert(function.machine, function).is_some() {
            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
        }
    }
    let mut consumed_functions = BTreeSet::new();
    let mut boundaries = Vec::new();
    for (function_index, requirement_function) in requirements.iter().enumerate() {
        work.function()?;
        let home_function = home_functions
            .get(&requirement_function.machine)
            .copied()
            .ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
        consumed_functions.insert(requirement_function.machine);
        let mut assignments = BTreeMap::new();
        for assignment in &home_function.assignments {
            work.home()?;
            if assignments
                .insert(
                    (assignment.virtual_register, assignment.source_segment),
                    assignment,
                )
                .is_some()
            {
                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
            }
        }
        let mut consumed_assignments = BTreeSet::new();
        for register in &requirement_function.registers {
            work.register()?;
            let mut closing = BTreeMap::<_, FixedPrecoloredSourceSegmentHome>::new();
            for fragment in &register.fragments {
                work.fragment()?;
                let mut previous = None;
                for segment in &fragment.segments {
                    work.segment()?;
                    let key = (register.virtual_register, segment.id);
                    let assignment = assignments
                        .get(&key)
                        .copied()
                        .ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                    consumed_assignments.insert(key);
                    replay_assignment(
                        register.virtual_register,
                        register.class,
                        segment.id,
                        &segment.candidates,
                        assignment,
                    )?;
                    match segment.opening {
                        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1 => {
                            if previous.is_some() {
                                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                            }
                        }
                        FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector } => {
                            let predecessor = closing
                                .get(&connector.source)
                                .copied()
                                .ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                            if connector.target != fragment.block
                                || predecessor.allocation_domain != assignment.allocation_domain
                                || predecessor.view != assignment.view
                            {
                                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                            }
                        }
                        FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                            incoming,
                            site,
                            destination_view,
                        } => {
                            let predecessor = match incoming {
                                Some(connector) => {
                                    if connector.target != fragment.block {
                                        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                                    }
                                    closing.get(&connector.source).copied()
                                }
                                None => previous,
                            }
                            .ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                            if assignment.view != destination_view
                                || predecessor.allocation_domain == assignment.allocation_domain
                            {
                                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                            }
                            work.boundary()?;
                            boundaries.push(AuthenticatedFixedViewBoundary {
                                function: function_index,
                                machine: requirement_function.machine,
                                virtual_register: register.virtual_register,
                                class: register.class,
                                source_segment: predecessor.source_segment,
                                source_domain: predecessor.allocation_domain,
                                from_view: predecessor.view,
                                destination_segment: assignment.source_segment,
                                destination_domain: assignment.allocation_domain,
                                site,
                                block: fragment.block,
                                to_view: assignment.view,
                                incoming,
                            });
                        }
                    }
                    previous = Some(*assignment);
                }
                closing.insert(
                    fragment.block,
                    previous.ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?,
                );
            }
        }
        if consumed_assignments.len() != assignments.len() {
            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
        }
        if structural && boundaries.iter().any(|row| row.function == function_index) {
            return Err(FixedViewCopyError::UnsupportedSegmentBoundarySet {
                function: function_index,
            });
        }
    }
    if consumed_functions.len() != home_functions.len() {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    Ok(boundaries)
}

fn replay_assignment(
    register: VirtualRegisterId,
    class: register_model::RegisterClassId,
    segment: FixedPrecoloredSourceSegmentId,
    candidates: &[register_model::RegisterViewId],
    assignment: &FixedPrecoloredSourceSegmentHome,
) -> Result<(), FixedViewCopyError> {
    if assignment.virtual_register != register
        || assignment.class != class
        || assignment.source_segment != segment
        || !candidates.contains(&assignment.view)
    {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    Ok(())
}

struct ReplayWork {
    functions: u64,
    registers: u64,
    fragments: u64,
    segments: u64,
    homes: u64,
    boundaries: u64,
}

impl ReplayWork {
    const fn new() -> Self {
        Self {
            functions: 0,
            registers: 0,
            fragments: 0,
            segments: 0,
            homes: 0,
            boundaries: 0,
        }
    }
    fn function(&mut self) -> Result<(), FixedViewCopyError> {
        replay_bump(&mut self.functions)
    }
    fn register(&mut self) -> Result<(), FixedViewCopyError> {
        replay_bump(&mut self.registers)
    }
    fn fragment(&mut self) -> Result<(), FixedViewCopyError> {
        replay_bump(&mut self.fragments)
    }
    fn segment(&mut self) -> Result<(), FixedViewCopyError> {
        replay_bump(&mut self.segments)
    }
    fn home(&mut self) -> Result<(), FixedViewCopyError> {
        replay_bump(&mut self.homes)
    }
    fn boundary(&mut self) -> Result<(), FixedViewCopyError> {
        replay_bump(&mut self.boundaries)
    }
    fn finish(self) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
        Ok(OptimizationWorkUsage {
            rule_evaluations: replay_add(replay_add(1, self.functions)?, self.boundaries)?,
            candidates: self.boundaries,
            validation_steps: replay_add(
                replay_add(
                    replay_add(
                        replay_add(
                            replay_add(replay_add(1, self.functions)?, self.registers)?,
                            self.fragments,
                        )?,
                        self.segments,
                    )?,
                    self.homes,
                )?,
                self.boundaries,
            )?,
            commits: self.boundaries,
            iterations: replay_add(
                replay_add(
                    replay_add(replay_add(1, self.functions)?, self.registers)?,
                    self.segments,
                )?,
                self.boundaries,
            )?,
        })
    }
}

fn replay_bump(value: &mut u64) -> Result<(), FixedViewCopyError> {
    *value = value
        .checked_add(1)
        .ok_or(FixedViewCopyError::WorkOverflow)?;
    Ok(())
}

fn replay_add(left: u64, right: u64) -> Result<u64, FixedViewCopyError> {
    left.checked_add(right)
        .ok_or(FixedViewCopyError::WorkOverflow)
}
