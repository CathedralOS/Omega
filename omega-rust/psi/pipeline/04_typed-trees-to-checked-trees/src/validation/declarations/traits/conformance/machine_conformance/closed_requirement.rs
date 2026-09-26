//! Rejoin a specialized provider with the exact closed requirement it refines.
//!
//! The source conformance continues to name the authored requirement template.
//! Retained specialization applications select the closed signature; neither
//! matching display names nor a successful generic declaration check authorizes
//! a different tuple. The caller applies ordinary conformance checking to the
//! resulting signature, contracts, lifetimes and effect ceiling.

use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::operator::{
    ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication,
};

pub(super) fn resolve<'program>(
    program: &'program TypedTrees,
    provider: &Machine,
    requirement: &'program Machine,
    symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
) -> Result<&'program Machine, Diagnostic> {
    if program.machine_type_parameters(requirement).is_empty()
        || !program.machine_type_parameters(provider).is_empty()
    {
        return Ok(requirement);
    }
    let failure = || {
        Diagnostic::error(format!(
            "provider machine `{}` has no unique checked closed application of top-level boundary requirement `{}`",
            provider.name, requirement.name,
        ))
    };
    let operator = program
        .machine_token_bindings()
        .iter()
        .find(|operator| operator.symbol == requirement.symbol)
        .ok_or_else(failure)?;
    let mut receipts = program
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == provider.symbol);
    let receipt = receipts.next().ok_or_else(failure)?;
    if receipts.next().is_some() {
        return Err(failure());
    }
    let mut applications = receipt
        .operator_realizations
        .iter()
        .filter(|application| application.requirement_symbol == requirement.symbol);
    let application = applications.next().ok_or_else(failure)?;
    if applications.next().is_some()
        || typed_trees::operator::closed_operator_realization_application(
            program, provider, operator,
        )
        .as_ref()
            != Some(application)
    {
        return Err(failure());
    }
    crate::declarations::operators::validate_closed_operator_application(
        program,
        symbols,
        operator,
        &application.arguments,
    )?;
    let mut requirements = program
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.template == requirement.symbol)
        .filter_map(|receipt| {
            program
                .machines()
                .iter()
                .find(|machine| machine.symbol == receipt.instance)
        })
        .filter(|candidate| {
            candidate.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && program.machine_type_parameters(candidate).is_empty()
                && typed_trees::operator::closed_operator_realization_application(
                    program, candidate, operator,
                )
                .is_some_and(|reconstructed| same_application(program, application, &reconstructed))
        });
    let closed = requirements.next().ok_or_else(failure)?;
    if requirements.next().is_some() {
        return Err(failure());
    }
    Ok(closed)
}

fn same_application(
    program: &TypedTrees,
    left: &ClosedOperatorRealizationApplication,
    right: &ClosedOperatorRealizationApplication,
) -> bool {
    left.requirement_symbol == right.requirement_symbol
        && left.arguments.len() == right.arguments.len()
        && left
            .arguments
            .iter()
            .zip(&right.arguments)
            .all(|(left, right)| match (left, right) {
                (
                    ClosedOperatorApplicationArgument::Type {
                        binder_symbol: left_binder,
                        type_reference: left_type,
                    },
                    ClosedOperatorApplicationArgument::Type {
                        binder_symbol: right_binder,
                        type_reference: right_type,
                    },
                ) => {
                    left_binder == right_binder
                        && program.package_qualified_type_identity(*left_type)
                            == program.package_qualified_type_identity(*right_type)
                }
                (
                    ClosedOperatorApplicationArgument::Const {
                        binder_symbol: left_binder,
                        declared_carrier: left_carrier,
                        value: left_value,
                    },
                    ClosedOperatorApplicationArgument::Const {
                        binder_symbol: right_binder,
                        declared_carrier: right_carrier,
                        value: right_value,
                    },
                ) => {
                    left_binder == right_binder
                        && left_value == right_value
                        && program.package_qualified_type_identity(*left_carrier)
                            == program.package_qualified_type_identity(*right_carrier)
                }
                _ => false,
            })
}
