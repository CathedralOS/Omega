/*
  Fixed-constraint live-range split requirement analysis
  (`analyze_fixed_precolored_split_requirements`,
  `ValidatedFixedPrecoloredSplitRequirements`,
  `FixedPrecoloredSplitRequirementValidationReceipt`).

  Partitions authenticated source live points and records cut boundaries where
  incoming and fixed-use exact-view domains are incompatible.
*/

mod compute {

    mod cuts {

        use super::super::{SiteKey, site_key, site_point};
        use std::collections::{BTreeMap, BTreeSet};

        use register_model::{RegisterOperandAccess, RegisterViewId};

        use crate::FixedPrecoloredSplitRequirementError;
        use register_homes::{
            EntryFixedViewTransition, FixedPrecoloredInterval, VirtualLegalitySpan,
        };
        use selected_instructions::VirtualFixedConstraintSite;

        pub(super) struct CutIndex<'a> {
            fixed: BTreeMap<(u32, u32), Vec<&'a FixedPrecoloredInterval>>,
            transitions: BTreeMap<(SiteKey, u16), BTreeSet<u16>>,
            used: BTreeSet<(SiteKey, u16, u16)>,
        }

        impl<'a> CutIndex<'a> {
            pub fn new(
                fixed: &'a [FixedPrecoloredInterval],
                transitions: &[EntryFixedViewTransition],
            ) -> Self {
                let mut fixed_by_point = BTreeMap::<_, Vec<_>>::new();
                for row in fixed {
                    fixed_by_point
                        .entry((row.block.0, row.start.0))
                        .or_default()
                        .push(row);
                }
                let mut transition_index = BTreeMap::<_, BTreeSet<_>>::new();
                for row in transitions {
                    transition_index
                        .entry((site_key(row.to_site), row.to_view.0))
                        .or_default()
                        .insert(row.from_view.0);
                }
                Self {
                    fixed: fixed_by_point,
                    transitions: transition_index,
                    used: BTreeSet::new(),
                }
            }

            pub(super) fn boundary(
                &self,
                function: usize,
                register: u32,
                point: &VirtualLegalitySpan,
            ) -> Result<
                (VirtualFixedConstraintSite, RegisterViewId),
                FixedPrecoloredSplitRequirementError,
            > {
                let rows = self.fixed.get(&(point.block.0, point.start.0));
                let Some(rows) = rows else {
                    return Err(
                        FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                            function,
                            register,
                            point: point.start.0,
                        },
                    );
                };
                if rows.len() != 1 {
                    return Err(
                        FixedPrecoloredSplitRequirementError::AmbiguousFixedCutSite {
                            function,
                            register,
                            point: point.start.0,
                        },
                    );
                }
                let row = rows[0];
                let VirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    access,
                    ..
                } = row.site
                else {
                    return Err(
                        FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                            function,
                            register,
                            point: point.start.0,
                        },
                    );
                };
                if access != RegisterOperandAccess::Use {
                    return Err(
                        FixedPrecoloredSplitRequirementError::UnsupportedFixedTransitionAccess {
                            function,
                            register,
                            instruction: instruction.0,
                            operand,
                        },
                    );
                }
                if point.candidates.as_slice() != [row.view] {
                    return Err(
                        FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                            function,
                            register,
                            point: point.start.0,
                        },
                    );
                }
                Ok((row.site, row.view))
            }

            pub(super) fn require_transition(
                &mut self,
                function: usize,
                register: u32,
                source: &BTreeSet<RegisterViewId>,
                site: VirtualFixedConstraintSite,
                destination: RegisterViewId,
            ) -> Result<(), FixedPrecoloredSplitRequirementError> {
                if self.use_transition(source, site, destination) {
                    Ok(())
                } else {
                    Err(
                        FixedPrecoloredSplitRequirementError::UnauthenticatedDomainBreak {
                            function,
                            register,
                            point: site_point(site),
                        },
                    )
                }
            }

            pub(super) fn use_transition(
                &mut self,
                source: &BTreeSet<RegisterViewId>,
                site: VirtualFixedConstraintSite,
                destination: RegisterViewId,
            ) -> bool {
                let key = (site_key(site), destination.0);
                let Some(from) = self.transitions.get(&key) else {
                    return false;
                };
                let mut matching = from
                    .iter()
                    .copied()
                    .filter(|view| source.contains(&RegisterViewId(*view)));
                let Some(chosen) = matching.next() else {
                    return false;
                };
                if matching.next().is_some() {
                    return false;
                }
                self.used.insert((key.0, key.1, chosen));
                true
            }

            pub(super) fn finish(
                self,
                function: usize,
                register: u32,
            ) -> Result<(), FixedPrecoloredSplitRequirementError> {
                let exhausted = self.used.iter().all(|(site, destination, from)| {
                    self.transitions
                        .get(&(*site, *destination))
                        .is_some_and(|declared| declared.contains(from))
                });
                if exhausted {
                    Ok(())
                } else {
                    Err(
                        FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange {
                            function,
                            register,
                        },
                    )
                }
            }
        }
    }
    mod function {

        use super::super::early_clobber_uses;
        use std::collections::BTreeSet;

        use selected_instructions::VirtualRegisterId;

        use crate::FixedPrecoloredSplitRequirementError;
        use register_homes::{
            FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
            FunctionFixedPrecoloredSplitRequirements,
        };
        use selected_instructions::FunctionLiveRanges;

        use super::partition;
        use crate::work_accounting::{Work, count};

        pub(super) fn derive(
            function: usize,
            ranges: &FunctionLiveRanges,
            legality: &FunctionAllocationLegality,
            fixed: &FunctionFixedPrecoloredIntervals,
            work: &mut Work,
        ) -> Result<FunctionFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError>
        {
            let tied = tied_registers(ranges);
            let early_uses = early_clobber_uses(ranges)?;
            work.rule_evaluations(1)?;
            work.validation_steps(1)?;
            work.validation_steps(count(ranges.tied_pairs.len())?)?;
            work.validation_steps(count(ranges.early_clobbers.len())?)?;
            work.validation_steps(count(early_uses)?)?;
            work.iterations(1)?;
            if ranges.machine != legality.machine
                || ranges.machine != fixed.machine
                || ranges.virtual_registers.len() != legality.virtual_registers.len()
            {
                return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
            }

            let mut fixed_offset = 0;
            let mut registers = Vec::with_capacity(ranges.virtual_registers.len());
            for (range, legal) in ranges
                .virtual_registers
                .iter()
                .zip(&legality.virtual_registers)
            {
                let start = fixed_offset;
                while fixed
                    .intervals
                    .get(fixed_offset)
                    .is_some_and(|row| row.virtual_register == range.virtual_register)
                {
                    fixed_offset += 1;
                }
                if fixed
                    .intervals
                    .get(fixed_offset)
                    .is_some_and(|row| row.virtual_register < range.virtual_register)
                {
                    return Err(FixedPrecoloredSplitRequirementError::RegisterMismatch {
                        function,
                        register: range.virtual_register.0,
                    });
                }
                registers.push(partition::register(
                    function,
                    range,
                    legal,
                    &fixed.intervals[start..fixed_offset],
                    tied.contains(&range.virtual_register),
                    work,
                )?);
            }
            if fixed_offset != fixed.intervals.len() {
                return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
            }
            Ok(FunctionFixedPrecoloredSplitRequirements {
                machine: ranges.machine,
                registers,
            })
        }

        fn tied_registers(ranges: &FunctionLiveRanges) -> BTreeSet<VirtualRegisterId> {
            ranges
                .tied_pairs
                .iter()
                .flat_map(|row| [row.use_virtual_register, row.def_virtual_register])
                .collect()
        }
    }
    mod partition {

        use std::collections::{BTreeMap, BTreeSet};

        use register_model::RegisterViewId;

        use crate::FixedPrecoloredSplitRequirementError;
        use register_homes::{
            FixedPrecoloredInterval, FixedPrecoloredRegisterSplitRequirements,
            FixedPrecoloredSourceFragmentRequirements, FixedPrecoloredSourceSegment,
            FixedPrecoloredSourceSegmentId, FixedPrecoloredSourceSegmentOpening,
            VirtualLegalitySpan, VirtualRegisterAllocationLegality,
        };
        use selected_instructions::{LiveRangeFragment, LiveRangePoint, VirtualLiveRange};

        use super::{cuts::CutIndex, topology};
        use crate::work_accounting::{Work, WorkDefect, count};

        #[allow(clippy::too_many_arguments)]
        pub(super) fn register(
            function: usize,
            range: &VirtualLiveRange,
            legality: &VirtualRegisterAllocationLegality,
            fixed: &[FixedPrecoloredInterval],
            tied: bool,
            work: &mut Work,
        ) -> Result<FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSplitRequirementError>
        {
            work.rule_evaluations(1)?;
            work.validation_steps(1)?;
            work.validation_steps(count(fixed.len())?)?;
            work.validation_steps(count(range.edge_connectors.len())?)?;
            work.validation_steps(count(legality.entry_transitions.len())?)?;
            work.iterations(1)?;
            let register = range.virtual_register.0;
            if range.virtual_register != legality.virtual_register || range.class != legality.class
            {
                return Err(FixedPrecoloredSplitRequirementError::RegisterMismatch {
                    function,
                    register,
                });
            }
            if tied {
                return Err(
                    FixedPrecoloredSplitRequirementError::UnsupportedTiedRegister {
                        function,
                        register,
                    },
                );
            }

            let inputs = topology::tree(function, range, legality)?;
            let mut cuts = CutIndex::new(fixed, &legality.entry_transitions);
            let mut next_segment_id = 0u32;
            let mut fragments = Vec::with_capacity(inputs.len());
            let mut closing_by_block = BTreeMap::<_, BTreeSet<RegisterViewId>>::new();
            for input in &inputs {
                let first_domain = point_domain(function, register, input.spans.first())?;
                let (initial, opening) = match input.incoming {
                    None => (
                        first_domain,
                        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1,
                    ),
                    Some(connector) => {
                        let parent = closing_by_block
                            .get(&connector.source)
                            .expect("tree topology admitted the connector source fragment");
                        let compatible = parent
                            .intersection(&first_domain)
                            .copied()
                            .collect::<BTreeSet<_>>();
                        if compatible.is_empty() {
                            let first = input.spans.first().expect("point domain established");
                            let (site, destination_view) =
                                cuts.boundary(function, register, first)?;
                            cuts.require_transition(
                                function,
                                register,
                                parent,
                                site,
                                destination_view,
                            )?;
                            work.commits(1)?;
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
                                FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 {
                                    connector,
                                },
                            )
                        }
                    }
                };
                let (derived, closing) = fragment(
                    function,
                    register,
                    input.fragment,
                    input.spans,
                    initial,
                    opening,
                    &mut cuts,
                    &mut next_segment_id,
                    work,
                )?;
                closing_by_block.insert(input.fragment.block, closing);
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
            spans: &[VirtualLegalitySpan],
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
            for point in spans {
                let points = u64::from(point.end.0 - point.start.0);
                let candidates = count(point.candidates.len())?
                    .checked_mul(points)
                    .ok_or(WorkDefect::Overflow)?;
                work.candidates(points)?;
                work.validation_steps(points)?;
                work.validation_steps(candidates)?;
                work.iterations(points)?;
                if point.start == source.start {
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
                    point.start,
                    prior.clone(),
                    opening,
                    work,
                )?;
                cuts.use_transition(&prior, site, destination_view);
                work.commits(1)?;
                segment_start = point.start;
                opening =
                    FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
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
            point: Option<&VirtualLegalitySpan>,
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
                FixedPrecoloredSplitRequirementError::SegmentIdentityOverflow {
                    function,
                    register,
                },
            )?;
            segments.push(FixedPrecoloredSourceSegment {
                id: FixedPrecoloredSourceSegmentId(id),
                start,
                end,
                candidates: candidates.into_iter().collect(),
                opening,
            });
            work.commits(1)?;
            Ok(())
        }
    }
    mod topology {

        use std::collections::{BTreeMap, BTreeSet};

        use crate::FixedPrecoloredSplitRequirementError;
        use register_homes::{VirtualLegalitySpan, VirtualRegisterAllocationLegality};
        use selected_instructions::{LiveRangeEdgeConnector, LiveRangeFragment, VirtualLiveRange};

        pub(super) struct FragmentInput<'a> {
            pub(super) fragment: &'a LiveRangeFragment,
            pub(super) spans: &'a [VirtualLegalitySpan],
            pub(super) incoming: Option<LiveRangeEdgeConnector>,
        }

        pub(super) fn tree<'a>(
            function: usize,
            range: &'a VirtualLiveRange,
            legality: &'a VirtualRegisterAllocationLegality,
        ) -> Result<Vec<FragmentInput<'a>>, FixedPrecoloredSplitRequirementError> {
            let register = range.virtual_register.0;
            if range.fragments.is_empty() {
                return Err(
                    FixedPrecoloredSplitRequirementError::MissingSourceFragment {
                        function,
                        register,
                    },
                );
            }
            let mut connectors = BTreeMap::new();
            for connector in &range.edge_connectors {
                if connectors.insert(connector.target, *connector).is_some() {
                    return Err(
                        FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange {
                            function,
                            register,
                        },
                    );
                }
            }

            let mut span_offset = 0usize;
            let mut admitted = BTreeSet::new();
            let mut inputs = Vec::with_capacity(range.fragments.len());
            for fragment in &range.fragments {
                if fragment.start > fragment.end {
                    return Err(FixedPrecoloredSplitRequirementError::IntervalOverflow {
                        function,
                        register,
                    });
                }
                let first_span = span_offset;
                let mut expected = fragment.start;
                while expected < fragment.end {
                    let span = legality
                        .spans
                        .get(span_offset)
                        .filter(|span| {
                            span.block == fragment.block
                                && span.start == expected
                                && span.start < span.end
                                && span.end <= fragment.end
                        })
                        .ok_or(
                            FixedPrecoloredSplitRequirementError::NonCanonicalPointDomain {
                                function,
                                register,
                                point: expected.0,
                            },
                        )?;
                    expected = span.end;
                    span_offset += 1;
                }
                let spans = &legality.spans[first_span..span_offset];
                match connectors.remove(&fragment.block) {
                    Some(connector) if admitted.contains(&connector.source) => {
                        inputs.push(FragmentInput {
                            fragment,
                            spans,
                            incoming: Some(connector),
                        });
                    }
                    None => {
                        inputs.push(FragmentInput {
                            fragment,
                            spans,
                            incoming: None,
                        });
                    }
                    _ => {
                        return Err(
                            FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange {
                                function,
                                register,
                            },
                        );
                    }
                }
                admitted.insert(fragment.block);
            }
            if span_offset != legality.spans.len() {
                return Err(
                    FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange {
                        function,
                        register,
                    },
                );
            }
            Ok(inputs)
        }
    }

    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

    use crate::work_accounting::Work;
    use crate::{
        FixedPrecoloredSplitRequirementError, ValidatedAllocationLegality,
        ValidatedFixedPrecoloredIntervals, ValidatedLiveRanges,
    };
    use register_homes::{
        FixedPrecoloredIntervalPolicy, FixedPrecoloredSplitRequirementPlan,
        FixedPrecoloredSplitRequirementPolicy, FunctionAllocationLegality,
        FunctionFixedPrecoloredIntervals, FunctionFixedPrecoloredSplitRequirements,
    };
    use selected_instructions::FunctionLiveRanges;

    pub fn compute(
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        fixed: &ValidatedFixedPrecoloredIntervals,
        policy: FixedPrecoloredSplitRequirementPolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<FixedPrecoloredSplitRequirementPlan, FixedPrecoloredSplitRequirementError> {
        roots(ranges, legality, fixed)?;
        match policy {
            FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1 => {}
        }
        let mut work = Work::new(OptimizationWorkUsage {
            validation_steps: 1,
            iterations: 2,
            ..Default::default()
        });
        let functions = family(
            &ranges.plan().functions,
            &legality.plan().functions,
            &fixed.plan().functions,
            &mut work,
        )?;
        let usage = work.finish(budget)?;
        Ok(FixedPrecoloredSplitRequirementPlan {
            fixed_intervals: fixed.receipt().identity(),
            ranges: ranges.receipt().identity(),
            legality: legality.receipt().identity(),
            register_environment: legality.receipt().register_environment(),
            allocator_availability: legality.receipt().allocator_availability(),
            optimization_unit: ranges.receipt().optimization_unit(),
            work_schedule: ranges.receipt().work_schedule(),
            target: ranges.plan().target,
            policy,
            budget,
            usage,
            functions,
        })
    }

    fn roots(
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        fixed: &ValidatedFixedPrecoloredIntervals,
    ) -> Result<(), FixedPrecoloredSplitRequirementError> {
        if legality.receipt().ranges() != ranges.receipt().identity()
            || fixed.receipt().ranges() != ranges.receipt().identity()
            || fixed.receipt().legality() != legality.receipt().identity()
            || fixed.receipt().register_environment() != legality.receipt().register_environment()
            || fixed.receipt().allocator_availability()
                != legality.receipt().allocator_availability()
            || fixed.receipt().optimization_unit() != ranges.receipt().optimization_unit()
            || fixed.receipt().work_schedule() != ranges.receipt().work_schedule()
            || fixed.receipt().policy()
                != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
        {
            return Err(FixedPrecoloredSplitRequirementError::RootMismatch);
        }
        Ok(())
    }

    fn family(
        ranges: &[std::sync::Arc<FunctionLiveRanges>],
        legality: &[std::sync::Arc<FunctionAllocationLegality>],
        fixed: &[FunctionFixedPrecoloredIntervals],
        work: &mut Work,
    ) -> Result<Vec<FunctionFixedPrecoloredSplitRequirements>, FixedPrecoloredSplitRequirementError>
    {
        if ranges.len() != legality.len() || ranges.len() != fixed.len() {
            return Err(FixedPrecoloredSplitRequirementError::RootMismatch);
        }
        ranges
            .iter()
            .zip(legality)
            .zip(fixed)
            .enumerate()
            .map(|(function, ((ranges, legality), fixed))| {
                function::derive(function, ranges, legality, fixed, work)
            })
            .collect()
    }
}
mod error {
    use crate::WorkDefect;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum FixedPrecoloredSplitRequirementError {
        RootMismatch,
        FunctionMismatch {
            function: usize,
        },
        RegisterMismatch {
            function: usize,
            register: u32,
        },
        UnsupportedCrossBlockRange {
            function: usize,
            register: u32,
        },
        UnsupportedTiedRegister {
            function: usize,
            register: u32,
        },
        MissingSourceFragment {
            function: usize,
            register: u32,
        },
        NonCanonicalPointDomain {
            function: usize,
            register: u32,
            point: u32,
        },
        UnauthenticatedDomainBreak {
            function: usize,
            register: u32,
            point: u32,
        },
        AmbiguousFixedCutSite {
            function: usize,
            register: u32,
            point: u32,
        },
        UnsupportedFixedTransitionAccess {
            function: usize,
            register: u32,
            instruction: u32,
            operand: u16,
        },
        IntervalOverflow {
            function: usize,
            register: u32,
        },
        SegmentIdentityOverflow {
            function: usize,
            register: u32,
        },
        Work(WorkDefect),
        UsageMismatch,
        NonCanonicalFunctions,
    }

    failed_display!(
        FixedPrecoloredSplitRequirementError,
        "fixed/precolored split-requirement analysis failed"
    );
}

use crate::WorkDefect;
pub use error::FixedPrecoloredSplitRequirementError;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::FixedPrecoloredSplitRequirementPolicy;
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSplitRequirementPlan, FixedPrecoloredSplitRequirementPlanIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::LiveRangeIdentity;
use semantic_vocabulary::WorkScheduleIdentity;
use target::NativeTarget;

pub fn analyze_fixed_precolored_split_requirements(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    fixed: &crate::ValidatedFixedPrecoloredIntervals,
    policy: FixedPrecoloredSplitRequirementPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
    let plan = compute::compute(ranges, legality, fixed, policy, budget)?;
    seal_fixed_precolored_split_requirements(plan)
}

seal_plan!(
    seal_fixed_precolored_split_requirements(FixedPrecoloredSplitRequirementPlan)
        -> ValidatedFixedPrecoloredSplitRequirements: FixedPrecoloredSplitRequirementValidationReceipt
        = register_homes::fixed_precolored_split_requirement_plan_identity,
        FixedPrecoloredSplitRequirementError
    {
        fixed_intervals,
        ranges,
        legality,
        register_environment,
        allocator_availability,
        optimization_unit,
        work_schedule,
        target,
        policy,
        usage
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSplitRequirementValidationReceipt {
    pub(crate) identity: FixedPrecoloredSplitRequirementPlanIdentity,
    pub(crate) fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) work_schedule: WorkScheduleIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) policy: FixedPrecoloredSplitRequirementPolicy,
    pub(crate) usage: OptimizationWorkUsage,
}

impl FixedPrecoloredSplitRequirementValidationReceipt {
    receipt_accessor_rows!(FixedPrecoloredSplitRequirementValidationReceipt {
        identity: FixedPrecoloredSplitRequirementPlanIdentity,
        fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
        target: NativeTarget,
        policy: FixedPrecoloredSplitRequirementPolicy,
        usage: OptimizationWorkUsage,
    });
    shared_analysis_identity_accessors!(pub);
}

validated_plan_carrier!(
    ValidatedFixedPrecoloredSplitRequirements: FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementValidationReceipt
);

pub(super) type SiteKey = (u8, u32, u32, u32, u16, u8);

pub(super) fn site_key(site: selected_instructions::VirtualFixedConstraintSite) -> SiteKey {
    match site {
        selected_instructions::VirtualFixedConstraintSite::Entry => (0, 0, 0, 0, 0, 0),
        selected_instructions::VirtualFixedConstraintSite::Operand {
            position,
            point,
            instruction,
            operand,
            access,
        } => (
            1,
            position.0,
            point.0,
            instruction.0,
            operand,
            access.access_tag(),
        ),
    }
}

pub(super) fn site_point(site: selected_instructions::VirtualFixedConstraintSite) -> u32 {
    match site {
        selected_instructions::VirtualFixedConstraintSite::Entry => 0,
        selected_instructions::VirtualFixedConstraintSite::Operand { point, .. } => point.0,
    }
}

pub(super) fn early_clobber_uses(
    ranges: &selected_instructions::FunctionLiveRanges,
) -> Result<usize, FixedPrecoloredSplitRequirementError> {
    let mut use_count = 0usize;
    for row in &ranges.early_clobbers {
        use_count = use_count
            .checked_add(row.uses.len())
            .ok_or(WorkDefect::Overflow)?;
    }
    Ok(use_count)
}
