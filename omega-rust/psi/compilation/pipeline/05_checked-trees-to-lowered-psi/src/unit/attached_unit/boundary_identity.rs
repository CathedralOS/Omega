//! Local boundary identity and its portable publication.
//!
//! Installation requirements use their normalized callable identities. Physical
//! foreign leaves retain their distinct declarations, even when target siblings
//! share a callable overload. Operator
//! slots retain their original operator coordinate because their portable slot
//! identities are outside the named-callable interner. Rehydrating an operator
//! spelling there loses a valid declaration as the zero handle. Physical leaves
//! use the common declaration callable key, including their target-qualified
//! original symbol path and full overload; a debug selection name alone cannot
//! distinguish same-path overloads. Render only
//! when publishing Terminal payloads and their canonical order; the original
//! declaration owns that rendering, never a diagnostic name join.

use super::call_closure::unique_terminal_selection;
use super::{CheckedBoundaryMachinePlan, CheckedTrees, LoweringError, unsupported};
use typed_trees::type_identity::CallableIdentityId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckedBoundaryIdentity {
    NamedCallable(CallableIdentityId),
    Operator(symbols::SymbolHandle),
    Declaration(symbols::SymbolHandle),
}

impl CheckedBoundaryIdentity {
    /// Export the original identity under the checked program that owns the
    /// handle. No absent or stale local coordinate exports an empty identity.
    pub(crate) fn portable_identity(self, checked: &CheckedTrees) -> Result<String, LoweringError> {
        let identity = match self {
            Self::NamedCallable(identity) if identity.is_valid() => {
                checked.typed.callable_identity_text(identity)
            }
            Self::Operator(symbol) if symbol.is_valid() => {
                let operator = checked
                    .typed
                    .operator_definition_by_symbol(symbol)
                    .filter(|operator| operator.is_boundary)
                    .ok_or(LoweringError::Unsupported(
                        "boundary operator has no original operator coordinate",
                    ))?;
                typed_trees::operator::boundary_operator_requirement_identity(
                    &checked.typed,
                    operator,
                )
            }
            Self::Declaration(symbol) if symbol.is_valid() => {
                let selection = unique_terminal_selection(checked, symbol)?;
                let identity = checked
                    .typed
                    .machine_declaration_callable_identity(selection.machine);
                if !identity.is_valid() {
                    return unsupported("physical boundary lost its exact declaration identity");
                }
                checked.typed.callable_identity_text(identity)
            }
            _ => return unsupported("boundary identity has an absent semantic coordinate"),
        };
        if identity.is_empty() {
            return unsupported("boundary identity has no portable declaration identity");
        }
        Ok(identity)
    }
}

/// Order the exported roster by its portable encoding, not by compilation-local
/// handle allocation. Export collisions are invalid even when the local handles
/// name distinct declarations. These strings own publication order only.
pub(crate) fn order_boundary_roster_for_publication(
    checked: &CheckedTrees,
    boundaries: &mut Vec<(&CheckedBoundaryMachinePlan, CheckedBoundaryIdentity)>,
) -> Result<(), LoweringError> {
    let mut published = boundaries
        .iter()
        .map(|&(plan, identity)| Ok((identity.portable_identity(checked)?, plan, identity)))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    published.sort_by(|left, right| left.0.cmp(&right.0));
    if published.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return unsupported("boundary Unit closure contains duplicate canonical identities");
    }
    boundaries.clear();
    boundaries.extend(
        published
            .into_iter()
            .map(|(_, plan, identity)| (plan, identity)),
    );
    Ok(())
}

pub(crate) fn checked_unit_boundary_identity(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<CheckedBoundaryIdentity, LoweringError> {
    if !symbol.is_valid() {
        return unsupported("boundary identity has an absent declaration coordinate");
    }
    let requirements = checked
        .typed
        .public_trait_requirement_signatures()
        .filter(|(definition, signature)| definition.is_boundary && signature.symbol == symbol)
        .collect::<Vec<_>>();
    if requirements.len() > 1 {
        return unsupported("boundary trait requirement identity is ambiguous");
    }
    if let [(definition, signature)] = requirements.as_slice() {
        let identity = checked
            .typed
            .trait_requirement_callable_identity(definition, signature);
        if identity.is_valid() {
            return Ok(CheckedBoundaryIdentity::NamedCallable(identity));
        }
        return unsupported("boundary trait has no normalized requirement identity");
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
            return Ok(CheckedBoundaryIdentity::NamedCallable(identity));
        }
        return unsupported("top-level boundary has no normalized machine overload identity");
    }
    if let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == symbol)
    {
        return Ok(CheckedBoundaryIdentity::Operator(operator.symbol));
    }
    // A physical foreign leaf names its exact declaration, not the overload
    // shared by target siblings. Retain the selected original symbol; the
    // shared declaration projection owns its complete portable identity.
    unique_terminal_selection(checked, symbol)
        .map(|selection| CheckedBoundaryIdentity::Declaration(selection.machine))
}
