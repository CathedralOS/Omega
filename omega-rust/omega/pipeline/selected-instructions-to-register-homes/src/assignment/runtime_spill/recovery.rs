//! Executable pressure recovery over a finite roster of original runtime values.
//! Allocation chooses the values; the selected rewrite owner validates semantics.

use super::model::{
    RuntimeSpillAllocation, RuntimeSpillAllocationError, RuntimeSpillFacts, RuntimeSpillSource,
    RuntimeSpillStep, RuntimeSpillStepRewrite,
};
use super::replay;
use crate::{StagedOptimizedAllocationLegality, ValidatedSelectedAnalysis};
use selected_instructions::{SelectedInstructionPlan, VirtualRegisterId, VirtualRegisterOrigin};

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
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
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
    availability: &crate::ValidatedAllocatorAvailability,
    previous: &impl ValidatedSelectedAnalysis,
    previous_liveness: &crate::ValidatedLiveness,
    previous_ranges: &crate::ValidatedLiveRanges,
    selected: &impl ValidatedSelectedAnalysis,
) -> Result<RuntimeSpillFacts, RuntimeSpillAllocationError> {
    let liveness = crate::analyze_liveness_reusing(previous, previous_liveness, selected)
        .map_err(RuntimeSpillAllocationError::Liveness)?;
    let ranges = crate::analyze_live_ranges_reusing(
        previous,
        previous_liveness,
        previous_ranges,
        selected,
        &liveness,
    )
    .map_err(RuntimeSpillAllocationError::Ranges)?;
    let legality = crate::analyze_allocation_legality(
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

// Keep a finite roster from the original program. A block parameter can use
// private edge-initialized storage only when the selected rewrite checks every
// incoming path, and an entry-bound register — a scalar or structural
// parameter, or the hidden aggregate-result destination — only while the
// entry boundary itself stores it; presence here grants neither that storage
// nor a home. Instruction-defined structural registers — field observations
// and transport pointers — join by origin so a pressured structural plan can
// spill them like any other value; admission still rejects the ones defined
// by address-producing instructions.
// Reloads from either recovery form never become new candidates.
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

/// Every recorded transformation this recovery produced, appended after the
/// source's own prefix: a fixed-view entry keeps its copy transformation first.
pub(super) fn transformations(
    prefix: &[crate::PostAllocationSelectedTransformation],
    steps: &[RuntimeSpillStep],
) -> Vec<crate::PostAllocationSelectedTransformation> {
    prefix
        .iter()
        .cloned()
        .chain(steps.iter().map(|step| match &step.rewrite {
            RuntimeSpillStepRewrite::Spill(rewrite) => {
                crate::PostAllocationSelectedTransformation::RuntimeSpill(
                    rewrite.receipt().transformed_selected(),
                )
            }
            RuntimeSpillStepRewrite::Rematerialization(rewrite) => {
                crate::PostAllocationSelectedTransformation::RuntimeRematerialization(
                    rewrite.receipt().transformed_selected(),
                )
            }
        }))
        .collect()
}

pub(super) fn overlaps_pressure(
    failure: &crate::RegisterHomeError,
    ranges: &crate::ValidatedLiveRanges,
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
    ranges: &crate::FunctionLiveRanges,
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
    ranges: &crate::FunctionLiveRanges,
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
    ranges: &crate::FunctionLiveRanges,
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
        .binary_search(&crate::VirtualInterference { lower, higher })
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
    ranges: &crate::ValidatedLiveRanges,
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
    ranges: &crate::FunctionLiveRanges,
    failed: VirtualRegisterId,
    register: VirtualRegisterId,
) -> usize {
    split_domain_members(ranges, failed)
        .iter()
        .copied()
        .filter(|member| *member == register || interferes(ranges, register, *member))
        .count()
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
    reanalysis: crate::StagedOptimizedSelectedReanalysis,
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
    policy: crate::FixedViewCopyPolicy,
    decline: crate::FixedPrecoloredSegmentHomeDecline,
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

fn recover_over(
    source: RuntimeSpillSource,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    let (upstream_manifest, prefix) = source.upstream_manifest()?;
    let environment = source.register_environment();
    let mut failure = match assign(environment, source.ranges(), source.legality()) {
        Err(error @ crate::RegisterHomeError::NoCompatibleHome { .. }) => error,
        Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
        Ok(_) => return Err(RuntimeSpillAllocationError::RecoveryNotRequired),
    };
    let budget = source.budget_per_pass();
    let mut steps: Vec<RuntimeSpillStep> = Vec::new();
    let mut roster = candidates(source.base().plan());
    let mut current_ranges = source.ranges().clone();
    let mut current_liveness = source.liveness().clone();
    while let Some(position) = candidate_position(
        &failure,
        &roster,
        |(function, register)| overlaps_pressure(&failure, &current_ranges, *function, *register),
        |(function, register)| split_domain_relief(&failure, &current_ranges, *function, *register),
    ) {
        let (function, register) = roster.remove(position);
        let selected = steps
            .last()
            .map_or_else(|| source.base(), |step| step.rewrite.selected());
        // The cost decision: regenerating one pure immediate materialization
        // per use is strictly cheaper than private storage plus reload pairs,
        // so rematerialization is attempted first. Its admission failure does
        // not commit the step; spilling the same victim remains the fallback.
        let rewrite = match crate::rematerialize_selected_runtime_value(
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
                if let Ok(crossing) = crate::spill_selected_runtime_value_with_span_policy(
                    &selected,
                    function,
                    register,
                    environment,
                    budget,
                    crate::RuntimeSpillSpanPolicy::UnitWriteCrossing,
                ) {
                    let probe = analyze(
                        environment,
                        source.allocator_availability(),
                        &selected,
                        &current_liveness,
                        &current_ranges,
                        &crate::SelectedProgramRef::new(&crossing),
                    )?;
                    match assign(environment, &probe.ranges, &probe.legality) {
                        Ok(homes) => {
                            steps.push(RuntimeSpillStep {
                                function,
                                register,
                                rewrite: RuntimeSpillStepRewrite::Spill(crossing),
                            });
                            let manifest = crate::project_post_allocation_optimization_manifest(
                                upstream_manifest,
                                &transformations(&prefix, &steps),
                                &probe.ranges,
                                &probe.legality,
                                &homes,
                            )
                            .map_err(RuntimeSpillAllocationError::Manifest)?;
                            let result = RuntimeSpillAllocation {
                                source,
                                steps,
                                facts: probe,
                                homes,
                                manifest,
                            };
                            replay::validate(&result)?;
                            return Ok(result);
                        }
                        Err(crate::RegisterHomeError::NoCompatibleHome { .. }) => {}
                        Err(error) => {
                            return Err(RuntimeSpillAllocationError::Homes(error));
                        }
                    }
                }
                match crate::spill_selected_runtime_value(
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
        let facts = analyze(
            environment,
            source.allocator_availability(),
            &selected,
            &current_liveness,
            &current_ranges,
            &rewrite.selected(),
        )?;
        let homes = assign(environment, &facts.ranges, &facts.legality);
        steps.push(RuntimeSpillStep {
            function,
            register,
            rewrite,
        });
        match homes {
            Ok(homes) => {
                let manifest = crate::project_post_allocation_optimization_manifest(
                    upstream_manifest,
                    &transformations(&prefix, &steps),
                    &facts.ranges,
                    &facts.legality,
                    &homes,
                )
                .map_err(RuntimeSpillAllocationError::Manifest)?;
                let result = RuntimeSpillAllocation {
                    source,
                    steps,
                    facts,
                    homes,
                    manifest,
                };
                replay::validate(&result)?;
                return Ok(result);
            }
            Err(error @ crate::RegisterHomeError::NoCompatibleHome { .. }) => {
                failure = error;
                current_ranges = facts.ranges;
                current_liveness = facts.liveness;
            }
            Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
        }
    }
    Err(RuntimeSpillAllocationError::Homes(failure))
}

#[cfg(test)]
#[path = "candidate_tests.rs"]
mod candidate_tests;
