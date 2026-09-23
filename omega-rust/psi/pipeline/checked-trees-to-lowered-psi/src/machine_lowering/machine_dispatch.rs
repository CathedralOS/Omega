//! Exact checked-plan selection and dispatch into one Terminal Psi machine family.

use checked_trees::{
    CheckedDynamicBindingKind, CheckedDynamicDispatchPlan, CheckedReturnPlan,
    CheckedTerminalMachineSelection, CheckedTerminalSignatureEligibility, CheckedTrees,
    CheckedUnitPlan,
};

use crate::lowering_error::LoweringError;
use crate::producer_result::{
    ConformancePublication, DebugPublication, LoweredSelectedMachine, LoweringCompletion,
    OperandProofCompletion, SourceMapping,
};
use crate::returns::lower_return_machine;
use crate::scalar_graph::scalar_call_closure::{
    checked_scalar_call_closure, lower_scalar_call_closure,
};
use crate::scalar_graph::scalar_graph_lowering::lower_scalar_graph_machine;
use crate::unit::attached_unit::lower_unit_effect_closure;
use crate::unit::dynamic_composed_unit::{LoweredDynamicDispatch, lower_dynamic_dispatch_machine};
use crate::unit::lower_unit_plan_machine;

/// Which checked Terminal machine one lowering selects.
///
/// `Symbol` rejoins an already-selected exact checked machine: build product
/// operands resolve their implementation lexically and retain the exact
/// machine symbol, so production rejoins that symbol rather than a qualified
/// name another package could also declare. `Name` is the display-name
/// lookup for ad hoc producers and tests; an absent or ambiguous name fails
/// closed.
#[derive(Debug, Clone, Copy)]
pub enum TerminalMachineSelection<'a> {
    Name(&'a str),
    Symbol(symbols::SymbolHandle),
}

/// Select the exact checked Terminal machine `machine` names.
pub fn select_terminal_machine<'checked>(
    checked: &'checked CheckedTrees,
    machine: TerminalMachineSelection<'_>,
) -> Result<&'checked CheckedTerminalMachineSelection, LoweringError> {
    let mut machines = checked.facts.flow.terminal_machines.machines.iter();
    match machine {
        TerminalMachineSelection::Name(machine_name) => {
            let mut matches = machines.filter(|machine| machine.name == machine_name);
            let selection = matches
                .next()
                .ok_or_else(|| LoweringError::MachineNotFound(machine_name.to_owned()))?;
            if matches.next().is_some() {
                return Err(LoweringError::AmbiguousMachineName(machine_name.to_owned()));
            }
            Ok(selection)
        }
        TerminalMachineSelection::Symbol(machine) => machines
            .find(|selection| selection.machine == machine)
            .ok_or_else(|| {
                LoweringError::MachineNotFound(checked.typed.symbols.display_path(machine, "::"))
            }),
    }
}

fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}

pub(crate) fn lower_selected_machine(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<LoweredSelectedMachine, LoweringError> {
    if let Some(plan) = select_dynamic_dispatch_plan(checked, selection)? {
        let completion = LoweringCompletion {
            conformances: match plan.binding_kind() {
                CheckedDynamicBindingKind::Joined => ConformancePublication::BoundedModule,
                CheckedDynamicBindingKind::Rebound => ConformancePublication::BoundedRoot,
                CheckedDynamicBindingKind::Direct | CheckedDynamicBindingKind::Stored => {
                    ConformancePublication::ExactRoot
                }
            },
            ..Default::default()
        };
        return Ok(match lower_dynamic_dispatch_machine(checked, plan)? {
            LoweredDynamicDispatch::SourceMapped(lowered) => {
                LoweredSelectedMachine::source_mapped(lowered, completion)
            }
            LoweredDynamicDispatch::EntryOnly {
                terminal,
                source_machines,
            } => LoweredSelectedMachine::entry_only(terminal, completion, source_machines),
        });
    }
    // A result-bearing plan owns both its result and the cleanup after it, so
    // the return family is tried before the Unit-only cleanup plans below.
    if let Some(plan) = select_return_plan(checked, selection)? {
        return lower_return_machine(checked, plan);
    }
    if let Some(plan) = select_unit_plan(checked, selection)? {
        return lower_unit_plan_machine(checked, plan);
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
        return Ok(LoweredSelectedMachine::source_mapped(
            lower_unit_effect_closure(checked, selection.machine)?,
            LoweringCompletion {
                operands: OperandProofCompletion::Finalize,
                debug: DebugPublication::Omit,
                ..Default::default()
            },
        ));
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
        // An attached machine whose only checked body is a scalar graph owns
        // the scalar route outright: the Unit roster declined it, so the
        // graph is its body rather than a competing plan.
        CheckedTerminalSignatureEligibility::Attached
            if checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(selection.machine)
                .is_some()
                && checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .for_machine(selection.machine)
                    .is_none()
                && checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_for_machine(selection.machine)
                    .is_none() => {}
        CheckedTerminalSignatureEligibility::Attached
        | CheckedTerminalSignatureEligibility::FreeUnitEffect => {
            return Ok(LoweredSelectedMachine::source_mapped(
                lower_unit_effect_closure(checked, selection.machine)?,
                LoweringCompletion {
                    operands: OperandProofCompletion::Finalize,
                    debug: DebugPublication::Omit,
                    ..Default::default()
                },
            ));
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
        return Ok(LoweredSelectedMachine::source_mapped(
            crate::unit::attached_unit::lower_scalar_effect_closure(checked, selection.machine)?,
            LoweringCompletion {
                operands: OperandProofCompletion::Finalize,
                ..Default::default()
            },
        ));
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

/// The one checked dynamic dispatch plan `selection` lowers, if any. Every
/// binding kind and result shape shares this admission: one plan per caller
/// and an attached caller signature.
fn select_dynamic_dispatch_plan<'checked>(
    checked: &'checked CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<Option<&'checked CheckedDynamicDispatchPlan>, LoweringError> {
    let plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .for_caller(selection.machine)
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        let Some(first) = plans.first() else {
            return Ok(None);
        };
        let has_join = |scalar: bool| {
            plans.iter().any(|plan| {
                plan.binding_kind() == CheckedDynamicBindingKind::Joined
                    && matches!(plan, CheckedDynamicDispatchPlan::Scalar(_)) == scalar
            })
        };
        if has_join(true) && has_join(false) {
            return unsupported("scalar and Unit dynamic joins compete for one caller");
        }
        return unsupported(duplicated_dynamic_plan_refusal(first));
    };
    if selection.signature != CheckedTerminalSignatureEligibility::Attached {
        return unsupported("dynamic dispatch requires an attached caller");
    }
    Ok(Some(plan))
}

/// The one checked return plan `selection` lowers, if any. Every result kind
/// shares this admission: the selection's signature is the one the plan's
/// attachment implies — attached when the plan retains its owner, free
/// otherwise. The operator-realized kinds constrain their realization rather
/// than the caller's selection and admit any signature.
fn select_return_plan<'checked>(
    checked: &'checked CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<Option<CheckedReturnPlan<'checked>>, LoweringError> {
    let Some(plan) = CheckedReturnPlan::for_machine(&checked.facts.flow, selection.machine) else {
        return Ok(None);
    };
    if plan
        .expected_signature()
        .is_some_and(|expected| expected != selection.signature)
    {
        return unsupported("return plan attachment disagrees with its selected signature");
    }
    Ok(Some(plan))
}

/// The one checked Unit plan `selection` lowers, if any. Every kind shares
/// this admission: one roster row per machine, and the signature the plan's
/// attachment implies — attached when the plan retains its owner, either free
/// Unit signature otherwise.
fn select_unit_plan<'checked>(
    checked: &'checked CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<Option<CheckedUnitPlan<'checked>>, LoweringError> {
    let Some(plan) = CheckedUnitPlan::for_machine(&checked.facts.flow, selection.machine) else {
        return Ok(None);
    };
    if plan.is_duplicated_in(&checked.facts.flow) {
        return unsupported("Unit plan is duplicated in its roster");
    }
    if !plan.admits_signature(selection.signature) {
        return unsupported("Unit plan attachment disagrees with its selected signature");
    }
    Ok(Some(plan))
}

fn duplicated_dynamic_plan_refusal(plan: &CheckedDynamicDispatchPlan) -> &'static str {
    use CheckedDynamicBindingKind::{Direct, Joined, Rebound, Stored};
    use CheckedDynamicDispatchPlan::{Scalar, Unit};
    match (plan, plan.binding_kind()) {
        (Scalar(_), Direct) => "direct dynamic dispatch plan is duplicated for one caller",
        (Scalar(_), Rebound) => "rebound dynamic dispatch plan is duplicated for one caller",
        (Scalar(_), Stored) => "stored dynamic dispatch plan is duplicated for one caller",
        (Scalar(_), Joined) => "joined dynamic dispatch plan is duplicated for one caller",
        (Unit(_), Direct) => "direct dynamic Unit plan is duplicated for one caller",
        (Unit(_), Rebound) => "rebound dynamic Unit plan is duplicated for one caller",
        (Unit(_), Stored) => "stored dynamic Unit plan is duplicated for one caller",
        (Unit(_), Joined) => "joined dynamic Unit plan is duplicated for one caller",
    }
}
