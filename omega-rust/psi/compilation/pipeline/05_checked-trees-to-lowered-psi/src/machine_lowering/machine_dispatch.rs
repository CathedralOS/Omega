//! Exact checked-plan selection and dispatch into one Terminal Psi machine family.

use crate::checked_facts::flow_facts;
use checked_trees::{
    CheckedTerminalMachineSelection, CheckedTerminalSignatureEligibility, CheckedTrees,
    CheckedUnitPlan,
};

use crate::lowering_error::LoweringError;
use crate::producer_result::{DebugPublication, LoweredSelectedMachine, LoweringCompletion};
use crate::unit::attached_unit::lower_unit_effect_closure;
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
    /// Retain declarations and their evaluation work without choosing a default
    /// executable. Explicit roots still retain real bodies.
    Declarations,
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
        TerminalMachineSelection::Declarations => Err(LoweringError::Unsupported(
            "declaration production has no selected entry",
        )),
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

use crate::lowering_error::unsupported;

pub(crate) fn lower_selected_machine(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
    program_roots: crate::TerminalProgramRoots<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<LoweredSelectedMachine, LoweringError> {
    // A portable head owns a checked contract and complete member catalog,
    // but no executable body. Close the same catalog without inserting the
    // head among body roots or selecting a target on Psi's side.
    if checked.typed.target_machine_families.iter().any(|family| {
        family.canonical == selection.machine
            && !family
                .members
                .iter()
                .any(|member| member.machine == family.canonical)
    }) {
        return Ok(LoweredSelectedMachine::source_mapped(
            crate::unit::attached_unit::lower_unit_closure(
                identities,
                checked,
                &crate::unit::attached_unit::UnitClosureRequest::unit(
                    checked,
                    selection.machine,
                    &[],
                    program_roots,
                    exact_toolchain_sources,
                ),
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        ));
    }
    // Cleanup and composed plans lower as their own machines; a
    // result-bearing ordinary body lowers through a closure below, its
    // result with the cleanup after it.
    if let Some(plan) = select_unit_plan(checked, selection)? {
        return lower_unit_plan_machine(
            identities,
            checked,
            plan,
            program_roots,
            exact_toolchain_sources,
        );
    }
    if selection.signature == CheckedTerminalSignatureEligibility::Unsupported {
        return unsupported("machine signature is outside the current terminal-Psi source slice");
    }
    // An ordinary body with a scalar completion is a scalar entry: the
    // scalar-entry closure lowers that body with its callees and publishes
    // the checked debug map. Any other ordinary body takes the Unit closure,
    // which publishes none. A free Unit-effect signature reaches the Unit
    // closure even without a plan, so the closure reports the missing body.
    let scalar_entry =
        crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            checked,
            selection.machine,
        )
        .is_ok();
    let unit_plan = flow_facts(checked)
        .terminal_unit_effects
        .for_machine(selection.machine)
        .is_some();
    if !scalar_entry
        && (unit_plan || selection.signature == CheckedTerminalSignatureEligibility::FreeUnitEffect)
    {
        return Ok(LoweredSelectedMachine::source_mapped(
            lower_unit_effect_closure(
                identities,
                checked,
                selection.machine,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
                ..Default::default()
            },
        ));
    }
    if !scalar_entry {
        return Err(LoweringError::InvalidUnitMachinePlan {
            machine: checked.symbols.display_path(selection.machine, "::"),
            reason: "machine has no checked executable body",
            gap: flow_facts(checked)
                .terminal_unit_effects
                .gap_diagnostic(selection.machine, &checked.symbols),
        });
    }
    Ok(LoweredSelectedMachine::source_mapped(
        crate::unit::attached_unit::lower_scalar_effect_closure(
            identities,
            checked,
            selection.machine,
            program_roots,
            exact_toolchain_sources,
        )?,
        LoweringCompletion {
            ..Default::default()
        },
    ))
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
