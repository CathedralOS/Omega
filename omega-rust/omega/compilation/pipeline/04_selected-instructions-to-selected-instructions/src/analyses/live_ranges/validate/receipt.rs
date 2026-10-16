//! Validation receipt projection from an already replayed live-range plan.

use std::collections::BTreeSet;

use super::FunctionCounts;

use crate::{LiveRangeValidationReceipt, ValidatedLiveRanges};
use selected_instructions::{DistinctUseDefTie, LiveRangePlan, live_range_identity_from_digests};

/// Seal a live-range plan with its receipt. A function marked in `carried`
/// keeps its predecessor's identity digest from `previous_digests`.
pub(crate) fn admit(
    plan: LiveRangePlan,
    carried: &[bool],
    previous_digests: &[[u8; 32]],
    previous_counts: &[FunctionCounts],
) -> ValidatedLiveRanges {
    let function_digests =
        crate::analyses::carried_digests(&plan.functions, carried, previous_digests, |function| {
            selected_instructions::live_range_function_digest(function)
        });
    // Reuse report metadata under precisely the same function carry decision
    // as its canonical digest. Missing metadata is computed from current rows.
    let function_counts: std::sync::Arc<[FunctionCounts]> = plan
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| match previous_counts.get(index) {
            Some(counts) if carried.get(index) == Some(&true) => *counts,
            _ => FunctionCounts::new(function),
        })
        .collect();
    let receipt = build_receipt(&plan, &function_digests, &function_counts);
    ValidatedLiveRanges {
        plan: plan.into(),
        receipt,
        function_digests,
        function_counts,
    }
}

fn build_receipt(
    plan: &LiveRangePlan,
    function_digests: &[[u8; 32]],
    function_counts: &[FunctionCounts],
) -> LiveRangeValidationReceipt {
    LiveRangeValidationReceipt {
        identity: live_range_identity_from_digests(plan, function_digests),
        selected: plan.selected,
        liveness: plan.liveness,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        function_count: plan.functions.len(),
        block_count: function_counts
            .iter()
            .map(|counts| counts.block_count)
            .sum(),
        virtual_register_count: function_counts
            .iter()
            .map(|counts| counts.virtual_register_count)
            .sum(),
        virtual_occurrence_count: function_counts
            .iter()
            .map(|counts| counts.virtual_occurrence_count)
            .sum(),
        fixed_constraint_count: function_counts
            .iter()
            .map(|counts| counts.fixed_constraint_count)
            .sum(),
        virtual_fragment_count: function_counts
            .iter()
            .map(|counts| counts.virtual_fragment_count)
            .sum(),
        architectural_unit_count: function_counts
            .iter()
            .map(|counts| counts.architectural_unit_count)
            .sum(),
        architectural_action_count: function_counts
            .iter()
            .map(|counts| counts.architectural_action_count)
            .sum(),
        architectural_fragment_count: function_counts
            .iter()
            .map(|counts| counts.architectural_fragment_count)
            .sum(),
        virtual_edge_connector_count: function_counts
            .iter()
            .map(|counts| counts.virtual_edge_connector_count)
            .sum(),
        architectural_edge_connector_count: function_counts
            .iter()
            .map(|counts| counts.architectural_edge_connector_count)
            .sum(),
        interference_count: function_counts
            .iter()
            .map(|counts| counts.interference_count)
            .sum(),
        tied_pair_count: function_counts
            .iter()
            .map(|counts| counts.tied_pair_count)
            .sum(),
        copy_affinity_count: function_counts
            .iter()
            .map(|counts| counts.copy_affinity_count)
            .sum(),
        tied_component_count: function_counts
            .iter()
            .map(|counts| counts.tied_component_count)
            .sum(),
        early_clobber_count: function_counts
            .iter()
            .map(|counts| counts.early_clobber_count)
            .sum(),
        early_clobber_use_count: function_counts
            .iter()
            .map(|counts| counts.early_clobber_use_count)
            .sum(),
    }
}

pub(super) fn tied_component_count(ties: &[DistinctUseDefTie]) -> usize {
    let mut components = Vec::<BTreeSet<selected_instructions::VirtualRegisterId>>::new();
    for tie in ties {
        let use_component = components
            .iter()
            .position(|component| component.contains(&tie.use_virtual_register));
        let def_component = components
            .iter()
            .position(|component| component.contains(&tie.def_virtual_register));
        match (use_component, def_component) {
            (None, None) => components.push(BTreeSet::from([
                tie.use_virtual_register,
                tie.def_virtual_register,
            ])),
            (Some(component), None) => {
                components[component].insert(tie.def_virtual_register);
            }
            (None, Some(component)) => {
                components[component].insert(tie.use_virtual_register);
            }
            (Some(left), Some(right)) if left != right => {
                let (keep, remove) = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                let removed = components.remove(remove);
                components[keep].extend(removed);
            }
            (Some(_), Some(_)) => {}
        }
    }
    components.len()
}
