//! Executable pressure recovery over a finite roster of original runtime values.
//! Allocation chooses the values; the selected rewrite owner validates semantics.

use super::model::{
    RuntimeSpillAllocation, RuntimeSpillAllocationError, RuntimeSpillFacts, RuntimeSpillStep,
    RuntimeSpillStepRewrite,
};
use super::replay;
use crate::{SelectedProgramRef, StagedOptimizedAllocationLegality, ValidatedSelectedAnalysis};
use selected_instructions::{VirtualRegisterId, VirtualRegisterOrigin};

pub(crate) fn assign_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    assign(
        source,
        source.live_range_stage().ranges(),
        source.legality(),
    )
}

pub(super) fn assign(
    source: &StagedOptimizedAllocationLegality,
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
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
    source: &StagedOptimizedAllocationLegality,
    previous: &impl ValidatedSelectedAnalysis,
    previous_liveness: &crate::ValidatedLiveness,
    previous_ranges: &crate::ValidatedLiveRanges,
    selected: &impl ValidatedSelectedAnalysis,
) -> Result<RuntimeSpillFacts, RuntimeSpillAllocationError> {
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
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
        source.allocator_availability(),
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
// incoming path; its presence here grants neither that storage nor a home.
// Reloads from either recovery form never become new candidates.
pub(super) fn candidates(
    source: &StagedOptimizedAllocationLegality,
) -> Vec<(usize, VirtualRegisterId)> {
    source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected()
        .plan()
        .functions
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
                    )
                })
                .map(move |register| (function, register.id))
        })
        .collect()
}

pub(super) fn transformations(
    steps: &[RuntimeSpillStep],
) -> Vec<crate::PostAllocationSelectedTransformation> {
    steps
        .iter()
        .map(|step| match &step.rewrite {
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
        })
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
fn candidate_position(
    failure: &crate::RegisterHomeError,
    roster: &[(usize, VirtualRegisterId)],
    overlaps: impl FnMut(&(usize, VirtualRegisterId)) -> bool,
) -> Option<usize> {
    let crate::RegisterHomeError::NoCompatibleHome { function, register } = failure else {
        return None;
    };
    roster
        .iter()
        .position(|candidate| *candidate == (*function, VirtualRegisterId(*register)))
        .or_else(|| roster.iter().position(overlaps))
}

pub(crate) fn recover(
    source: StagedOptimizedAllocationLegality,
) -> Result<RuntimeSpillAllocation, RuntimeSpillAllocationError> {
    let upstream = crate::validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(RuntimeSpillAllocationError::Upstream)?;
    let mut failure = match assign_source(&source) {
        Err(error @ crate::RegisterHomeError::NoCompatibleHome { .. }) => error,
        Err(error) => return Err(RuntimeSpillAllocationError::Homes(error)),
        Ok(_) => return Err(RuntimeSpillAllocationError::RecoveryNotRequired),
    };
    let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
    let environment = selected_stage.register_environment();
    let budget = selected_stage
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let mut steps: Vec<RuntimeSpillStep> = Vec::new();
    let mut roster = candidates(&source);
    let mut current_ranges = source.live_range_stage().ranges().clone();
    let mut current_liveness = source
        .live_range_stage()
        .liveness_stage()
        .liveness()
        .clone();
    while let Some(position) = candidate_position(&failure, &roster, |(function, register)| {
        overlaps_pressure(&failure, &current_ranges, *function, *register)
    }) {
        let (function, register) = roster.remove(position);
        let selected = steps.last().map_or_else(
            || SelectedProgramRef::new(selected_stage.selected()),
            |step| step.rewrite.selected(),
        );
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
            Err(_) => match crate::spill_selected_runtime_value(
                &selected,
                function,
                register,
                environment,
                budget,
            ) {
                Ok(rewrite) => RuntimeSpillStepRewrite::Spill(rewrite),
                Err(error) if replay::inadmissible(&error) => continue,
                Err(error) => return Err(RuntimeSpillAllocationError::Rewrite(error)),
            },
        };
        let facts = analyze(
            &source,
            &selected,
            &current_liveness,
            &current_ranges,
            &rewrite.selected(),
        )?;
        let homes = assign(&source, &facts.ranges, &facts.legality);
        steps.push(RuntimeSpillStep {
            function,
            register,
            rewrite,
        });
        match homes {
            Ok(homes) => {
                let manifest = crate::project_post_allocation_optimization_manifest(
                    upstream.manifest(),
                    &transformations(&steps),
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
