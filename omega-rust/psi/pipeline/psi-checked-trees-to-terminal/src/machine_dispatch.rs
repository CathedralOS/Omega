//! Exact checked-plan selection and dispatch into one Terminal Psi machine family.

use psi_checked_trees::{
    CheckedTerminalMachineSelection, CheckedTerminalSignatureEligibility, CheckedTrees,
};

use crate::attached_unit::{lower_composed_unit_control_machine, lower_unit_effect_closure};
use crate::boundary_scalar_return::lower_boundary_scalar_return_machine;
use crate::dynamic_composed_unit::{
    lower_direct_dynamic_composed_unit_machine, lower_direct_dynamic_unit_machine,
    lower_joined_dynamic_composed_unit_machine, lower_joined_dynamic_unit_machine,
    lower_rebound_dynamic_composed_unit_machine, lower_rebound_dynamic_unit_machine,
    lower_stored_dynamic_composed_unit_machine,
};
use crate::payloadless_case_return::lower_payloadless_case_return_machine;
use crate::payloadless_guarded_call_return::lower_payloadless_guarded_call_return_machine;
use crate::scalar_call_closure::{checked_scalar_call_closure, lower_scalar_call_closure};
use crate::scalar_graph_lowering::lower_scalar_graph_machine;
use crate::structural_call_return::lower_structural_call_return_machine;
use crate::structural_return::lower_structural_return_machine;
use crate::structural_scalar_return::{
    lower_selected_operator_structural_scalar_return_machine,
    lower_structural_scalar_return_machine, lower_trait_operator_scalar_return_machine,
};
use crate::structural_unit_control::lower_structural_unit_control_machine;
use crate::unit_cleanup::{
    lower_nominal_affine_unit_cleanup_machine, lower_partial_affine_unit_cleanup_machine,
};
use crate::{LoweredTerminalPsi, LoweringError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectedMachineRoute {
    GuardedPayloadlessCallReturn {
        target_machine: psi_symbols::SymbolHandle,
    },
    TraitOperatorScalarReturn {
        realization_machine: psi_symbols::SymbolHandle,
    },
    SelectedOperatorStructuralScalarReturn {
        realization_machine: psi_symbols::SymbolHandle,
    },
    DirectDynamicComposedUnit {
        realization_machine: psi_symbols::SymbolHandle,
    },
    ReboundDynamicComposedUnit {
        realization_machine: psi_symbols::SymbolHandle,
    },
    StoredDynamicComposedUnit {
        realization_machine: psi_symbols::SymbolHandle,
    },
    JoinedDynamicComposedUnit {
        realization_machines: [psi_symbols::SymbolHandle; 2],
    },
    StructuralScalarReturn,
    NominalAffineUnitCleanup,
    PartialAffineUnitCleanup,
    BoundaryScalarReturn,
    StructuralCallReturn,
    PayloadlessCaseReturn,
    StructuralReturn,
    ComposedAttachedUnit,
    StructuralUnitControl,
    UnitEffect,
    ScalarGraph,
}

pub(super) struct LoweredSelectedMachine {
    pub(super) terminal: LoweredTerminalPsi,
    pub(super) route: SelectedMachineRoute,
}

pub(super) fn select_terminal_machine<'checked>(
    checked: &'checked CheckedTrees,
    machine_name: &str,
) -> Result<&'checked CheckedTerminalMachineSelection, LoweringError> {
    let mut matches = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|machine| machine.name == machine_name);
    let selection = matches
        .next()
        .ok_or_else(|| LoweringError::MachineNotFound(machine_name.to_owned()))?;
    if matches.next().is_some() {
        return Err(LoweringError::AmbiguousMachineName(machine_name.to_owned()));
    }
    Ok(selection)
}

fn routed_machine(
    terminal: Result<LoweredTerminalPsi, LoweringError>,
    route: SelectedMachineRoute,
) -> Result<LoweredSelectedMachine, LoweringError> {
    Ok(LoweredSelectedMachine {
        terminal: terminal?,
        route,
    })
}

fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}

pub(super) fn lower_selected_machine(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<LoweredSelectedMachine, LoweringError> {
    let joined_dynamic_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_scalar_calls
        .iter()
        .filter(|plan| plan.caller_machine == selection.machine)
        .collect::<Vec<_>>();
    let joined_dynamic_unit_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_unit_calls
        .iter()
        .filter(|plan| plan.caller_machine == selection.machine)
        .collect::<Vec<_>>();
    if !joined_dynamic_plans.is_empty() && !joined_dynamic_unit_plans.is_empty() {
        return unsupported("scalar and Unit dynamic joins compete for one caller");
    }
    if !joined_dynamic_plans.is_empty() {
        let [plan] = joined_dynamic_plans.as_slice() else {
            return unsupported("joined dynamic dispatch plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("joined dynamic dispatch requires an attached caller");
        }
        return routed_machine(
            lower_joined_dynamic_composed_unit_machine(checked, plan),
            SelectedMachineRoute::JoinedDynamicComposedUnit {
                realization_machines: [
                    plan.when_true.call.realization_machine,
                    plan.when_false.call.realization_machine,
                ],
            },
        );
    }
    if !joined_dynamic_unit_plans.is_empty() {
        let [plan] = joined_dynamic_unit_plans.as_slice() else {
            return unsupported("joined dynamic Unit plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("joined dynamic Unit dispatch requires an attached caller");
        }
        return routed_machine(
            lower_joined_dynamic_unit_machine(checked, plan),
            SelectedMachineRoute::JoinedDynamicComposedUnit {
                realization_machines: [
                    plan.when_true.call.realization_machine,
                    plan.when_false.call.realization_machine,
                ],
            },
        );
    }
    let stored_dynamic_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .stored_scalar_calls
        .iter()
        .filter(|plan| plan.call.caller_machine == selection.machine)
        .collect::<Vec<_>>();
    if !stored_dynamic_plans.is_empty() {
        let [plan] = stored_dynamic_plans.as_slice() else {
            return unsupported("stored dynamic dispatch plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("stored dynamic dispatch requires an attached caller");
        }
        return routed_machine(
            lower_stored_dynamic_composed_unit_machine(checked, plan),
            SelectedMachineRoute::StoredDynamicComposedUnit {
                realization_machine: plan.call.realization_machine,
            },
        );
    }
    let rebound_dynamic_unit_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .rebound_unit_calls
        .iter()
        .filter(|plan| plan.latest.caller_machine == selection.machine)
        .collect::<Vec<_>>();
    if !rebound_dynamic_unit_plans.is_empty() {
        let [plan] = rebound_dynamic_unit_plans.as_slice() else {
            return unsupported("rebound dynamic Unit plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("rebound dynamic Unit dispatch requires an attached caller");
        }
        return routed_machine(
            lower_rebound_dynamic_unit_machine(checked, plan),
            SelectedMachineRoute::ReboundDynamicComposedUnit {
                realization_machine: plan.latest.realization_machine,
            },
        );
    }
    let direct_dynamic_unit_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_unit_calls
        .iter()
        .filter(|plan| plan.caller_machine == selection.machine)
        .collect::<Vec<_>>();
    if !direct_dynamic_unit_plans.is_empty() {
        let [plan] = direct_dynamic_unit_plans.as_slice() else {
            return unsupported("direct dynamic Unit plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("direct dynamic Unit dispatch requires an attached caller");
        }
        return routed_machine(
            lower_direct_dynamic_unit_machine(checked, plan),
            SelectedMachineRoute::DirectDynamicComposedUnit {
                realization_machine: plan.realization_machine,
            },
        );
    }
    let rebound_dynamic_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .rebound_scalar_calls
        .iter()
        .filter(|plan| plan.latest.caller_machine == selection.machine)
        .collect::<Vec<_>>();
    if !rebound_dynamic_plans.is_empty() {
        let [plan] = rebound_dynamic_plans.as_slice() else {
            return unsupported("rebound dynamic dispatch plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("rebound dynamic dispatch requires an attached caller");
        }
        return routed_machine(
            lower_rebound_dynamic_composed_unit_machine(checked, plan),
            SelectedMachineRoute::ReboundDynamicComposedUnit {
                realization_machine: plan.latest.realization_machine,
            },
        );
    }
    let direct_dynamic_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls
        .iter()
        .filter(|plan| plan.caller_machine == selection.machine)
        .collect::<Vec<_>>();
    if !direct_dynamic_plans.is_empty() {
        let [plan] = direct_dynamic_plans.as_slice() else {
            return unsupported("direct dynamic dispatch plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("direct dynamic dispatch requires an attached caller");
        }
        return routed_machine(
            lower_direct_dynamic_composed_unit_machine(checked, plan),
            SelectedMachineRoute::DirectDynamicComposedUnit {
                realization_machine: plan.realization_machine,
            },
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_for_machine(selection.machine)
    {
        return routed_machine(
            lower_selected_operator_structural_scalar_return_machine(checked, plan),
            SelectedMachineRoute::SelectedOperatorStructuralScalarReturn {
                realization_machine: plan.realization_machine,
            },
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("guarded payloadless call return requires an attached signature");
        }
        return routed_machine(
            lower_payloadless_guarded_call_return_machine(checked, plan),
            SelectedMachineRoute::GuardedPayloadlessCallReturn {
                target_machine: plan.target_machine,
            },
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_for_machine(selection.machine)
    {
        return routed_machine(
            lower_trait_operator_scalar_return_machine(checked, plan),
            SelectedMachineRoute::TraitOperatorScalarReturn {
                realization_machine: plan.realization_machine,
            },
        );
    }
    // A result-bearing structural plan owns both the scalar result and its
    // post-result cleanup. It must win over overlapping Unit-only cleanup.
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural scalar return plan requires an attached signature");
        }
        return routed_machine(
            lower_structural_scalar_return_machine(checked, plan),
            SelectedMachineRoute::StructuralScalarReturn,
        );
    }
    let mut nominal_matches = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .iter()
        .filter(|plan| plan.machine.machine == selection.machine);
    if let Some(plan) = nominal_matches.next() {
        if nominal_matches.next().is_some() {
            return unsupported("nominal affine Unit cleanup plan is duplicated");
        }
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("nominal affine Unit cleanup requires an attached signature");
        }
        return routed_machine(
            lower_nominal_affine_unit_cleanup_machine(checked, plan),
            SelectedMachineRoute::NominalAffineUnitCleanup,
        );
    }
    let mut partial_matches = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .iter()
        .filter(|plan| plan.machine.machine == selection.machine);
    if let Some(plan) = partial_matches.next() {
        if partial_matches.next().is_some() {
            return unsupported("partial affine Unit cleanup plan is duplicated");
        }
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("partial affine Unit cleanup requires an attached signature");
        }
        return routed_machine(
            lower_partial_affine_unit_cleanup_machine(checked, plan),
            SelectedMachineRoute::PartialAffineUnitCleanup,
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("result-bearing boundary custody requires an attached signature");
        }
        return routed_machine(
            lower_boundary_scalar_return_machine(checked, plan),
            SelectedMachineRoute::BoundaryScalarReturn,
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural call result transfer requires an attached signature");
        }
        return routed_machine(
            lower_structural_call_return_machine(checked, plan),
            SelectedMachineRoute::StructuralCallReturn,
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_returns
        .payloadless_case_for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported(
                "payloadless structural case return requires an attached signature",
            );
        }
        return routed_machine(
            lower_payloadless_case_return_machine(checked, plan),
            SelectedMachineRoute::PayloadlessCaseReturn,
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural result transfer requires an attached signature");
        }
        return routed_machine(
            lower_structural_return_machine(checked, plan),
            SelectedMachineRoute::StructuralReturn,
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("composed Unit control requires an attached signature");
        }
        return routed_machine(
            lower_composed_unit_control_machine(checked, plan),
            SelectedMachineRoute::ComposedAttachedUnit,
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural Unit control plan requires an attached signature");
        }
        return routed_machine(
            lower_structural_unit_control_machine(checked, plan),
            SelectedMachineRoute::StructuralUnitControl,
        );
    }
    if selection.signature == CheckedTerminalSignatureEligibility::Eligible
        && checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(selection.machine)
            .is_some()
    {
        return routed_machine(
            lower_unit_effect_closure(checked, selection.machine),
            SelectedMachineRoute::UnitEffect,
        );
    }
    match selection.signature {
        CheckedTerminalSignatureEligibility::Eligible => {}
        CheckedTerminalSignatureEligibility::Attached
        | CheckedTerminalSignatureEligibility::FreeUnitEffect => {
            return routed_machine(
                lower_unit_effect_closure(checked, selection.machine),
                SelectedMachineRoute::UnitEffect,
            );
        }
        CheckedTerminalSignatureEligibility::Unsupported => {
            return unsupported(
                "machine signature is outside the current terminal-Psi source slice",
            );
        }
    }

    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(selection.machine)
        .ok_or(LoweringError::Unsupported(
            "machine has no source-independent checked scalar control plan",
        ))?;
    let closure = checked_scalar_call_closure(checked, selection.machine)?;
    let terminal = if closure.len() == 1 {
        lower_scalar_graph_machine(checked, selection.machine, graph)
    } else {
        lower_scalar_call_closure(checked, &closure)
    };
    routed_machine(terminal, SelectedMachineRoute::ScalarGraph)
}
