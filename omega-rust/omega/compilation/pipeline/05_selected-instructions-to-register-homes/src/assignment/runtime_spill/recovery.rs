//! Executable pressure recovery over a finite roster of original runtime values.
//! Allocation chooses the values; the selected rewrite owner validates semantics.
//!
//! Empty interval-home intersections are also recoverable: a scalar carried
//! across a call can require callee-saved views there and a fixed accumulator
//! at a later division. No single home fits that interval, but the existing
//! admitted split or spill can realize its separate segments. Home assignment
//! remains strict after every rewrite; this classification grants no view or
//! storage authority and does not recover malformed classes or tied topology.
//!
//! Each step takes the cheapest admissible decision. A live-range split at a
//! fixed-view definition comes first: one copy right after the pinned write
//! frees the pinned home for the rest of the value's range and realizes no
//! storage. Rematerialization of a pure immediate comes next, and private
//! storage is the fallback. The split is searched over every overlapping
//! candidate, not only the named one, because the value that must move is
//! usually the pinned neighbour occupying the home the failed register
//! needs — the named register itself is typically the one with the fixed
//! use and cannot give that home up.
//!
//! Each step's rewrite and analyses are checked by their own owners as they
//! are produced; the completed recovery is not replayed afterwards
//! (verification placement in `omega-rust/pipeline.md`).

use super::{
    RuntimeSpillAllocation, RuntimeSpillAllocationError, RuntimeSpillFacts,
    RuntimeSpillManifestBase, RuntimeSpillSource, RuntimeSpillStep, RuntimeSpillStepRewrite,
};
use crate::assignment::home_assignment::compute::FunctionHomeReuse;
use optimization_core::OptimizationWorkBudget;
use register_homes::PostAllocationSelectedTransformation;
use selected_instructions::{
    FunctionLiveRanges, SelectedInstructionPlan, VirtualRegisterId, VirtualRegisterOrigin,
};
use selected_instructions_to_selected_instructions::{
    FixedViewCopyPolicy, RuntimeSpillError, RuntimeSpillSpanPolicy, SelectedProgramRef,
    StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationRun,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedRuntimeSpill, ValidatedRuntimeSplit, ValidatedSelectedAnalysis,
    analyze_allocation_legality_reusing, analyze_live_ranges_reusing, analyze_liveness_reusing,
    materialize_selected_fixed_occurrences, rematerialize_selected_runtime_value,
    spill_selected_runtime_value, spill_selected_runtime_value_with_span_policy,
    split_selected_runtime_value,
};

pub(crate) fn assign_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    assign(
        source.register_environment(),
        source.ranges(),
        source.legality(),
    )
}

pub(super) fn assign(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    assign_reusing(environment, ranges, legality, None)
}

/// `assign` offering the homes of functions an earlier recovery step left
/// unchanged (`FunctionHomeReuse`).
fn assign_reusing(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    reuse: Option<&mut FunctionHomeReuse>,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    crate::assignment::home_assignment::assign_register_homes_reusing(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        reuse,
    )
}

pub(super) fn analyze(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    availability: &ValidatedAllocatorAvailability,
    previous: &impl ValidatedSelectedAnalysis,
    previous_liveness: &ValidatedLiveness,
    previous_ranges: &ValidatedLiveRanges,
    previous_legality: &ValidatedAllocationLegality,
    selected: &impl ValidatedSelectedAnalysis,
) -> Result<RuntimeSpillFacts, RuntimeSpillAllocationError> {
    let liveness = analyze_liveness_reusing(previous, previous_liveness, selected)
        .map_err(RuntimeSpillAllocationError::Liveness)?;
    let ranges = analyze_live_ranges_reusing(
        previous,
        previous_liveness,
        previous_ranges,
        selected,
        &liveness,
    )
    .map_err(RuntimeSpillAllocationError::Ranges)?;
    let legality = analyze_allocation_legality_reusing(
        previous_ranges,
        previous_legality,
        &ranges,
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(RuntimeSpillAllocationError::Legality)?;
    Ok(RuntimeSpillFacts {
        liveness,
        ranges,
        legality,
    })
}

/// A failed crossing probe belongs only to the current victim attempt. The
/// bounded policy must still construct and independently validate its rewrite;
/// equal complete plans and receipts then mean allocation has exactly the same
/// inputs under this attempt's unchanged environment and availability. Keep the
/// established facts and pressure instead of analyzing and assigning them twice.
/// This is producer work reuse, never evidence for final independent replay.
struct SpillPressureProbe {
    rewrite: ValidatedRuntimeSpill,
    facts: RuntimeSpillFacts,
    failure: crate::RegisterHomeError,
}

impl SpillPressureProbe {
    fn reuse_for(
        self,
        bounded: &ValidatedRuntimeSpill,
    ) -> Option<(RuntimeSpillFacts, crate::RegisterHomeError)> {
        (self.rewrite.receipt() == bounded.receipt()
            && self.rewrite.transformed() == bounded.transformed())
        .then_some((self.facts, self.failure))
    }
}

// Keep a finite roster from the current program. A block parameter can use
// private edge-initialized storage only when the selected rewrite checks every
// incoming path, and an entry-bound register — a scalar or structural
// parameter, or the hidden aggregate-result destination — only while the
// entry boundary itself stores it; presence here grants neither that storage
// nor a home. Instruction-defined structural registers — field observations
// and transport pointers — join by origin so a pressured structural plan can
// spill them like any other value; admission still rejects the ones defined
// by address-producing instructions. A committed rewrite can mint registers
// — a split's fresh register keeps any pinned use over its own span — so the
// pool re-derives against each step's plan. Reloads from either recovery form
// restate the victim's own origin, so `retire_minted` keeps them out. Only the
// failed function's registers can relieve its pressure and every step rewrites
// only that function, so the roster reads that one function's registers rather
// than every register of the program at each step.
pub(super) fn candidates(
    plan: &SelectedInstructionPlan,
    function: usize,
) -> Vec<(usize, VirtualRegisterId)> {
    plan.functions
        .get(function)
        .into_iter()
        .flat_map(|body| {
            body.virtual_registers
                .iter()
                .filter(|register| {
                    matches!(
                        register.origin,
                        VirtualRegisterOrigin::InstructionResult { .. }
                            | VirtualRegisterOrigin::BlockParameter { .. }
                            | VirtualRegisterOrigin::StructuralObservation { .. }
                            | VirtualRegisterOrigin::AbiTransport { .. }
                    ) || body.is_entry_register(register)
                })
                .map(move |register| (function, register.id))
        })
        .collect()
}

/// Retire every candidate a spill or rematerialization step minted. Their
/// reloads and carriers restate the victim's origin — an instruction result
/// or a structural observation — so `candidates` would readmit them, and
/// each spill would mint the next victim: one pressured value then chains
/// through a fresh reload per step, each step a whole-function analysis.
/// A split's fresh register is the only minted register that joins the pool.
/// The step rewrote `function` alone, so only its registers can be new.
pub(super) fn retire_minted(
    retired: &mut std::collections::BTreeSet<(usize, VirtualRegisterId)>,
    function: usize,
    before: &SelectedInstructionPlan,
    after: &SelectedInstructionPlan,
) {
    let existing: std::collections::BTreeSet<_> =
        candidates(before, function).into_iter().collect();
    retired.extend(
        candidates(after, function)
            .into_iter()
            .filter(|entry| !existing.contains(entry)),
    );
}

/// Every recorded transformation this recovery produced, appended after the
/// source's own prefix: a fixed-view entry keeps its copy transformation first.
pub(super) fn transformations(
    prefix: &[PostAllocationSelectedTransformation],
    recorded: &[PostAllocationSelectedTransformation],
) -> Vec<PostAllocationSelectedTransformation> {
    prefix.iter().chain(recorded).cloned().collect()
}

/// The pressure domain one recovery step must relieve: the function and
/// leader a placement failure names, with the leader's member fragments
/// collected once. Every roster candidate is scored against this one
/// reconstruction; rebuilding the members per candidate made each step
/// quadratic in the failed function's ties and transfers.
pub(super) struct FailedDomain<'ranges> {
    function: usize,
    ranges: &'ranges FunctionLiveRanges,
    members: Vec<VirtualRegisterId>,
    components: crate::assignment::same_home::SameHomeComponents,
    pressure: Vec<(VirtualRegisterId, usize)>,
    intrinsic: bool,
}

impl<'ranges> FailedDomain<'ranges> {
    /// A placement-pressure component or a value-local empty common domain;
    /// unrelated failures never enter recovery.
    pub(super) fn of(
        failure: &crate::RegisterHomeError,
        ranges: &'ranges ValidatedLiveRanges,
    ) -> Option<Self> {
        let (function, register, intrinsic) = match failure {
            crate::RegisterHomeError::NoCompatibleHome { function, register } => {
                (*function, *register, false)
            }
            crate::RegisterHomeError::NoCommonCandidate { function, register }
            | crate::RegisterHomeError::UnresolvedEntryTransitions {
                function, register, ..
            } => (*function, *register, true),
            _ => return None,
        };
        let function_ranges = ranges.plan().functions.get(function)?;
        let mut domain = Self::in_function(function, function_ranges, VirtualRegisterId(register));
        if intrinsic {
            // Empty point-domain intersections and retained entry transitions
            // belong to this exact source value. Spilling neighbours cannot
            // change its pinned uses, clobbers or preservation obligation.
            domain.members = vec![VirtualRegisterId(register)];
            domain.intrinsic = true;
        }
        Some(domain)
    }

    pub(super) fn in_function(
        function: usize,
        ranges: &'ranges FunctionLiveRanges,
        leader: VirtualRegisterId,
    ) -> Self {
        let components = crate::assignment::same_home::SameHomeComponents::from_ranges(ranges);
        let members = components.members(leader);
        let mut covered = Vec::new();
        for pair in &ranges.interference {
            for (member, other) in [(pair.lower, pair.higher), (pair.higher, pair.lower)] {
                if members.binary_search(&member).is_ok() {
                    covered.push((components.leader(other), member));
                }
            }
        }
        // Multiple interfering fragments of one candidate component cover a
        // failed member only once. Keep this sparse relation in sorted storage.
        covered.sort_unstable();
        covered.dedup();
        let mut pressure: Vec<(VirtualRegisterId, usize)> = Vec::new();
        for (component, _) in covered {
            if let Some((previous, count)) = pressure.last_mut()
                && *previous == component
            {
                *count += 1;
            } else {
                pressure.push((component, 1));
            }
        }
        Self {
            function,
            ranges,
            members,
            components,
            pressure,
            intrinsic: false,
        }
    }

    /// Score a candidate's whole shared-home component against the failed
    /// component. Each failed member counts once even if several candidate
    /// fragments interfere with it. A failed member itself can dissolve its
    /// own share. Intrinsic value-local failures never admit neighbours.
    pub(super) fn relief(&self, function: usize, register: VirtualRegisterId) -> usize {
        if function != self.function {
            return 0;
        }
        if self.members.binary_search(&register).is_ok() {
            return 1;
        }
        if self.intrinsic {
            return 0;
        }
        self.pressure
            .binary_search_by_key(&self.components.leader(register), |(component, _)| {
                *component
            })
            .map_or(0, |position| self.pressure[position].1)
    }

    pub(super) fn function(&self) -> usize {
        self.function
    }

    // Prefer directly occupied fragments before their remote shared-home
    // peers. Both are eligible, but moving the directly live value removes
    // pressure here without depending on how much of its peer's range survives.
    fn direct_relief(&self, register: VirtualRegisterId) -> usize {
        self.members
            .iter()
            .filter(|member| {
                **member == register || {
                    let (lower, higher) = if register < **member {
                        (register, **member)
                    } else {
                        (**member, register)
                    };
                    self.ranges
                        .interference
                        .binary_search(&selected_instructions::VirtualInterference {
                            lower,
                            higher,
                        })
                        .is_ok()
                }
            })
            .count()
    }

    fn spill_priority(&self, function: usize, register: VirtualRegisterId) -> (usize, u64) {
        let points = self
            .ranges
            .virtual_registers
            .iter()
            .find(|range| range.virtual_register == register)
            .map_or(0, |range| {
                range
                    .fragments
                    .iter()
                    .map(|fragment| u64::from(fragment.end.0 - fragment.start.0))
                    .sum()
            });
        (
            if function == self.function {
                self.direct_relief(register)
            } else {
                0
            },
            points,
        )
    }
}

// The failed identity is a domain leader, not a preferred spill payload.
// Among eligible components, relieve the most directly covered fragments first,
// then the greatest actual live extent (not the convex whole-function hull).
// Long-lived blockers otherwise force recovery through many short failed
// leaders and a whole-function analysis per victim. Equal scores retain roster
// order. Intrinsic constraints still admit only the exact failed value.
fn candidate_position<Priority: Ord + Copy>(
    failure: &crate::RegisterHomeError,
    roster: &[(usize, VirtualRegisterId)],
    mut overlaps: impl FnMut(&(usize, VirtualRegisterId)) -> bool,
    relief: impl Fn(&(usize, VirtualRegisterId)) -> Priority,
) -> Option<usize> {
    match failure {
        crate::RegisterHomeError::NoCompatibleHome { .. } => {}
        crate::RegisterHomeError::NoCommonCandidate { function, register }
        | crate::RegisterHomeError::UnresolvedEntryTransitions {
            function, register, ..
        } => {
            return roster
                .iter()
                .position(|candidate| *candidate == (*function, VirtualRegisterId(*register)));
        }
        _ => return None,
    };
    roster
        .iter()
        .enumerate()
        .filter(|(_, candidate)| overlaps(candidate))
        .map(|(position, candidate)| (position, relief(candidate)))
        .min_by_key(|(position, relief)| (std::cmp::Reverse(*relief), *position))
        .map(|(position, _)| position)
}

/// The split decision at one step: among the still-available roster
/// candidates overlapping the failed domain, in most-relief-then-roster
/// order, the first whose fixed-view definition
/// the split rewrite admits. `None` means no split is admissible and the step
/// falls through to rematerialization or storage.
#[allow(clippy::too_many_arguments)]
pub(super) fn admissible_split<'roster>(
    domain: Option<&FailedDomain<'_>>,
    available: impl Iterator<Item = &'roster (usize, VirtualRegisterId)>,
    selected: &impl ValidatedSelectedAnalysis,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Option<((usize, VirtualRegisterId), ValidatedRuntimeSplit)>, RuntimeSpillAllocationError>
{
    let Some(domain) = domain else {
        return Ok(None);
    };
    let mut ordered: Vec<(usize, (usize, VirtualRegisterId))> = available
        .copied()
        .filter_map(|candidate| {
            let relief = domain.relief(candidate.0, candidate.1);
            (relief > 0).then_some((domain.direct_relief(candidate.1), candidate))
        })
        .collect();
    // A stable sort keeps roster order among equal relief.
    ordered.sort_by_key(|(relief, _)| std::cmp::Reverse(*relief));
    for (_, (function, register)) in ordered {
        match split_selected_runtime_value(selected, function, register, environment, budget) {
            Ok(split) => return Ok(Some(((function, register), split))),
            Err(error) if error.is_inadmissible() => {}
            Err(error) => return Err(RuntimeSpillAllocationError::Split(error)),
        }
    }
    Ok(None)
}

pub(crate) fn recover(
    source: StagedOptimizedAllocationLegality,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::Legality(source))
}

/// A fixed-view sequence that exhausted its own assignment hands custody here:
/// the copy transformation's complete reanalysis becomes the recovery source,
/// and the produced manifest keeps the copy step ahead of every spill step.
pub(crate) fn recover_after_fixed_view_copies(
    reanalysis: StagedOptimizedSelectedReanalysis,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::FixedViewCopies(reanalysis))
}

/// A declared fixed-view sequence whose segment-home probe reported a
/// capacity decline hands custody here before the sequence consumes it:
/// recovery runs over the original legality, and the declined policy stays
/// recorded so the produced allocation still binds the declared selection.
pub(crate) fn recover_after_declined_fixed_view_probe(
    legality: StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::DeclinedFixedView { legality, policy })
}

/// An active-resident rematerialization sweep whose rebuilt facts still
/// report `NoCompatibleHome` hands custody here: recovery runs over the
/// rematerialized program and its rebuilt analyses, the recorded sweep stays
/// the first transformation in the produced manifest.
pub(crate) fn recover_after_active_resident_rematerialization(
    pressure: crate::StagedOptimizedActiveResidentRematerializationPressure,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::ActiveResidentRematerialization(
        pressure,
    ))
}

/// A completed selected-lowering run whose final facts still report
/// `NoCompatibleHome` hands custody here: recovery runs over the run's
/// current program and rebuilt analyses, the committed fold ledger and the
/// suite completion stay recorded ahead of every spill step.
pub(crate) fn recover_after_selected_lowering(
    run: StagedSelectedLoweringOptimizationRun,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::SelectedLowering(run))
}

/// A completed pre-allocation run whose final facts still report
/// `NoCompatibleHome` hands custody here the same way: recovery runs over
/// the run's current program and rebuilt analyses, the committed
/// transformation ledger and the suite completion stay recorded ahead of
/// every spill step.
pub(crate) fn recover_after_pre_allocation(
    run: StagedPreAllocationOptimizationRun,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::PreAllocation(run))
}

/// Both exit points declare the manifest over the same retained prefix.
fn project_recovery_manifest(
    base: &RuntimeSpillManifestBase,
    recorded: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &crate::ValidatedRegisterHomes,
) -> Result<crate::ValidatedPostAllocationOptimizationManifest, RuntimeSpillAllocationError> {
    crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest_with_suite_completions(
        base.pre_physical,
        base.selected_lowering_completion,
        base.pre_allocation_completion,
        &transformations(&base.prefix, recorded),
        ranges,
        legality,
        homes,
    )
    .map_err(RuntimeSpillAllocationError::Manifest)
}

fn recover_over(
    source: RuntimeSpillSource,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    let base = source.declared_manifest();
    let environment = source.register_environment();
    // One reuse set spans every step: each step rewrites one function, so the
    // others keep the homes their unchanged rows already received.
    let mut home_reuse = FunctionHomeReuse::default();
    let budget = source.budget_per_pass();
    let rewrite = RuntimeSpillStepRewrite::FixedOccurrences(
        materialize_selected_fixed_occurrences(&source.base(), environment, budget)
            .map_err(RuntimeSpillAllocationError::FixedOccurrences)?,
    );
    let facts = analyze(
        environment,
        source.allocator_availability(),
        &source.base(),
        source.liveness(),
        source.ranges(),
        source.legality(),
        &rewrite.selected(),
    )?;
    let mut recorded = vec![rewrite.transformation()];
    let mut failure = match assign_reusing(
        environment,
        &facts.ranges,
        &facts.legality,
        Some(&mut home_reuse),
    ) {
        Ok(homes) => {
            let manifest = project_recovery_manifest(
                &base,
                &recorded,
                &facts.ranges,
                &facts.legality,
                &homes,
            )?;
            return Ok(RuntimeSpillAllocation {
                source,
                last: RuntimeSpillStep { rewrite },
                facts,
                homes,
                manifest,
            });
        }
        Err(
            error @ (crate::RegisterHomeError::NoCompatibleHome { .. }
            | crate::RegisterHomeError::NoCommonCandidate { .. }
            | crate::RegisterHomeError::UnresolvedEntryTransitions { .. }),
        ) => error,
        Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
    };
    let mut current = Some(rewrite);
    let mut tried: std::collections::BTreeSet<(usize, VirtualRegisterId)> =
        std::collections::BTreeSet::new();
    let mut current_ranges = facts.ranges;
    let mut current_liveness = facts.liveness;
    let mut current_legality = facts.legality;
    loop {
        let selected = current
            .as_ref()
            .map_or_else(|| source.base(), RuntimeSpillStepRewrite::selected);
        // Committed rewrites mint registers the original plan never had: a
        // split's fresh register inherits every later reference, fixed views
        // included, so it can hold the surviving pin the failed domain
        // overlaps. Re-derive the pool from the current plan minus every
        // consumed victim, or that fresh interval stays unreachable and the
        // same failure rotates through the remaining members forever.
        // Reloads minted by spill and rematerialization retire as they are
        // minted and never join.
        // Only the failed function's registers can relieve its pressure, so
        // the roster and every score below stay within that function.
        let domain = FailedDomain::of(&failure, &current_ranges);
        let Some(pressured) = domain.as_ref().map(FailedDomain::function).or_else(|| {
            if let crate::RegisterHomeError::NoCommonCandidate { function, .. } = &failure {
                Some(*function)
            } else {
                None
            }
        }) else {
            break;
        };
        let mut roster: Vec<(usize, VirtualRegisterId)> = candidates(selected.plan(), pressured)
            .into_iter()
            .filter(|entry| !tried.contains(entry))
            .collect();
        // The split decision comes first: one copy after a pinned definition
        // is cheaper than any regeneration or storage, and it is committed
        // whenever admitted. It replaces one live range by two disjoint ones
        // joined at the copy, so no point gains a live value; the pinned
        // segment the failed domain overlapped now ends at the copy.
        let split = admissible_split(
            domain.as_ref(),
            roster.iter(),
            &selected,
            environment,
            budget,
        )?;
        // Preserve the already-admitted after-definition split. The narrower
        // integer fixed-use copy is an optional refinement for a value-local
        // empty domain; unsupported shapes keep the ordinary rematerialize/spill
        // route, including Boolean and floating-point values admitted there.
        let before_use = if split.is_none() {
            if let crate::RegisterHomeError::NoCommonCandidate { function, register } = &failure {
                match ValidatedRuntimeSplit::before_fixed_use(
                    &selected,
                    *function,
                    VirtualRegisterId(*register),
                    environment,
                    budget,
                ) {
                    Ok(split) => Some(split),
                    Err(error) if error.is_inadmissible() => None,
                    Err(error) => return Err(RuntimeSpillAllocationError::Split(error)),
                }
            } else {
                None
            }
        } else {
            None
        };
        let used_before_use = before_use.is_some();
        let mut failed_crossing = None;
        let rewrite = if let Some(split) = before_use {
            RuntimeSpillStepRewrite::Split(split)
        } else if let Some((candidate, split)) = split {
            tried.insert(candidate);
            RuntimeSpillStepRewrite::Split(split)
        } else {
            let Some(position) = candidate_position(
                &failure,
                &roster,
                |(function, register)| {
                    domain
                        .as_ref()
                        .is_some_and(|domain| domain.relief(*function, *register) > 0)
                },
                |(function, register)| {
                    domain
                        .as_ref()
                        .map_or((0, 0), |domain| domain.spill_priority(*function, *register))
                },
            ) else {
                break;
            };
            let (function, register) = roster.remove(position);
            tried.insert((function, register));
            // The cost decision: regenerating one pure immediate materialization
            // per use is strictly cheaper than private storage plus reload pairs,
            // so rematerialization is attempted first. Its admission failure does
            // not commit the step; spilling the same victim remains the fallback.
            let rewrite = match rematerialize_selected_runtime_value(
                &selected,
                function,
                register,
                environment,
                budget,
            ) {
                Ok(rewrite) => RuntimeSpillStepRewrite::Rematerialization(rewrite),
                Err(_) => {
                    // The call-surviving shape first: where a clobber-set-reduced
                    // view of the victim's class survives every unit written
                    // inside a flexible-use span, the still-open reload reaches
                    // across the call — or any other unit-writing instruction —
                    // and the allocator proves whether that surviving, most often
                    // callee-saved, home is actually free. Only an assignment
                    // that succeeds commits the step, ending recovery one reload
                    // pair earlier; anything else falls back to the bounded
                    // shape, so a committed step never strands an unhomeable
                    // interval. Each candidate's fresh analyses and final
                    // home assignment establish whether it can commit.
                    if let Ok(crossing) = spill_selected_runtime_value_with_span_policy(
                        &selected,
                        function,
                        register,
                        environment,
                        budget,
                        RuntimeSpillSpanPolicy::UnitWriteCrossing,
                    ) {
                        let probe = analyze(
                            environment,
                            source.allocator_availability(),
                            &selected,
                            &current_liveness,
                            &current_ranges,
                            &current_legality,
                            &SelectedProgramRef::new(&crossing),
                        )?;
                        match assign_reusing(
                            environment,
                            &probe.ranges,
                            &probe.legality,
                            Some(&mut home_reuse),
                        ) {
                            Ok(homes) => {
                                let rewrite = RuntimeSpillStepRewrite::Spill(crossing);
                                recorded.push(rewrite.transformation());
                                let manifest = project_recovery_manifest(
                                    &base,
                                    &recorded,
                                    &probe.ranges,
                                    &probe.legality,
                                    &homes,
                                )?;
                                let result = RuntimeSpillAllocation {
                                    source,
                                    last: RuntimeSpillStep { rewrite },
                                    facts: probe,
                                    homes,
                                    manifest,
                                };
                                return Ok(result);
                            }
                            Err(
                                error @ (crate::RegisterHomeError::NoCompatibleHome { .. }
                                | crate::RegisterHomeError::NoCommonCandidate { .. }
                                | crate::RegisterHomeError::UnresolvedEntryTransitions {
                                    ..
                                }),
                            ) => {
                                failed_crossing = Some(SpillPressureProbe {
                                    rewrite: crossing,
                                    facts: probe,
                                    failure: error,
                                });
                            }
                            Err(error) => {
                                return Err(RuntimeSpillAllocationError::Homes(error));
                            }
                        }
                    }
                    match spill_selected_runtime_value(
                        &selected,
                        function,
                        register,
                        environment,
                        budget,
                    ) {
                        Ok(rewrite) => RuntimeSpillStepRewrite::Spill(rewrite),
                        Err(error) if inadmissible(&error) => continue,
                        Err(error) => return Err(RuntimeSpillAllocationError::Rewrite(error)),
                    }
                }
            };
            rewrite
        };
        let reused_pressure = match &rewrite {
            RuntimeSpillStepRewrite::Spill(bounded) => {
                failed_crossing.and_then(|probe| probe.reuse_for(bounded))
            }
            _ => None,
        };
        let (facts, homes) = if let Some((facts, pressure)) = reused_pressure {
            (facts, Err(pressure))
        } else {
            let facts = analyze(
                environment,
                source.allocator_availability(),
                &selected,
                &current_liveness,
                &current_ranges,
                &current_legality,
                &rewrite.selected(),
            )?;
            let homes = assign_reusing(
                environment,
                &facts.ranges,
                &facts.legality,
                Some(&mut home_reuse),
            );
            (facts, homes)
        };
        if used_before_use || !matches!(rewrite, RuntimeSpillStepRewrite::Split(_)) {
            retire_minted(
                &mut tried,
                pressured,
                selected.plan(),
                rewrite.selected().plan(),
            );
        }
        recorded.push(rewrite.transformation());
        match homes {
            Ok(homes) => {
                let manifest = project_recovery_manifest(
                    &base,
                    &recorded,
                    &facts.ranges,
                    &facts.legality,
                    &homes,
                )?;
                let result = RuntimeSpillAllocation {
                    source,
                    last: RuntimeSpillStep { rewrite },
                    facts,
                    homes,
                    manifest,
                };
                return Ok(result);
            }
            Err(
                error @ (crate::RegisterHomeError::NoCompatibleHome { .. }
                | crate::RegisterHomeError::NoCommonCandidate { .. }
                | crate::RegisterHomeError::UnresolvedEntryTransitions { .. }),
            ) => {
                failure = error;
                current_ranges = facts.ranges;
                current_legality = facts.legality;
                current_liveness = facts.liveness;
                current = Some(rewrite);
            }
            Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
        }
    }
    Err(RuntimeSpillAllocationError::Homes(failure))
}

#[cfg(test)]
mod candidate_tests;

/// A spill the rewrite owner declines for this value's shape, use or control
/// flow: recovery moves on to the next candidate instead of failing.
fn inadmissible(error: &RuntimeSpillError) -> bool {
    matches!(
        error,
        RuntimeSpillError::UnsupportedValue
            | RuntimeSpillError::UnsupportedUse
            | RuntimeSpillError::UnsupportedControlFlow
    )
}
