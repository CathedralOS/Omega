//! Validation receipt projection from independently checked custody.

use std::collections::BTreeSet;

use selected_instructions::VirtualRegisterId;

use crate::{
    DistinctUseDefTie, RegisterHomePlan, RegisterHomeValidationReceipt, ValidatedLiveRanges,
    register_home_identity,
};

pub(super) fn build(
    plan: &RegisterHomePlan,
    ranges: &ValidatedLiveRanges,
) -> RegisterHomeValidationReceipt {
    RegisterHomeValidationReceipt {
        identity: register_home_identity(plan),
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        function_count: plan.functions.len(),
        structural_unit_function_count: plan.structural_unit_functions.len(),
        assignment_count: plan
            .functions
            .iter()
            .map(|function| function.assignments.len())
            .sum(),
        tied_pair_count: ranges
            .plan()
            .functions
            .iter()
            .map(|function| function.tied_pairs.len())
            .sum(),
        tied_component_count: ranges
            .plan()
            .functions
            .iter()
            .map(|function| tied_component_count(&function.tied_pairs))
            .sum(),
        early_clobber_count: ranges
            .plan()
            .functions
            .iter()
            .map(|function| function.early_clobbers.len())
            .sum(),
    }
}

fn tied_component_count(ties: &[DistinctUseDefTie]) -> usize {
    let mut components = Vec::<BTreeSet<VirtualRegisterId>>::new();
    for tie in ties {
        let used = components
            .iter()
            .position(|component| component.contains(&tie.use_virtual_register));
        let defined = components
            .iter()
            .position(|component| component.contains(&tie.def_virtual_register));
        match (used, defined) {
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
