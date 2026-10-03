//! Executable pressure recovery over a finite roster of original runtime values.
//! Allocation chooses the values; the selected rewrite owner validates semantics.
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
//! Success returns an unadmitted candidate. Full-result replay belongs to the
//! retained allocation boundary, which every caller enters before publication.
//! Keep the per-step semantic checks here, but do not replay the completed
//! history once here and then again at that boundary.

use super::{
    RuntimeSpillAllocation, RuntimeSpillAllocationError, RuntimeSpillFacts,
    RuntimeSpillManifestBase, RuntimeSpillSource, RuntimeSpillStep, RuntimeSpillStepRewrite,
    replay,
};
use optimization_core::OptimizationWorkBudget;
use register_homes::PostAllocationSelectedTransformation;
use selected_instructions::{
    FunctionLiveRanges, SelectedInstructionPlan, VirtualInterference, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use selected_instructions_to_selected_instructions::{
    FixedPrecoloredSegmentHomeDecline, FixedViewCopyPolicy, RuntimeSpillSpanPolicy,
    SelectedProgramRef, StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationRun,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedRuntimeSpill, ValidatedRuntimeSplit, ValidatedSelectedAnalysis,
    analyze_allocation_legality_reusing, analyze_live_ranges_reusing, analyze_liveness_reusing,
    rematerialize_selected_runtime_value, spill_selected_runtime_value,
    spill_selected_runtime_value_with_span_policy, split_selected_runtime_value,
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
    crate::assign_register_homes(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
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
// restate the victim's own origin, so `retire_minted` keeps them out.
pub(super) fn candidates(plan: &SelectedInstructionPlan) -> Vec<(usize, VirtualRegisterId)> {
    plan.functions
        .iter()
        .enumerate()
        .flat_map(|(function, body)| {
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
pub(super) fn retire_minted(
    retired: &mut std::collections::BTreeSet<(usize, VirtualRegisterId)>,
    before: &SelectedInstructionPlan,
    after: &SelectedInstructionPlan,
) {
    let existing: std::collections::BTreeSet<_> = candidates(before).into_iter().collect();
    retired.extend(
        candidates(after)
            .into_iter()
            .filter(|entry| !existing.contains(entry)),
    );
}

/// Every recorded transformation this recovery produced, appended after the
/// source's own prefix: a fixed-view entry keeps its copy transformation first.
pub(super) fn transformations(
    prefix: &[PostAllocationSelectedTransformation],
    steps: &[RuntimeSpillStep],
) -> Vec<PostAllocationSelectedTransformation> {
    prefix
        .iter()
        .cloned()
        .chain(steps.iter().map(|step| match &step.rewrite {
            RuntimeSpillStepRewrite::Split(rewrite) => {
                PostAllocationSelectedTransformation::RuntimeSplit(
                    rewrite.receipt().transformed_selected(),
                )
            }
            RuntimeSpillStepRewrite::Spill(rewrite) => {
                PostAllocationSelectedTransformation::RuntimeSpill(
                    rewrite.receipt().transformed_selected(),
                )
            }
            RuntimeSpillStepRewrite::Rematerialization(rewrite) => {
                PostAllocationSelectedTransformation::RuntimeRematerialization(
                    rewrite.receipt().transformed_selected(),
                )
            }
        }))
        .collect()
}

pub(super) fn overlaps_pressure(
    failure: &crate::RegisterHomeError,
    ranges: &ValidatedLiveRanges,
    function: usize,
    register: VirtualRegisterId,
) -> bool {
    let crate::RegisterHomeError::NoCompatibleHome {
        function: failed_function,
        register: failed_register,
    } = failure
    else {
        return false;
    };
    let Some(function_ranges) = ranges.plan().functions.get(*failed_function) else {
        return false;
    };
    function == *failed_function
        && split_domain_pressure(
            function_ranges,
            VirtualRegisterId(*failed_register),
            register,
        )
}

// A pressure failure names the leader of one tied/edge-transferred domain: a
// single live range split into disjoint member fragments at the tie and
// transfer points, all sharing one home. Member fragments never interfere
// with each other, so the domain's pressure is the union of every member's
// interference. Coalescing that evidence across the split points admits a
// victim overlapping any member fragment — or a member fragment itself, whose
// own split breaks the shared-home requirement.
fn split_domain_pressure(
    ranges: &FunctionLiveRanges,
    leader: VirtualRegisterId,
    register: VirtualRegisterId,
) -> bool {
    let members = split_domain_members(ranges, leader);
    members.contains(&register)
        || members
            .iter()
            .any(|member| interferes(ranges, register, *member))
}

// Collect the failed domain's member fragments: the connected component of
// use/def ties and edge transfers rooted at the named leader. Those are the
// same relations domain construction unions, so this reproduces the split
// range's membership without rebuilding allocation domains.
fn split_domain_members(
    ranges: &FunctionLiveRanges,
    leader: VirtualRegisterId,
) -> std::collections::BTreeSet<VirtualRegisterId> {
    let mut members = std::collections::BTreeSet::from([leader]);
    let mut frontier = vec![leader];
    while let Some(member) = frontier.pop() {
        for tie in &ranges.tied_pairs {
            let other = if tie.use_virtual_register == member {
                tie.def_virtual_register
            } else if tie.def_virtual_register == member {
                tie.use_virtual_register
            } else {
                continue;
            };
            if members.insert(other) {
                frontier.push(other);
            }
        }
        for transfer in &ranges.edge_transfers {
            let other = if transfer.argument == member {
                transfer.parameter
            } else if transfer.parameter == member {
                transfer.argument
            } else {
                continue;
            };
            if members.insert(other) {
                frontier.push(other);
            }
        }
    }
    members
}

fn interferes(
    ranges: &FunctionLiveRanges,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
) -> bool {
    let (lower, higher) = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    ranges
        .interference
        .binary_search(&VirtualInterference { lower, higher })
        .is_ok()
}

// The failed id is a tied-domain leader, not necessarily an admissible payload.
// Prefer it only while it remains in the original roster; ordinary admission
// still rejects terminal/edge uses, and removal restores the same fallback order.
// Among the remaining overlapping candidates the choice coalesces the same
// split-domain evidence the pressure predicate widened: the victim covering
// the most member fragments of the failed domain — a member fragment covering
// itself, an outsider covering every member it interferes with — relieves the
// most of the failed requirement per step. Equal relief keeps roster order.
fn candidate_position(
    failure: &crate::RegisterHomeError,
    roster: &[(usize, VirtualRegisterId)],
    mut overlaps: impl FnMut(&(usize, VirtualRegisterId)) -> bool,
    relief: impl Fn(&(usize, VirtualRegisterId)) -> usize,
) -> Option<usize> {
    let crate::RegisterHomeError::NoCompatibleHome { function, register } = failure else {
        return None;
    };
    roster
        .iter()
        .position(|candidate| *candidate == (*function, VirtualRegisterId(*register)))
        .or_else(|| {
            roster
                .iter()
                .enumerate()
                .filter(|(_, candidate)| overlaps(candidate))
                .map(|(position, candidate)| (position, relief(candidate)))
                .min_by_key(|(position, relief)| (std::cmp::Reverse(*relief), *position))
                .map(|(position, _)| position)
        })
}

// One candidate's relief against the failed domain: the member fragments it
// covers across the split points, counting membership itself — a member's own
// split dissolves its share of the shared-home requirement — plus every other
// member it interferes with. Zero means the candidate is not a pressure victim
// at all, so callers keep the predicate and the score from one reconstruction.
fn split_domain_relief(
    failure: &crate::RegisterHomeError,
    ranges: &ValidatedLiveRanges,
    function: usize,
    register: VirtualRegisterId,
) -> usize {
    let crate::RegisterHomeError::NoCompatibleHome {
        function: failed_function,
        register: failed_register,
    } = failure
    else {
        return 0;
    };
    let Some(function_ranges) = ranges.plan().functions.get(*failed_function) else {
        return 0;
    };
    if function != *failed_function {
        return 0;
    }
    member_relief(
        function_ranges,
        VirtualRegisterId(*failed_register),
        register,
    )
}

fn member_relief(
    ranges: &FunctionLiveRanges,
    failed: VirtualRegisterId,
    register: VirtualRegisterId,
) -> usize {
    split_domain_members(ranges, failed)
        .iter()
        .copied()
        .filter(|member| *member == register || interferes(ranges, register, *member))
        .count()
}

/// The split decision both the producer and retained replay take at one
/// step: among the still-available roster candidates overlapping the failed
/// domain, in the same most-relief-then-roster order the spill choice uses,
/// the first whose fixed-view definition the split rewrite admits. `None`
/// means no split is admissible and the step falls through to
/// rematerialization or storage. Sharing this one policy is what lets replay
/// reject a recorded storage step that a cheaper split would have replaced.
#[allow(clippy::too_many_arguments)]
pub(super) fn admissible_split<'roster>(
    failure: &crate::RegisterHomeError,
    available: impl Iterator<Item = &'roster (usize, VirtualRegisterId)>,
    ranges: &ValidatedLiveRanges,
    selected: &impl ValidatedSelectedAnalysis,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Option<((usize, VirtualRegisterId), ValidatedRuntimeSplit)>, RuntimeSpillAllocationError>
{
    let mut ordered: Vec<(usize, (usize, VirtualRegisterId))> = available
        .copied()
        .filter(|(function, register)| overlaps_pressure(failure, ranges, *function, *register))
        .map(|candidate| {
            (
                split_domain_relief(failure, ranges, candidate.0, candidate.1),
                candidate,
            )
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
/// recovery runs over the original legality, and the declined policy and
/// verdict stay recorded so retained replay re-proves the probe outcome and
/// still binds the declared selection.
pub(crate) fn recover_after_declined_fixed_view_probe(
    legality: StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
    decline: FixedPrecoloredSegmentHomeDecline,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::DeclinedFixedView {
        legality,
        policy,
        decline,
    })
}

/// An active-resident rematerialization sweep whose rebuilt facts still
/// report `NoCompatibleHome` hands custody here: recovery runs over the
/// rematerialized program and its rebuilt analyses, the recorded sweep stays
/// the first transformation in the produced manifest, and retained replay
/// re-proves the whole prefix before trusting any spill step.
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
/// suite completion stay recorded ahead of every spill step, and retained
/// replay re-proves the whole run before trusting any step.
pub(crate) fn recover_after_selected_lowering(
    run: StagedSelectedLoweringOptimizationRun,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::SelectedLowering(run))
}

/// A completed pre-allocation run whose final facts still report
/// `NoCompatibleHome` hands custody here the same way: recovery runs over
/// the run's current program and rebuilt analyses, the committed
/// transformation ledger and the suite completion stay recorded ahead of
/// every spill step, and retained replay re-proves the whole run before
/// trusting any step.
pub(crate) fn recover_after_pre_allocation(
    run: StagedPreAllocationOptimizationRun,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    recover_over(RuntimeSpillSource::PreAllocation(run))
}

/// Both exit points declare the manifest over the same retained prefix.
/// Allocation admission independently reconstructs that prefix and the complete
/// recovery before publishing any current facts.
fn project_recovery_manifest(
    base: &RuntimeSpillManifestBase,
    steps: &[RuntimeSpillStep],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &crate::ValidatedRegisterHomes,
) -> Result<crate::ValidatedPostAllocationOptimizationManifest, RuntimeSpillAllocationError> {
    crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest_with_suite_completions(
        base.pre_physical,
        base.selected_lowering_completion,
        base.pre_allocation_completion,
        &transformations(&base.prefix, steps),
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
    let mut failure = match assign(environment, source.ranges(), source.legality()) {
        Err(error @ crate::RegisterHomeError::NoCompatibleHome { .. }) => error,
        Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
        Ok(_) => return Err(RuntimeSpillAllocationError::RecoveryNotRequired),
    };
    let budget = source.budget_per_pass();
    let mut steps: Vec<RuntimeSpillStep> = Vec::new();
    let mut tried: std::collections::BTreeSet<(usize, VirtualRegisterId)> =
        std::collections::BTreeSet::new();
    let mut current_ranges = source.ranges().clone();
    let mut current_liveness = source.liveness().clone();
    let mut current_legality = source.legality().clone();
    loop {
        let selected = steps
            .last()
            .map_or_else(|| source.base(), |step| step.rewrite.selected());
        // Committed rewrites mint registers the original plan never had: a
        // split's fresh register inherits every later reference, fixed views
        // included, so it can hold the surviving pin the failed domain
        // overlaps. Re-derive the pool from the current plan minus every
        // consumed victim, or that fresh interval stays unreachable and the
        // same failure rotates through the remaining members forever.
        // Reloads minted by spill and rematerialization retire as they are
        // minted and never join.
        let mut roster: Vec<(usize, VirtualRegisterId)> = candidates(selected.plan())
            .into_iter()
            .filter(|entry| !tried.contains(entry))
            .collect();
        // The split decision comes first: one copy after a pinned definition
        // is cheaper than any regeneration or storage, and it is committed
        // whenever admitted. It replaces one live range by two disjoint ones
        // joined at the copy, so no point gains a live value; the pinned
        // segment the failed domain overlapped now ends at the copy.
        let split = admissible_split(
            &failure,
            roster.iter(),
            &current_ranges,
            &selected,
            environment,
            budget,
        )?;
        let mut failed_crossing = None;
        let (function, register, rewrite) = if let Some((candidate, split)) = split {
            tried.insert(candidate);
            (
                candidate.0,
                candidate.1,
                RuntimeSpillStepRewrite::Split(split),
            )
        } else {
            let Some(position) = candidate_position(
                &failure,
                &roster,
                |(function, register)| {
                    overlaps_pressure(&failure, &current_ranges, *function, *register)
                },
                |(function, register)| {
                    split_domain_relief(&failure, &current_ranges, *function, *register)
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
                    // interval. Replay independently re-derives this exact
                    // decision.
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
                        match assign(environment, &probe.ranges, &probe.legality) {
                            Ok(homes) => {
                                steps.push(RuntimeSpillStep {
                                    function,
                                    register,
                                    rewrite: RuntimeSpillStepRewrite::Spill(crossing),
                                });
                                let manifest = project_recovery_manifest(
                                    &base,
                                    &steps,
                                    &probe.ranges,
                                    &probe.legality,
                                    &homes,
                                )?;
                                let result = RuntimeSpillAllocation {
                                    source,
                                    steps,
                                    facts: probe,
                                    homes,
                                    manifest,
                                };
                                return Ok(result);
                            }
                            Err(error @ crate::RegisterHomeError::NoCompatibleHome { .. }) => {
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
                        Err(error) if replay::inadmissible(&error) => continue,
                        Err(error) => return Err(RuntimeSpillAllocationError::Rewrite(error)),
                    }
                }
            };
            (function, register, rewrite)
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
            let homes = assign(environment, &facts.ranges, &facts.legality);
            (facts, homes)
        };
        if !matches!(rewrite, RuntimeSpillStepRewrite::Split(_)) {
            retire_minted(&mut tried, selected.plan(), rewrite.selected().plan());
        }
        steps.push(RuntimeSpillStep {
            function,
            register,
            rewrite,
        });
        match homes {
            Ok(homes) => {
                let manifest = project_recovery_manifest(
                    &base,
                    &steps,
                    &facts.ranges,
                    &facts.legality,
                    &homes,
                )?;
                let result = RuntimeSpillAllocation {
                    source,
                    steps,
                    facts,
                    homes,
                    manifest,
                };
                return Ok(result);
            }
            Err(error @ crate::RegisterHomeError::NoCompatibleHome { .. }) => {
                failure = error;
                current_ranges = facts.ranges;
                current_legality = facts.legality;
                current_liveness = facts.liveness;
            }
            Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
        }
    }
    Err(RuntimeSpillAllocationError::Homes(failure))
}

#[cfg(test)]
mod candidate_tests;

#[cfg(test)]
mod probe_tests;
