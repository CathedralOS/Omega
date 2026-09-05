//! Positional split-requirement to segment-home join used by production.

use std::collections::BTreeMap;

use optimization_core::OptimizationWorkUsage;

use crate::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomePolicy,
    FixedPrecoloredSourceSegmentHome, FixedPrecoloredSourceSegmentOpening,
    FixedPrecoloredSplitRequirementPolicy, FixedViewCopyError, FunctionFixedPrecoloredSegmentHomes,
    FunctionFixedPrecoloredSplitRequirements, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
    ValidatedFixedPrecoloredSplitRequirements, ValidatedLiveRanges,
};

use super::{AuthenticatedFixedViewBoundary, FixedViewBoundaryEvidence};

pub(crate) fn derive(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
) -> Result<FixedViewBoundaryEvidence, FixedViewCopyError> {
    validate_roots(ranges, legality, fixed, requirements, homes)?;
    let mut work = Work::new();
    let mut boundaries = derive_roster(
        requirements.plan().functions.as_slice(),
        homes.plan().functions.as_slice(),
        false,
        &mut work,
    )?;
    let structural = derive_roster(
        requirements.plan().structural_unit_functions.as_slice(),
        homes.plan().structural_unit_functions.as_slice(),
        true,
        &mut work,
    )?;
    if !structural.is_empty() {
        return Err(FixedViewCopyError::UnsupportedSegmentBoundarySet { function: 0 });
    }
    boundaries.sort_by_key(boundary_key);
    Ok(FixedViewBoundaryEvidence {
        boundaries,
        usage: work.finish()?,
    })
}

fn validate_roots(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
) -> Result<(), FixedViewCopyError> {
    if fixed.receipt().ranges() != ranges.receipt().identity()
        || fixed.receipt().legality() != legality.receipt().identity()
        || fixed.receipt().policy()
            != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
        || requirements.receipt().fixed_intervals() != fixed.receipt().identity()
        || requirements.receipt().ranges() != ranges.receipt().identity()
        || requirements.receipt().legality() != legality.receipt().identity()
        || requirements.receipt().policy()
            != FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1
        || homes.receipt().fixed_intervals() != fixed.receipt().identity()
        || homes.receipt().split_requirements() != requirements.receipt().identity()
        || homes.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().legality() != legality.receipt().identity()
        || homes.receipt().register_environment() != legality.receipt().register_environment()
        || homes.receipt().allocator_availability() != legality.receipt().allocator_availability()
        || homes.receipt().optimization_unit() != ranges.receipt().optimization_unit()
        || homes.receipt().fuel_schedule() != ranges.receipt().fuel_schedule()
        || homes.receipt().target() != ranges.plan().target
        || homes.receipt().policy()
            != FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1
    {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    Ok(())
}

fn derive_roster(
    requirements: &[FunctionFixedPrecoloredSplitRequirements],
    homes: &[FunctionFixedPrecoloredSegmentHomes],
    structural: bool,
    work: &mut Work,
) -> Result<Vec<AuthenticatedFixedViewBoundary>, FixedViewCopyError> {
    if requirements.len() != homes.len() {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    let mut boundaries = Vec::new();
    for (function, (requirements, homes)) in requirements.iter().zip(homes).enumerate() {
        work.function()?;
        if requirements.machine != homes.machine {
            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
        }
        let mut assignment_index = 0;
        for register in &requirements.registers {
            work.register()?;
            let mut closing = BTreeMap::<_, FixedPrecoloredSourceSegmentHome>::new();
            for fragment in &register.fragments {
                work.fragment()?;
                let mut previous = None;
                for segment in &fragment.segments {
                    work.segment()?;
                    let assignment = homes
                        .assignments
                        .get(assignment_index)
                        .ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                    assignment_index += 1;
                    work.home()?;
                    validate_assignment(
                        register.virtual_register,
                        register.class,
                        segment,
                        assignment,
                    )?;
                    match segment.opening {
                        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1 => {
                            if previous.is_some() {
                                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                            }
                        }
                        FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector } => {
                            let source = closing.get(&connector.source).copied().ok_or(
                                FixedViewCopyError::SegmentEvidenceMismatch,
                            )?;
                            if connector.target != fragment.block
                                || source.allocation_domain != assignment.allocation_domain
                                || source.view != assignment.view
                            {
                                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                            }
                        }
                        FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                            incoming,
                            site,
                            destination_view,
                        } => {
                            let source = match incoming {
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
                                || source.allocation_domain == assignment.allocation_domain
                            {
                                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                            }
                            work.boundary()?;
                            boundaries.push(AuthenticatedFixedViewBoundary {
                                function,
                                machine: requirements.machine,
                                virtual_register: register.virtual_register,
                                class: register.class,
                                source_segment: source.source_segment,
                                source_domain: source.allocation_domain,
                                from_view: source.view,
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
                let last = previous.ok_or(FixedViewCopyError::SegmentEvidenceMismatch)?;
                closing.insert(fragment.block, last);
            }
        }
        if assignment_index != homes.assignments.len() {
            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
        }
        if structural
            && boundaries
                .iter()
                .any(|boundary| boundary.function == function)
        {
            return Err(FixedViewCopyError::UnsupportedSegmentBoundarySet { function });
        }
    }
    Ok(boundaries)
}

fn validate_assignment(
    register: selected_instructions::VirtualRegisterId,
    class: register_model::RegisterClassId,
    segment: &crate::FixedPrecoloredSourceSegment,
    assignment: &FixedPrecoloredSourceSegmentHome,
) -> Result<(), FixedViewCopyError> {
    if assignment.virtual_register != register
        || assignment.class != class
        || assignment.source_segment != segment.id
        || segment.candidates.binary_search(&assignment.view).is_err()
    {
        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
    }
    Ok(())
}

fn boundary_key(boundary: &AuthenticatedFixedViewBoundary) -> (usize, u32, u32, u32) {
    (
        boundary.function,
        boundary.virtual_register.0,
        boundary.block.0,
        boundary.destination_segment.0,
    )
}

struct Work {
    functions: u64,
    registers: u64,
    fragments: u64,
    segments: u64,
    homes: u64,
    boundaries: u64,
}

impl Work {
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
        bump(&mut self.functions)
    }
    fn register(&mut self) -> Result<(), FixedViewCopyError> {
        bump(&mut self.registers)
    }
    fn fragment(&mut self) -> Result<(), FixedViewCopyError> {
        bump(&mut self.fragments)
    }
    fn segment(&mut self) -> Result<(), FixedViewCopyError> {
        bump(&mut self.segments)
    }
    fn home(&mut self) -> Result<(), FixedViewCopyError> {
        bump(&mut self.homes)
    }
    fn boundary(&mut self) -> Result<(), FixedViewCopyError> {
        bump(&mut self.boundaries)
    }
    fn finish(self) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
        Ok(OptimizationWorkUsage {
            rule_evaluations: add(add(1, self.functions)?, self.boundaries)?,
            candidates: self.boundaries,
            validation_steps: add(
                add(
                    add(
                        add(
                            add(add(1, self.functions)?, self.registers)?,
                            self.fragments,
                        )?,
                        self.segments,
                    )?,
                    self.homes,
                )?,
                self.boundaries,
            )?,
            commits: self.boundaries,
            iterations: add(
                add(add(add(1, self.functions)?, self.registers)?, self.segments)?,
                self.boundaries,
            )?,
        })
    }
}

fn bump(value: &mut u64) -> Result<(), FixedViewCopyError> {
    *value = value
        .checked_add(1)
        .ok_or(FixedViewCopyError::WorkOverflow)?;
    Ok(())
}

fn add(left: u64, right: u64) -> Result<u64, FixedViewCopyError> {
    left.checked_add(right)
        .ok_or(FixedViewCopyError::WorkOverflow)
}
