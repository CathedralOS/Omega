//! Exact checked Unit call-closure and identity validation.

use super::super::CheckedTerminalSignatureEligibility;
use super::bodies::UnitPlans;
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError,
    UnitBody, unsupported,
};
use checked_trees::{CheckedUnitCallResult, CheckedUnitCallRoute};
pub(crate) fn checked_unit_call_closure_including(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    entry: symbols::SymbolHandle,
    additional_roots: &[symbols::SymbolHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let mut closure = vec![entry];
    for root in additional_roots {
        if closure.contains(root) {
            return unsupported("attached Unit closure contains a duplicate explicit root");
        }
        closure.push(*root);
    }
    let mut next = 0_usize;
    while let Some(machine_symbol) = closure.get(next).copied() {
        next += 1;
        let name = checked_terminal_machine_name(checked, machine_symbol)?;
        let machine = UnitBody::find(plans, machine_symbol).map_err(
            crate::lowering_error::unit_machine_plan_error(checked, &name, machine_symbol),
        )?;
        for target in machine
            .operations()
            .filter_map(|operation| unit_closure_target(plans, operation))
        {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
        for cleanup in
            crate::unit::unit_cleanup::continuation_cleanups(machine.operations())
        {
            if !closure.contains(&cleanup.cleanup_machine) {
                closure.push(cleanup.cleanup_machine);
            }
        }
        // A nominal consuming machine's return edge invokes its exact
        // owner-attached `::drop` hook; the hook is roster work, not an
        // operation in the synthetic body, so it enters the closure here.
        if let Some(nominal) = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .iter()
            .find(|nominal| nominal.machine.machine == machine_symbol)
        {
            for cleanup in &nominal.cleanups {
                if !closure.contains(&cleanup.cleanup_machine) {
                    closure.push(cleanup.cleanup_machine);
                }
            }
        }
        if let Some(composed) = plans.composed_for_machine(machine_symbol) {
            for cleanup in composed
                .states
                .iter()
                .flat_map(|state| &state.normal_exit_cleanups)
                .flat_map(|exit| &exit.actions)
                .filter_map(|action| match action {
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup)
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_)
                    | checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => None,
                })
            {
                if !closure.contains(&cleanup.cleanup_machine) {
                    closure.push(cleanup.cleanup_machine);
                }
            }
        }
    }
    Ok(closure)
}

pub(super) use crate::scalar_lowering::call_closure::embedded::checked_scalar_call_closure_with_structural_roots;

pub(super) fn unique_unit_boundary(
    plans: UnitPlans<'_>,
    symbol: symbols::SymbolHandle,
) -> Result<&CheckedBoundaryMachinePlan, LoweringError> {
    let mut matches = plans
        .boundary_machines()
        .iter()
        .filter(|plan| plan.machine == symbol);
    let plan = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit closure is missing a checked boundary machine plan",
    ))?;
    if matches.next().is_some() {
        return unsupported("attached Unit closure contains duplicate boundary machine plans");
    }
    Ok(plan)
}

pub(super) fn checked_terminal_machine_name(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<&str, LoweringError> {
    let selection = unique_terminal_selection(checked, symbol)?;
    if !matches!(
        selection.signature,
        CheckedTerminalSignatureEligibility::FreeUnitEffect
            | CheckedTerminalSignatureEligibility::Eligible
    ) {
        return unsupported("Unit-effect member has an invalid checked terminal selection");
    }
    Ok(&selection.name)
}

/// The exact selection naming `symbol`, whatever its signature class. A
/// bodyless boundary machine is never lowered as a Terminal root, so its
/// `Unsupported` class does not bear on the identity its calls name.
fn unique_terminal_selection(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<&checked_trees::CheckedTerminalMachineSelection, LoweringError> {
    let mut matches = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == symbol);
    // The selection is what names the machine, so a missing one cannot report
    // through it. Name the member from its own symbol instead: reaching here
    // means the closure called a machine the selected target never selected --
    // a declaration scoped to another target, for one -- and a reader given
    // only "no checked terminal selection" has nothing to look for.
    let selection = matches
        .next()
        .ok_or_else(|| LoweringError::InvalidUnitMachinePlan {
            machine: checked.symbols.name(symbol).to_owned(),
            reason: "is called by this closure but the selected target selects no \
                 terminal machine for it",
            gap: checked
                .facts
                .flow
                .terminal_unit_effects
                .gap_diagnostic(symbol, &checked.symbols),
        })?;
    if matches.next().is_some() || selection.name.is_empty() {
        return unsupported("Unit-effect member has an invalid checked terminal selection");
    }
    Ok(selection)
}

pub(crate) fn checked_unit_boundary_identity(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let requirements = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == symbol)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    if let [(definition, signature)] = requirements.as_slice() {
        let identity = checked
            .typed
            .trait_requirement_callable_identity_text(definition, signature);
        if !identity.is_empty() {
            return Ok(identity);
        }
    }
    // Top-level installation dependencies are keyed by the normalized machine
    // overload, not its display name. Boundary calls must use that same exact
    // requirement identity or independent root replay sees an unused bound.
    let mut requirements = checked.typed.machines().iter().filter(|machine| {
        machine.symbol == symbol
            && (machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                || (machine.supply_mode == language_semantics::MachineSupplyMode::Boundary
                    && machine.service_reach_is_installation_bound))
    });
    if let Some(requirement) = requirements.next() {
        if requirements.next().is_some() {
            return unsupported("top-level boundary requirement identity is ambiguous");
        }
        return checked
            .typed
            .machine_callable_identity_text(requirement)
            .ok_or(LoweringError::Unsupported(
                "top-level boundary has no normalized machine overload identity",
            ));
    }
    if let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == symbol)
    {
        return Ok(
            typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator),
        );
    }
    // An owner-qualified boundary machine keeps its qualified display name.
    // Qualification contributes no receiver, so its selection class is the
    // bodyless `Unsupported` one rather than `Attached`.
    unique_terminal_selection(checked, symbol).map(|selection| selection.name.clone())
}

/// The machine an ordinary call enters inside the Unit roster: every ordinary
/// Unit call, and an ordinary structural call whose target has a Unit body.
/// Scalar and boundary calls reach their targets through other rosters.
fn unit_closure_target(
    plans: UnitPlans<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<symbols::SymbolHandle> {
    let call = operation.call()?;
    match (call.route(), call.result()) {
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Unit) => {
            Some(call.target.machine)
        }
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Structural { .. })
            if UnitBody::contains(plans, call.target.machine) =>
        {
            Some(call.target.machine)
        }
        _ => None,
    }
}

pub(super) fn reject_recursive_unit_closure(
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    fn visit(
        plans: UnitPlans<'_>,
        symbol: symbols::SymbolHandle,
        active: &mut Vec<symbols::SymbolHandle>,
        complete: &mut Vec<symbols::SymbolHandle>,
    ) -> Result<(), LoweringError> {
        if active.contains(&symbol) {
            return unsupported("recursive Unit call closure is not yet terminal");
        }
        if complete.contains(&symbol) {
            return Ok(());
        }
        active.push(symbol);
        for target in UnitBody::find(plans, symbol)?
            .operations()
            .filter_map(|operation| unit_closure_target(plans, operation))
        {
            visit(plans, target, active, complete)?;
        }
        active.pop();
        complete.push(symbol);
        Ok(())
    }

    let mut active = Vec::new();
    let mut complete = Vec::new();
    for symbol in closure {
        visit(plans, *symbol, &mut active, &mut complete)?;
    }
    Ok(())
}
