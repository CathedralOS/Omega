//! Exact-view-domain partitioning for one source register.

use std::collections::BTreeSet;

use omega_register_model::RegisterViewId;

use crate::{
    FixedPrecoloredInterval, FixedPrecoloredRegisterSplitRequirements,
    FixedPrecoloredSourceFragmentRequirements, FixedPrecoloredSourceSegment,
    FixedPrecoloredSourceSegmentId, FixedPrecoloredSourceSegmentOpening,
    FixedPrecoloredSplitRequirementError, LiveRangeFragment, LiveRangePoint, VirtualLiveRange,
    VirtualPointLegality, VirtualRegisterAllocationLegality,
};

use super::{cuts::CutIndex, topology, work::Work};

#[allow(clippy::too_many_arguments)]
pub(super) fn register(
    function: usize,
    range: &VirtualLiveRange,
    legality: &VirtualRegisterAllocationLegality,
    fixed: &[FixedPrecoloredInterval],
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
    if range.virtual_register != legality.virtual_register || range.class != legality.class {
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

    let inputs = topology::fanout(function, range, legality)?;
    let mut cuts = CutIndex::new(fixed, &legality.entry_transitions);
    let mut next_segment_id = 0u32;
    let source_input = &inputs[0];
    let source_initial = point_domain(function, register, source_input.points.first())?;
    let (source, source_closing) = fragment(
        function,
        register,
        source_input.fragment,
        source_input.points,
        source_initial,
        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1,
        &mut cuts,
        &mut next_segment_id,
        work,
    )?;
    let mut fragments = vec![source];

    for input in &inputs[1..] {
        let connector = input.incoming.expect("fanout topology authenticated");
        let first_domain = point_domain(function, register, input.points.first())?;
        let compatible = source_closing
            .intersection(&first_domain)
            .copied()
            .collect::<BTreeSet<_>>();
        let (initial, opening) = if compatible.is_empty() {
            let first = input.points.first().expect("point domain established");
            let (site, destination_view) = cuts.boundary(function, register, first)?;
            cuts.require_transition(function, register, &source_closing, site, destination_view)?;
            work.incompatible_boundary()?;
            (
                first_domain,
                FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                    incoming: Some(connector),
                    site,
                    destination_view,
                },
            )
        } else {
            (
                compatible,
                FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { connector },
            )
        };
        let (derived, _) = fragment(
            function,
            register,
            input.fragment,
            input.points,
            initial,
            opening,
            &mut cuts,
            &mut next_segment_id,
            work,
        )?;
        fragments.push(derived);
    }
    cuts.finish(function, register)?;
    Ok(FixedPrecoloredRegisterSplitRequirements {
        virtual_register: range.virtual_register,
        class: range.class,
        fragments,
    })
}

#[allow(clippy::too_many_arguments)]
fn fragment(
    function: usize,
    register: u32,
    source: &LiveRangeFragment,
    points: &[VirtualPointLegality],
    mut shared: BTreeSet<RegisterViewId>,
    mut opening: FixedPrecoloredSourceSegmentOpening,
    cuts: &mut CutIndex<'_>,
    next_segment_id: &mut u32,
    work: &mut Work,
) -> Result<
    (
        FixedPrecoloredSourceFragmentRequirements,
        BTreeSet<RegisterViewId>,
    ),
    FixedPrecoloredSplitRequirementError,
> {
    let mut segment_start = source.start;
    let mut segments = Vec::new();
    for (offset, point) in points.iter().enumerate() {
        work.point(point.candidates.len())?;
        let offset = u32::try_from(offset).map_err(|_| {
            FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register }
        })?;
        let expected =
            source.start.0.checked_add(offset).ok_or(
                FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register },
            )?;
        if point.block != source.block || point.point != LiveRangePoint(expected) {
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
        let prior = shared.clone();
        shared.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
        if !shared.is_empty() {
            continue;
        }
        let (site, destination_view) = cuts.boundary(function, register, point)?;
        push_segment(
            function,
            register,
            &mut segments,
            next_segment_id,
            segment_start,
            point.point,
            prior.clone(),
            opening,
            work,
        )?;
        cuts.use_transition(&prior, site, destination_view);
        work.incompatible_boundary()?;
        segment_start = point.point;
        opening = FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
            incoming: None,
            site,
            destination_view,
        };
        shared = point.candidates.iter().copied().collect();
    }
    push_segment(
        function,
        register,
        &mut segments,
        next_segment_id,
        segment_start,
        source.end,
        shared.clone(),
        opening,
        work,
    )?;
    Ok((
        FixedPrecoloredSourceFragmentRequirements {
            block: source.block,
            source_start: source.start,
            source_end: source.end,
            segments,
        },
        shared,
    ))
}

fn point_domain(
    function: usize,
    register: u32,
    point: Option<&VirtualPointLegality>,
) -> Result<BTreeSet<RegisterViewId>, FixedPrecoloredSplitRequirementError> {
    let point = point.ok_or(
        FixedPrecoloredSplitRequirementError::MissingSourceFragment { function, register },
    )?;
    Ok(point.candidates.iter().copied().collect())
}

#[allow(clippy::too_many_arguments)]
fn push_segment(
    function: usize,
    register: u32,
    segments: &mut Vec<FixedPrecoloredSourceSegment>,
    next_segment_id: &mut u32,
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
    let id = *next_segment_id;
    *next_segment_id = next_segment_id.checked_add(1).ok_or(
        FixedPrecoloredSplitRequirementError::SegmentIdentityOverflow { function, register },
    )?;
    segments.push(FixedPrecoloredSourceSegment {
        id: FixedPrecoloredSourceSegmentId(id),
        start,
        end,
        candidates: candidates.into_iter().collect(),
        opening,
    });
    work.segment()
}
