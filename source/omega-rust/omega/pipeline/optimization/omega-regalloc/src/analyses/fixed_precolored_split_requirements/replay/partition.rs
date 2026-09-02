//! Independent register partition replay.

use std::collections::BTreeSet;

use omega_register_model::RegisterViewId;

use crate::{
    FixedPrecoloredInterval, FixedPrecoloredRegisterSplitRequirements,
    FixedPrecoloredSourceFragmentRequirements, FixedPrecoloredSourceSegment,
    FixedPrecoloredSourceSegmentId, FixedPrecoloredSourceSegmentOpening,
    FixedPrecoloredSplitRequirementError, LiveRangePoint, VirtualLiveRange, VirtualPointLegality,
    VirtualRegisterAllocationLegality,
};

use super::{cuts::CutRows, topology, work::Work};

#[allow(clippy::too_many_arguments)]
pub(super) fn register(
    function: usize,
    range: &VirtualLiveRange,
    legality: &VirtualRegisterAllocationLegality,
    fixed: &[&FixedPrecoloredInterval],
    tied: bool,
    early_clobber: bool,
    work: &mut Work,
) -> Result<FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSplitRequirementError> {
    work.register(
        fixed.len(),
        range.edge_connectors.len(),
        legality.entry_transitions.len(),
    )?;
    let register = range.virtual_register.0;
    if range.class != legality.class {
        return Err(FixedPrecoloredSplitRequirementError::RegisterMismatch { function, register });
    }
    if tied {
        return Err(
            FixedPrecoloredSplitRequirementError::UnsupportedTiedRegister { function, register },
        );
    }
    if early_clobber {
        return Err(
            FixedPrecoloredSplitRequirementError::UnsupportedEarlyClobberDomain {
                function,
                register,
            },
        );
    }

    let topology = topology::reconstruct(function, range, legality)?;
    let mut cuts = CutRows::collect(fixed, &legality.entry_transitions);
    let mut id = 0u32;
    let entry = &topology[0];
    let initial = candidates(function, register, entry.points.first())?;
    let (first, source_exit) = replay_fragment(
        function,
        register,
        entry,
        initial,
        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1,
        &mut cuts,
        &mut id,
        work,
    )?;
    let mut fragments = vec![first];
    for input in topology.iter().skip(1) {
        let edge = input.incoming.expect("replayed topology has incoming edge");
        let at_entry = candidates(function, register, input.points.first())?;
        let shared = source_exit
            .intersection(&at_entry)
            .copied()
            .collect::<BTreeSet<_>>();
        let (domain, opening) = if shared.is_empty() {
            let first_point = input.points.first().expect("candidate point exists");
            let (site, destination_view) = cuts.boundary(function, register, first_point)?;
            cuts.require_transition(function, register, &source_exit, site, destination_view)?;
            work.incompatible_boundary()?;
            (
                at_entry,
                FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                    incoming: Some(edge),
                    site,
                    destination_view,
                },
            )
        } else {
            (
                shared,
                FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector: edge },
            )
        };
        let (fragment, _) = replay_fragment(
            function, register, input, domain, opening, &mut cuts, &mut id, work,
        )?;
        fragments.push(fragment);
    }
    cuts.finish(function, register)?;
    Ok(FixedPrecoloredRegisterSplitRequirements {
        virtual_register: range.virtual_register,
        class: range.class,
        fragments,
    })
}

#[allow(clippy::too_many_arguments)]
fn replay_fragment(
    function: usize,
    register: u32,
    input: &topology::FragmentInput<'_>,
    initial: BTreeSet<RegisterViewId>,
    initial_opening: FixedPrecoloredSourceSegmentOpening,
    cuts: &mut CutRows<'_>,
    next_id: &mut u32,
    work: &mut Work,
) -> Result<
    (
        FixedPrecoloredSourceFragmentRequirements,
        BTreeSet<RegisterViewId>,
    ),
    FixedPrecoloredSplitRequirementError,
> {
    let mut start = input.source.start;
    let mut opening = initial_opening;
    let mut common = Some(initial);
    let mut segments = Vec::new();
    for (offset, point) in input.points.iter().enumerate() {
        work.point(point.candidates.len())?;
        let offset = u32::try_from(offset).map_err(|_| {
            FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register }
        })?;
        let expected =
            input.source.start.0.checked_add(offset).ok_or(
                FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register },
            )?;
        if point.block != input.source.block || point.point.0 != expected {
            return Err(
                FixedPrecoloredSplitRequirementError::NonCanonicalPointDomain {
                    function,
                    register,
                    point: expected,
                },
            );
        }
        if offset == 0 {
            continue;
        }
        let prior = common.take().expect("initial domain established");
        let at_point = point.candidates.iter().copied().collect::<BTreeSet<_>>();
        let intersection = prior
            .intersection(&at_point)
            .copied()
            .collect::<BTreeSet<_>>();
        if !intersection.is_empty() {
            common = Some(intersection);
            continue;
        }
        let (site, destination_view) = cuts.boundary(function, register, point)?;
        commit(
            function,
            register,
            &mut segments,
            next_id,
            start,
            point.point,
            prior.clone(),
            opening,
            work,
        )?;
        cuts.mark_transition(&prior, site, destination_view);
        work.incompatible_boundary()?;
        start = point.point;
        opening = FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
            incoming: None,
            site,
            destination_view,
        };
        common = Some(at_point);
    }
    let common = common.ok_or(
        FixedPrecoloredSplitRequirementError::MissingSourceFragment { function, register },
    )?;
    commit(
        function,
        register,
        &mut segments,
        next_id,
        start,
        input.source.end,
        common.clone(),
        opening,
        work,
    )?;
    Ok((
        FixedPrecoloredSourceFragmentRequirements {
            block: input.source.block,
            source_start: input.source.start,
            source_end: input.source.end,
            segments,
        },
        common,
    ))
}

fn candidates(
    function: usize,
    register: u32,
    point: Option<&VirtualPointLegality>,
) -> Result<BTreeSet<RegisterViewId>, FixedPrecoloredSplitRequirementError> {
    point
        .map(|point| point.candidates.iter().copied().collect())
        .ok_or(FixedPrecoloredSplitRequirementError::MissingSourceFragment { function, register })
}

#[allow(clippy::too_many_arguments)]
fn commit(
    function: usize,
    register: u32,
    output: &mut Vec<FixedPrecoloredSourceSegment>,
    next_id: &mut u32,
    start: LiveRangePoint,
    end: LiveRangePoint,
    candidates: BTreeSet<RegisterViewId>,
    opening: FixedPrecoloredSourceSegmentOpening,
    work: &mut Work,
) -> Result<(), FixedPrecoloredSplitRequirementError> {
    if start >= end || candidates.is_empty() {
        return Err(
            FixedPrecoloredSplitRequirementError::NonCanonicalPointDomain {
                function,
                register,
                point: start.0,
            },
        );
    }
    let id = *next_id;
    *next_id = next_id.checked_add(1).ok_or(
        FixedPrecoloredSplitRequirementError::SegmentIdentityOverflow { function, register },
    )?;
    output.push(FixedPrecoloredSourceSegment {
        id: FixedPrecoloredSourceSegmentId(id),
        start,
        end,
        candidates: candidates.into_iter().collect(),
        opening,
    });
    work.segment()
}
