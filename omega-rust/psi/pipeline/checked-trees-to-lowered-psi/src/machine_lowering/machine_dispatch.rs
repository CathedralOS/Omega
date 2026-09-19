//! Exact checked-plan selection and dispatch into one Terminal Psi machine family.

use checked_trees::{
    CheckedTerminalMachineSelection, CheckedTerminalSignatureEligibility, CheckedTrees,
};

use crate::lowering_error::LoweringError;
use crate::producer_result::{
    ConformancePublication, DebugPublication, LoweredSelectedMachine, LoweringCompletion,
    OperandProofCompletion, SourceMappedLowered, SourceMapping,
};
use crate::returns::boundary_scalar_return::lower_boundary_scalar_return_machine;
use crate::returns::payloadless_case_return::lower_payloadless_case_return_machine;
use crate::returns::payloadless_guarded_call_return::lower_payloadless_guarded_call_return_machine;
use crate::returns::structural_return::lower_structural_return_machine;
use crate::returns::structural_scalar_return::{
    lower_selected_operator_structural_scalar_return_machine,
    lower_structural_scalar_return_machine, lower_trait_operator_scalar_return_machine,
};
use crate::scalar_graph::scalar_call_closure::{
    checked_scalar_call_closure, lower_scalar_call_closure,
};
use crate::scalar_graph::scalar_graph_lowering::lower_scalar_graph_machine;
use crate::unit::attached_unit::{lower_composed_unit_control_machine, lower_unit_effect_closure};
use crate::unit::dynamic_composed_unit::{
    lower_direct_dynamic_composed_unit_machine, lower_direct_dynamic_unit_machine,
    lower_joined_dynamic_composed_unit_machine, lower_joined_dynamic_unit_machine,
    lower_rebound_dynamic_composed_unit_machine, lower_rebound_dynamic_unit_machine,
    lower_stored_dynamic_composed_unit_machine,
};
use crate::unit::structural_unit_control::lower_structural_unit_control_machine;
use crate::unit::unit_cleanup::{
    lower_nominal_affine_unit_cleanup_machine, lower_partial_affine_unit_cleanup_machine,
};
use lowered_psi::LoweredPsi;

pub fn select_terminal_machine<'checked>(
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

/// Rejoin an already-selected machine by its exact checked symbol instead of
/// its qualified display name. Lexically resolved build product operands carry
/// the exact symbol so a same-named machine in another package cannot capture
/// this production.
pub fn select_terminal_machine_by_symbol(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<&CheckedTerminalMachineSelection, LoweringError> {
    checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.machine == machine)
        .ok_or_else(|| {
            LoweringError::MachineNotFound(checked.typed.symbols.display_path(machine, "::"))
        })
}

fn selected_machine(
    terminal: Result<LoweredPsi, LoweringError>,
    completion: LoweringCompletion,
    source_machines: Vec<symbols::SymbolHandle>,
) -> Result<LoweredSelectedMachine, LoweringError> {
    Ok(LoweredSelectedMachine {
        terminal: terminal?,
        source_machines,
        completion,
        source_mapping: SourceMapping::EntryOnly,
    })
}

fn source_mapped_machine(
    lowered: Result<SourceMappedLowered, LoweringError>,
    completion: LoweringCompletion,
) -> Result<LoweredSelectedMachine, LoweringError> {
    let lowered = lowered?;
    Ok(LoweredSelectedMachine {
        terminal: lowered.terminal,
        source_machines: lowered
            .source_machine_ids
            .iter()
            .map(|(source, _)| *source)
            .collect(),
        completion,
        source_mapping: SourceMapping::ExactCatalog(lowered.source_machine_ids),
    })
}

fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}

pub(crate) fn lower_selected_machine(
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
        return selected_machine(
            lower_joined_dynamic_composed_unit_machine(checked, plan),
            LoweringCompletion {
                conformances: ConformancePublication::BoundedModule,
                ..Default::default()
            },
            joined_source_machines(
                selection.machine,
                [
                    plan.when_true.call.realization_machine,
                    plan.when_false.call.realization_machine,
                ],
            ),
        );
    }
    if !joined_dynamic_unit_plans.is_empty() {
        let [plan] = joined_dynamic_unit_plans.as_slice() else {
            return unsupported("joined dynamic Unit plan is duplicated for one caller");
        };
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("joined dynamic Unit dispatch requires an attached caller");
        }
        return selected_machine(
            lower_joined_dynamic_unit_machine(checked, plan),
            LoweringCompletion {
                conformances: ConformancePublication::BoundedModule,
                ..Default::default()
            },
            joined_source_machines(
                selection.machine,
                [
                    plan.when_true.call.realization_machine,
                    plan.when_false.call.realization_machine,
                ],
            ),
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
        return source_mapped_machine(
            lower_stored_dynamic_composed_unit_machine(checked, plan),
            LoweringCompletion {
                conformances: ConformancePublication::ExactRoot,
                ..Default::default()
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
        return selected_machine(
            lower_rebound_dynamic_unit_machine(checked, plan),
            LoweringCompletion {
                conformances: ConformancePublication::BoundedRoot,
                ..Default::default()
            },
            vec![selection.machine, plan.latest.realization_machine],
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
        return selected_machine(
            lower_direct_dynamic_unit_machine(checked, plan),
            LoweringCompletion {
                conformances: ConformancePublication::ExactRoot,
                ..Default::default()
            },
            vec![selection.machine, plan.realization_machine],
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
        return source_mapped_machine(
            lower_rebound_dynamic_composed_unit_machine(checked, plan),
            LoweringCompletion {
                conformances: ConformancePublication::BoundedRoot,
                ..Default::default()
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
        return source_mapped_machine(
            lower_direct_dynamic_composed_unit_machine(checked, plan),
            LoweringCompletion {
                conformances: ConformancePublication::ExactRoot,
                ..Default::default()
            },
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_for_machine(selection.machine)
    {
        return selected_machine(
            lower_selected_operator_structural_scalar_return_machine(checked, plan),
            LoweringCompletion::default(),
            vec![selection.machine, plan.realization_machine],
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
        return selected_machine(
            lower_payloadless_guarded_call_return_machine(checked, plan),
            LoweringCompletion::default(),
            vec![selection.machine, plan.target_machine],
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_for_machine(selection.machine)
    {
        return selected_machine(
            lower_trait_operator_scalar_return_machine(checked, plan),
            LoweringCompletion::default(),
            vec![selection.machine, plan.realization_machine],
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
        let expected_signature = if plan.attachment_type_identity.is_some() {
            CheckedTerminalSignatureEligibility::Attached
        } else {
            CheckedTerminalSignatureEligibility::Eligible
        };
        if selection.signature != expected_signature {
            return unsupported(
                "structural scalar return plan disagrees with its selected signature",
            );
        }
        return selected_machine(
            lower_structural_scalar_return_machine(checked, plan),
            LoweringCompletion::default(),
            vec![selection.machine],
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
        if !matches!(
            (
                selection.signature,
                plan.machine.attachment_type_identity.is_some()
            ),
            (CheckedTerminalSignatureEligibility::Attached, true)
                | (CheckedTerminalSignatureEligibility::Eligible, false)
                | (CheckedTerminalSignatureEligibility::FreeUnitEffect, false)
        ) {
            return unsupported(
                "nominal affine Unit cleanup attachment disagrees with its signature",
            );
        }
        return selected_machine(
            lower_nominal_affine_unit_cleanup_machine(checked, plan),
            LoweringCompletion {
                operands: OperandProofCompletion::Finalize,
                ..Default::default()
            },
            vec![selection.machine],
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
        if !matches!(
            (
                selection.signature,
                plan.machine.attachment_type_identity.is_some()
            ),
            (CheckedTerminalSignatureEligibility::Attached, true)
                | (CheckedTerminalSignatureEligibility::Eligible, false)
                | (CheckedTerminalSignatureEligibility::FreeUnitEffect, false)
        ) {
            return unsupported(
                "partial affine Unit cleanup attachment disagrees with its signature",
            );
        }
        return source_mapped_machine(
            lower_partial_affine_unit_cleanup_machine(checked, plan),
            LoweringCompletion {
                operands: OperandProofCompletion::Finalize,
                debug: DebugPublication::Omit,
                ..Default::default()
            },
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
        let lowered = lower_boundary_scalar_return_machine(checked, plan)?;
        return Ok(LoweredSelectedMachine {
            terminal: lowered.terminal,
            source_machines: lowered
                .source_machine_ids
                .iter()
                .map(|(source, _)| *source)
                .collect(),
            completion: LoweringCompletion::default(),
            source_mapping: SourceMapping::ExactCatalog(lowered.source_machine_ids),
        });
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
        return selected_machine(
            lower_payloadless_case_return_machine(checked, plan),
            LoweringCompletion::default(),
            vec![selection.machine],
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
        return selected_machine(
            lower_structural_return_machine(checked, plan),
            LoweringCompletion::default(),
            vec![selection.machine],
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_for_machine(selection.machine)
    {
        if !matches!(
            selection.signature,
            CheckedTerminalSignatureEligibility::Eligible
                | CheckedTerminalSignatureEligibility::Attached
        ) || plan.attachment_type_identity.is_some()
            != (selection.signature == CheckedTerminalSignatureEligibility::Attached)
        {
            return unsupported(
                "affine identity return requires an exact free or attached signature",
            );
        }
        return selected_machine(
            crate::returns::affine_return::lower_affine_return_machine(checked, selection.machine),
            LoweringCompletion {
                debug: DebugPublication::Omit,
                ..Default::default()
            },
            vec![selection.machine],
        );
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(selection.machine)
    {
        if !matches!(
            selection.signature,
            CheckedTerminalSignatureEligibility::Eligible
                | CheckedTerminalSignatureEligibility::FreeUnitEffect
                | CheckedTerminalSignatureEligibility::Attached
        ) || plan.attachment_type_identity.is_some()
            != (selection.signature == CheckedTerminalSignatureEligibility::Attached)
        {
            return unsupported(
                "composed Unit control requires an exact free or attached signature",
            );
        }
        let composed = lower_composed_unit_control_machine(checked, plan)?;
        return Ok(LoweredSelectedMachine {
            terminal: composed.terminal,
            source_machines: composed
                .source_machine_ids
                .iter()
                .map(|(source, _)| *source)
                .collect(),
            completion: LoweringCompletion {
                debug: DebugPublication::Omit,
                ..Default::default()
            },
            source_mapping: SourceMapping::ExactCatalog(composed.source_machine_ids),
        });
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
        return selected_machine(
            lower_structural_unit_control_machine(checked, plan),
            LoweringCompletion::default(),
            vec![selection.machine],
        );
    }
    // A scalar forwarding body can also have a Unit-closure plan. Its scalar
    // graph owns the source signature, operand, and affine-transfer custody;
    // selecting the overlapping closure plan here would bypass those checks
    // and suppress the graph's debug map. The graph path below already shares
    // the real structural/Unit callee catalog when it needs that namespace.
    // A rejected graph must not fall back to the overlapping plan.
    if selection.signature == CheckedTerminalSignatureEligibility::Eligible
        && checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(selection.machine)
            .is_none()
        && checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(selection.machine)
            .is_some()
    {
        return source_mapped_machine(
            lower_unit_effect_closure(checked, selection.machine),
            LoweringCompletion {
                operands: OperandProofCompletion::Finalize,
                debug: DebugPublication::Omit,
                ..Default::default()
            },
        );
    }
    match selection.signature {
        CheckedTerminalSignatureEligibility::Eligible => {}
        CheckedTerminalSignatureEligibility::FreeUnitEffect
            if checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(selection.machine)
                .is_some() => {}
        CheckedTerminalSignatureEligibility::Attached
        | CheckedTerminalSignatureEligibility::FreeUnitEffect => {
            return source_mapped_machine(
                lower_unit_effect_closure(checked, selection.machine),
                LoweringCompletion {
                    operands: OperandProofCompletion::Finalize,
                    debug: DebugPublication::Omit,
                    ..Default::default()
                },
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
    if crate::scalar_graph::scalar_call_closure::requires_shared_catalog(
        checked,
        selection.machine,
    )? {
        return source_mapped_machine(
            crate::unit::attached_unit::lower_scalar_effect_closure(checked, selection.machine),
            LoweringCompletion {
                operands: OperandProofCompletion::Finalize,
                ..Default::default()
            },
        );
    }
    let closure = checked_scalar_call_closure(checked, selection.machine)?;
    let terminal = if closure.len() == 1 {
        lower_scalar_graph_machine(checked, selection.machine, graph)
    } else {
        lower_scalar_call_closure(checked, &closure)
    };
    Ok(LoweredSelectedMachine {
        terminal: terminal?,
        completion: LoweringCompletion {
            operands: OperandProofCompletion::Finalize,
            ..Default::default()
        },
        source_machines: closure,
        source_mapping: SourceMapping::ScalarClosureOrder,
    })
}

fn joined_source_machines(
    caller: symbols::SymbolHandle,
    realizations: [symbols::SymbolHandle; 2],
) -> Vec<symbols::SymbolHandle> {
    let mut sources = vec![caller];
    sources.extend(realizations);
    sources.sort_by_key(|machine| (machine.arena_index(), machine.generation()));
    sources.dedup();
    sources
}
