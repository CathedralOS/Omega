//! Rejoin a specialized provider with the exact closed requirement it refines.
//!
//! The source conformance continues to name the authored requirement template.
//! Retained specialization applications select the closed signature; neither
//! matching display names nor a successful generic declaration check authorizes
//! a different tuple. The caller applies ordinary conformance checking to the
//! resulting signature, contracts, lifetimes and effect ceiling.

use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use typed_trees::machine::Machine;
use typed_trees::operator::{
    ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication,
    closed_operator_realization_application,
};
use typed_trees::typed_trees::MachineSpecialization;

/// Resolve the exact requirement application named by a checked provider.
///
/// This joins retained specialization identities, not provider selection or
/// execution authority. Consumers must still validate the provider's
/// signature, body and conformance.
pub fn resolve<'program>(
    program: &'program dyn crate::CheckingInput,
    provider: &Machine,
    requirement: &'program Machine,
    symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
) -> Result<&'program Machine, Diagnostic> {
    if !requirement.has_specialization_parameters() || provider.has_specialization_parameters() {
        return Ok(requirement);
    }
    let failure = || {
        Diagnostic::error(format!(
            "provider machine `{}` has no unique checked closed application of top-level boundary requirement `{}`",
            provider.name, requirement.name,
        ))
    };
    if !provider.supplies_checked_body() || !provider.body_is_present {
        return Err(failure());
    }
    // Preparation publishes the original conformance-to-application join for
    // named requirements as well as operator requirements. This is the source
    // edge currently being checked, not an elected runtime provider.
    let mut produced = program
        .instantiated_requirement_realizations
        .iter()
        .filter(|(_, row)| {
            row.provider_instance == provider.symbol
                && row.requirement_template == requirement.symbol
        });
    if let Some((_, row)) = produced.next() {
        if produced.next().is_some() {
            return Err(failure());
        }
        let original = program.machine(row.provider_template).ok_or_else(failure)?;
        if !program
            .machine_trait_conformances(original)
            .iter()
            .enumerate()
            .any(|(ordinal, edge)| {
                original.satisfies.handle_at(ordinal as u32) == row.conformance
                    && edge.symbol == requirement.symbol
                    && edge.requirement_symbol == requirement.symbol
            })
        {
            return Err(failure());
        }
        let closed = program
            .machine(row.requirement_instance)
            .ok_or_else(failure)?;
        let retains = |template, instance| {
            let mut receipts = program
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == instance);
            receipts.next().is_some_and(|receipt| {
                receipt.template == template && receipt.binding_application.is_valid()
            }) && receipts.next().is_none()
        };
        if closed.supply_mode != MachineSupplyMode::TopLevelRequirement
            || closed.has_specialization_parameters()
            || !retains(requirement.symbol, closed.symbol)
            || !retains(row.provider_template, provider.symbol)
        {
            return Err(failure());
        }
        return Ok(closed);
    }
    let operator = program
        .machine_token_binding_by_symbol(requirement.symbol)
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
                .machine(receipt.instance)
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
    // incompatible origins. The receipt already names this requirement as its
    // template symbol; the retained tuple must also name the application
    // reconstructed from its signature.
    if program
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == closed.symbol)
        .count()
        != 1
        || closed_receipt.template_parameters != requirement.type_parameters
        || !retains_application_tuple(program, closed_receipt, application)
    {
        return Err(failure());
    }
    Ok(closed)
}

fn retains_application_tuple(
    program: &dyn crate::CheckingInput,
    receipt: &MachineSpecialization,
    application: &ClosedOperatorRealizationApplication,
) -> bool {
    let mut types = receipt.type_argument_identities.iter();
    let mut constants = receipt.const_argument_identities.iter();
    for argument in &application.arguments {
        let matches = match argument {
            ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                types.next().is_some_and(|identity| {
                    *identity == program.normalized_type_identity_text(*type_reference)
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
                        *identity == program.normalized_type_identity_text(reference)
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
    program: &dyn crate::CheckingInput,
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

/// Rejoin a prepared exact trait edge to its owned closed public header.
/// The retained preparation coordinate selects a contract, never a provider
/// or conformance proof. Ordinary signature/body checking consumes the result.
pub(crate) fn resolve_trait<'program>(
    program: &'program dyn crate::CheckingInput,
    provider: &Machine,
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    requirement: &typed_trees::signature::StateSignature,
    conformance: arena::Handle<typed_trees::machine::TraitConformance>,
) -> Result<Option<&'program typed_trees::signature::StateSignature>, Diagnostic> {
    let failure = || {
        Diagnostic::error(
            "declared trait provider lost its unique exact prepared requirement application",
        )
    };
    let mut selected = None;
    for (_, row) in program
        .instantiated_requirement_realizations
        .iter()
        .filter(|(_, row)| {
            row.provider_instance == provider.symbol
                && row.requirement_template == requirement.symbol
                && row.instance_conformance == conformance
        })
    {
        let template = program.machine(row.provider_template).ok_or_else(failure)?;
        let edge = program
            .machine_trait_conformances(template)
            .iter()
            .enumerate()
            .find(|(ordinal, _)| template.satisfies.handle_at(*ordinal as u32) == row.conformance)
            .map(|(_, edge)| edge)
            .ok_or_else(failure)?;
        let instance_edge = program
            .machine_trait_conformances(provider)
            .iter()
            .enumerate()
            .find(|(ordinal, _)| provider.satisfies.handle_at(*ordinal as u32) == conformance)
            .map(|(_, edge)| edge)
            .ok_or_else(failure)?;
        if edge.requirement_symbol != requirement.symbol || edge != instance_edge {
            return Err(failure());
        }
        let Some(retained_scope) =
            typed_trees::trait_definition::AppliedTraitScope::from_machine_conformance(
                program.typed_program(),
                template,
                edge,
            )
            .and_then(|scope| {
                scope.declaring_scope(program.typed_program(), application.declaration.symbol)
            })
        else {
            return Err(failure());
        };
        if !retained_scope.same_application(program.typed_program(), application) {
            continue;
        }
        let mut headers =
            program
                .instantiated_requirement_signatures
                .iter()
                .filter(|(_, header)| {
                    header.declaring_trait == application.declaration.symbol
                        && header.requirement_template == requirement.symbol
                        && program
                            .trait_machine_signatures
                            .get(header.signature)
                            .symbol
                            == row.requirement_instance
                });
        let (_, header) = headers.next().ok_or_else(failure)?;
        if headers.next().is_some() || !header.binding_application.is_valid() {
            return Err(failure());
        }
        if row.provider_template != row.provider_instance {
            let mut receipts = program
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == row.provider_instance);
            if !receipts.next().is_some_and(|receipt| {
                receipt.template == row.provider_template && receipt.binding_application.is_valid()
            }) || receipts.next().is_some()
            {
                return Err(failure());
            }
        }
        let signature = program.trait_machine_signatures.get(header.signature);
        if !signature.type_parameters.is_empty() || selected.is_some() {
            return Err(failure());
        }
        selected = Some(signature);
    }
    Ok(selected)
}
