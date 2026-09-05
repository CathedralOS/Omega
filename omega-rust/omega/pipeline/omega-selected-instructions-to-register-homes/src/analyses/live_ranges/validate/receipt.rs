//! Validation receipt projection from an already replayed live-range plan.

use std::collections::BTreeSet;

use crate::{DistinctUseDefTie, LiveRangePlan, LiveRangeValidationReceipt, live_range_identity};

pub(super) fn build_receipt(plan: &LiveRangePlan) -> LiveRangeValidationReceipt {
    LiveRangeValidationReceipt {
        identity: live_range_identity(plan),
        selected: plan.selected,
        liveness: plan.liveness,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        function_count: plan.functions.len(),
        structural_unit_function_count: plan.structural_unit_functions.len(),
        block_count: plan
            .functions
            .iter()
            .chain(&plan.structural_unit_functions)
            .map(|row| row.block_domains.len())
            .sum(),
        virtual_register_count: plan
            .functions
            .iter()
            .map(|row| row.virtual_registers.len())
            .sum(),
        virtual_occurrence_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|row| row.occurrences.len())
            .sum(),
        fixed_constraint_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|row| row.fixed_constraints.len())
            .sum(),
        virtual_fragment_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|row| row.fragments.len())
            .sum(),
        architectural_unit_count: plan
            .functions
            .iter()
            .chain(&plan.structural_unit_functions)
            .map(|row| row.architectural_units.len())
            .sum(),
        architectural_action_count: plan
            .functions
            .iter()
            .chain(&plan.structural_unit_functions)
            .flat_map(|row| &row.architectural_units)
            .map(|row| row.actions.len())
            .sum(),
        architectural_fragment_count: plan
            .functions
            .iter()
            .chain(&plan.structural_unit_functions)
            .flat_map(|row| &row.architectural_units)
            .map(|row| row.fragments.len())
            .sum(),
        virtual_edge_connector_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.virtual_registers)
            .map(|range| range.edge_connectors.len())
            .sum(),
        architectural_edge_connector_count: plan
            .functions
            .iter()
            .chain(&plan.structural_unit_functions)
            .flat_map(|row| &row.architectural_units)
            .map(|range| range.edge_connectors.len())
            .sum(),
        interference_count: plan
            .functions
            .iter()
            .map(|row| row.interference.len())
            .sum(),
        tied_pair_count: plan.functions.iter().map(|row| row.tied_pairs.len()).sum(),
        tied_component_count: plan
            .functions
            .iter()
            .map(|row| tied_component_count(&row.tied_pairs))
            .sum(),
        early_clobber_count: plan
            .functions
            .iter()
            .map(|row| row.early_clobbers.len())
            .sum(),
        early_clobber_use_count: plan
            .functions
            .iter()
            .flat_map(|row| &row.early_clobbers)
            .map(|row| row.uses.len())
            .sum(),
    }
}

pub(super) fn tied_component_count(ties: &[DistinctUseDefTie]) -> usize {
    let mut components = Vec::<BTreeSet<omega_selected_instructions::VirtualRegisterId>>::new();
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
