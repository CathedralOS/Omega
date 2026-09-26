//! Rejoin a specialized provider with the exact closed requirement it refines.
//!
//! The source conformance continues to name the authored requirement template.
//! Retained specialization applications select the closed signature; neither
//! matching display names nor a successful generic declaration check authorizes
//! a different tuple. The caller applies ordinary conformance checking to the
//! resulting signature, contracts, lifetimes and effect ceiling.

use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::operator::{
    ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication,
    closed_operator_realization_application,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::typed_trees::MachineSpecialization;

pub(super) fn resolve<'program>(
    program: &'program TypedTrees,
    provider: &Machine,
    requirement: &'program Machine,
    symbols: &crate::validation::declarations::symbols::TopLevelSymbols<'_>,
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
    if provider.supply_mode != MachineSupplyMode::CheckedBody || !provider.body_is_present {
        return Err(failure());
    }
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
        || closed_operator_realization_application(program, provider, operator).as_ref()
            != Some(application)
    {
        return Err(failure());
    }
    crate::validation::declarations::operators::validate_closed_operator_application(
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
                .map(|machine| (receipt, machine))
        })
        .filter(|(_, candidate)| {
            candidate.supply_mode == MachineSupplyMode::TopLevelRequirement
                && program.machine_type_parameters(candidate).is_empty()
                && closed_operator_realization_application(program, candidate, operator)
                    .is_some_and(|reconstructed| {
                        same_application(program, application, &reconstructed)
                    })
        });
    let (closed_receipt, closed) = requirements.next().ok_or_else(failure)?;
    if requirements.next().is_some() {
        return Err(failure());
    }
    // Uniqueness is instance-wide, not scoped to the expected template: an
    // extra receipt under another template must not lend this instance two
    // incompatible origins. The retained tuple must also name the application
    // reconstructed from its signature. Checked commitment replay remains the
    // later authority for the rest of specialization custody.
    let identity = program
        .normalized_machine_overload_identity(requirement)
        .ok_or_else(failure)?;
    if program
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == closed.symbol)
        .count()
        != 1
        || closed_receipt.template_parameters != requirement.type_parameters
        || !crate::validation::machine_specialization_matches_template_identity(
            program,
            closed_receipt,
            &identity.identity(),
            program.symbols.symbol_package_identity(requirement.symbol),
        )
        || !retains_application_tuple(program, closed_receipt, application)
    {
        return Err(failure());
    }
    Ok(closed)
}

fn retains_application_tuple(
    program: &TypedTrees,
    receipt: &MachineSpecialization,
    application: &ClosedOperatorRealizationApplication,
) -> bool {
    let mut types = receipt.type_argument_identities.iter();
    let mut constants = receipt.const_argument_identities.iter();
    for argument in &application.arguments {
        let matches = match argument {
            ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                types.next().is_some_and(|identity| {
                    *identity
                        == program
                            .normalized_type_identity(*type_reference)
                            .into_string()
                })
            }
            ClosedOperatorApplicationArgument::Const { value, .. } => {
                let Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                    value,
                    ..
                }) = value.decode_encoding()
                else {
                    return false;
                };
                let value = value.to_string();
                let reference = program
                    .type_reference_table
                    .named_references()
                    .find(|(_, symbol, name)| !symbol.is_valid() && *name == value);
                reference.is_some_and(|(reference, _, _)| {
                    constants.next().is_some_and(|identity| {
                        *identity == program.normalized_type_identity(reference).into_string()
                    })
                })
            }
        };
        if !matches {
            return false;
        }
    }
    types.next().is_none() && constants.next().is_none()
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
