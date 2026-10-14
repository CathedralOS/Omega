//! Exact checked Unit call-closure and identity validation.

use super::super::CheckedTerminalSignatureEligibility;
use super::bodies::UnitPlans;
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError,
    UnitBody, unsupported,
};
use checked_trees::{CheckedUnitCallResult, CheckedUnitCallRoute};
use typed_trees::type_identity::CallableIdentityId;

/// Installation requirements share an overload identity across target bodies.
/// Foreign leaves instead name physical declarations: sibling leaves may have
/// that same overload but remain distinct imports in the portable artifact.
/// Keep both relationships as handles; text is only their exported spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoundaryIdentity {
    Requirement(CallableIdentityId),
    ForeignLeaf(symbols::SymbolHandle),
}

impl BoundaryIdentity {
    pub(super) fn order_key(self) -> (bool, CallableIdentityId, u32, u32) {
        match self {
            Self::Requirement(callable) => (false, callable, 0, 0),
            Self::ForeignLeaf(declaration) => (
                true,
                CallableIdentityId::default(),
                declaration.arena_index(),
                declaration.generation(),
            ),
        }
    }

    pub(crate) fn render(self, checked: &CheckedTrees) -> Result<String, LoweringError> {
        match self {
            Self::Requirement(callable) => Ok(checked.typed.callable_identity_text(callable)),
            Self::ForeignLeaf(declaration) => Ok(unique_terminal_selection(checked, declaration)?
                .name
                .clone()),
        }
    }
}

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
        for request in checked
            .facts
            .quotient_operations
            .iter()
            .filter(|row| row.public_machine == machine_symbol)
        {
            let (representative, _) = validation::find_machine_by_entry_state(
                &checked.typed,
                request.representative_state,
            )
            .ok_or(LoweringError::Unsupported(
                "quotient operation has no representative body",
            ))?;
            if !closure.contains(&representative.symbol) {
                closure.push(representative.symbol);
            }
        }
        for cleanup in crate::unit::unit_cleanup::continuation_cleanups(machine.operations()) {
            if !closure.contains(&cleanup.cleanup_machine) {
                closure.push(cleanup.cleanup_machine);
            }
        }
        {
            let rows = plans
                .for_machine(machine_symbol)
                .map(|machine| machine.normal_exit_cleanups)
                .into_iter()
                .chain(
                    plans
                        .composed_for_machine(machine_symbol)
                        .into_iter()
                        .flat_map(|machine| {
                            machine
                                .states
                                .iter()
                                .map(|state| state.normal_exit_cleanups)
                        }),
                );
            for cleanup in rows
                .flat_map(|rows| {
                    checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .cleanup_control
                        .normal_actions(rows)
                })
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
) -> Result<BoundaryIdentity, LoweringError> {
    let requirements = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_instances(definition)
                .filter(move |signature| signature.symbol == symbol)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    if let [(definition, signature)] = requirements.as_slice() {
        let identity = checked
            .typed
            .trait_requirement_callable_identity(definition, signature);
        if identity.is_valid() {
            return Ok(BoundaryIdentity::Requirement(identity));
        }
    }
    // Sealed generated accessors are semantic requirement slots, not foreign
    // import/export spellings. Psi and provider planning share the exact role.
    if let Some(accessor) = typed_trees::typed_trees::PlacedScalarAccessorDeclaration::authenticate(
        &checked.typed,
        symbol,
    )
    .map_err(LoweringError::InvalidSealedAccessorDeclaration)?
    {
        let machine =
            checked
                .typed
                .machine(accessor.machine())
                .ok_or(LoweringError::Unsupported(
                    "sealed accessor lost its exact machine",
                ))?;
        let identity = checked.typed.machine_callable_identity(machine);
        if !identity.is_valid() {
            return unsupported("sealed accessor has no normalized callable identity");
        }
        return Ok(BoundaryIdentity::Requirement(identity));
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
        let identity = checked.typed.machine_callable_identity(requirement);
        if identity.is_valid() {
            return Ok(BoundaryIdentity::Requirement(identity));
        }
        return unsupported("top-level boundary has no normalized machine overload identity");
    }
    if let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == symbol)
    {
        let identity = checked
            .typed
            .boundary_operator_requirement_callable_identity(operator);
        if !identity.is_valid() {
            return unsupported("boundary operator has no normalized requirement identity");
        }
        return Ok(BoundaryIdentity::Requirement(identity));
    }
    // A foreign import names this physical declaration, not its portable
    // overload. Its display spelling is an export payload, never a lookup
    // into the canonical callable-identity table.
    unique_terminal_selection(checked, symbol)
        .map(|selection| BoundaryIdentity::ForeignLeaf(selection.machine))
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
