//! Trait application obligations: machine declaration identity arguments,
//! proposition family arguments and indexed carrier parameters. Every source
//! use, including inherited headers and unused type applications, shares kind,
//! property and callable admission; closed representation is not that judgment.

use crate::declarations::traits::conformance::signature_matching::{
    carrier_data_definition_for_argument, required_trait_type_parameter, trait_type_bindings,
    type_references_match_for_trait_bindings,
};
use crate::value_custody::type_references::type_references_match;
use diagnostics::Diagnostic;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Admit the complete argument roster of a trait application in its caller's
/// lexical scope, including unused binders and applications stored in fields.
/// Binder kind, declared properties and callable/evidence contracts remain
/// obligations independently of closedness or runtime carrier equality.
pub(crate) fn validate_trait_application_obligations(
    program: &dyn crate::CheckingInput,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    applied_lifetime_arguments: &[typed_trees::name::Identifier],
    application_label: &str,
    symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
    caller_scope: crate::value_custody::type_references::TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program.trait_type_parameters(applied_trait);
    if applied_arguments.len() != parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "{application_label} expects {} generic arguments, got {}",
            parameters.len(),
            applied_arguments.len()
        )));
        return;
    }

    use crate::value_custody::type_references::{
        AppliedValueCarrier, TypeReferenceOwner, validate_const_application_argument,
        validate_machine_application_argument, validate_parameter_bounds,
        validate_type_reference_handle,
    };
    validate_parameter_bounds(
        program,
        applied_trait.name.as_str(),
        parameters,
        applied_arguments,
        caller_scope,
        diagnostics,
    );
    for (parameter, argument) in parameters.iter().zip(applied_arguments) {
        match &parameter.kind {
            TypeParameterKind::Type => validate_type_reference_handle(
                program,
                *argument,
                symbols,
                diagnostics,
                TypeReferenceOwner::GenericBinder {
                    owner: application_label,
                    parameter: parameter.name.as_str(),
                    generic_depth: 0,
                },
                caller_scope,
            ),
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => validate_const_application_argument(
                program,
                applied_trait.name.as_str(),
                parameter,
                AppliedValueCarrier::applied(*type_reference, parameters, applied_arguments),
                *argument,
                caller_scope,
                diagnostics,
            ),
            TypeParameterKind::Machine { contract }
                if !matches!(
                    contract,
                    typed_trees::data::MachineParameterContract::RequirementIdentity
                ) =>
            {
                validate_machine_application_argument(
                    program,
                    applied_trait.symbol,
                    applied_trait.name.as_str(),
                    parameter,
                    contract,
                    *argument,
                    caller_scope,
                    diagnostics,
                )
            }
            TypeParameterKind::Implementation
            | TypeParameterKind::Proposition { .. }
            | TypeParameterKind::Machine { .. } => {}
        }
    }
    super::super::trait_implementation_arguments::validate_trait_implementation_application(
        program,
        applied_trait,
        applied_arguments,
        applied_lifetime_arguments,
        caller_scope.lexical_owner,
        application_label,
        diagnostics,
    );

    validate_machine_declaration_identity_arguments(
        program,
        applied_trait,
        applied_arguments,
        application_label,
        diagnostics,
    );
    validate_proposition_family_arguments(
        program,
        applied_trait,
        applied_arguments,
        application_label,
        diagnostics,
    );
}

fn validate_machine_declaration_identity_arguments(
    program: &dyn crate::CheckingInput,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter, argument) in program
        .trait_type_parameters(applied_trait)
        .iter()
        .zip(applied_arguments)
    {
        let TypeParameterKind::Machine {
            contract: typed_trees::data::MachineParameterContract::RequirementIdentity,
        } = &parameter.kind
        else {
            continue;
        };
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(*argument)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} declaration-identity argument `{}` must be one exact machine or trait-requirement name",
                parameter.name,
            )));
            continue;
        };

        if symbol.is_valid()
            && program.symbols.get(*symbol).kind == symbols::SymbolKind::MachineParameter
            && program.data_type_parameters.iter().any(|(_, candidate)| {
                candidate.symbol == *symbol
                    && matches!(
                        candidate.kind,
                        TypeParameterKind::Machine {
                            contract:
                                typed_trees::data::MachineParameterContract::RequirementIdentity
                        }
                    )
            })
        {
            continue;
        }

        if !symbol.is_valid() || program.symbols.get(*symbol).kind != symbols::SymbolKind::State {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} argument `{name}` for declaration-identity parameter `{}` is not an exact machine or trait requirement",
                parameter.name,
            )));
            continue;
        }

        // Trait requirement paths are signature-free. An overload addition
        // therefore invalidates every such identity use rather than allowing
        // an expected call shape or a visible satisfier to choose one row.
        if let Some((declaring_trait, selected_requirement)) =
            program.trait_machine_signature(*symbol)
        {
            let same_name_count = program
                .trait_machine_signatures(declaring_trait)
                .iter()
                .filter(|requirement| requirement.name == selected_requirement.name)
                .count();
            if same_name_count != 1 {
                diagnostics.push(Diagnostic::error(format!(
                    "{application_label} declaration-identity argument `{name}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                )));
            }
        }
    }
}

fn validate_proposition_family_arguments(
    program: &dyn crate::CheckingInput,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program.trait_type_parameters(applied_trait);
    let mut bindings = trait_type_bindings(parameters.iter(), applied_arguments.iter());

    for (parameter, argument) in parameters.iter().zip(applied_arguments) {
        let TypeParameterKind::Proposition { contract: expected } = &parameter.kind else {
            continue;
        };
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(*argument)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition-family argument `{}` must be a direct proposition name",
                parameter.name,
            )));
            continue;
        };
        let actual_declaration = if symbol.is_valid()
            && program.symbols.get(*symbol).kind == symbols::SymbolKind::Proposition
        {
            program.proposition_definition_by_symbol(*symbol)
        } else {
            None
        };
        let actual_parameters = if let Some(declaration) = actual_declaration {
            Some(program.proposition_parameters(declaration))
        } else if symbol.is_valid()
            && program.symbols.get(*symbol).kind == symbols::SymbolKind::PropositionParameter
        {
            program
                .data_type_parameters
                .iter()
                .map(|(_, parameter)| parameter)
                .find_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Proposition { contract } if parameter.symbol == *symbol => {
                        Some(program.state_parameters.span_or_empty(contract.parameters))
                    }
                    _ => None,
                })
        } else {
            None
        };
        let Some(actual_parameters) = actual_parameters else {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} argument `{}` for proposition parameter `{}` is not a proposition family",
                name, parameter.name,
            )));
            continue;
        };
        let expected_parameters = program.state_parameters.span_or_empty(expected.parameters);
        if actual_parameters.len() != expected_parameters.len() {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition family `{name}` has {} value parameter(s), but `{}` requires {}",
                actual_parameters.len(),
                parameter.name,
                expected_parameters.len(),
            )));
            continue;
        }
        let actual_binders = actual_declaration
            .map(|declaration| program.proposition_binders(declaration))
            .unwrap_or(&[]);
        let mut indexed_binder_offset = 0usize;
        let mut used_indexed_carrier = false;
        for (index, (actual, expected)) in actual_parameters
            .iter()
            .zip(expected_parameters)
            .enumerate()
        {
            let indexed_match = if actual_binders.is_empty() {
                None
            } else {
                indexed_carrier_parameter_matches(
                    program,
                    expected.type_reference,
                    actual.type_reference,
                    parameters,
                    applied_arguments,
                    actual_binders,
                    &mut indexed_binder_offset,
                )
            };
            used_indexed_carrier |= indexed_match.is_some();
            let matches = indexed_match.unwrap_or_else(|| {
                type_references_match_for_trait_bindings(
                    program,
                    actual.type_reference,
                    expected.type_reference,
                    parameters,
                    &mut bindings,
                )
            });
            if !matches {
                let expected_type =
                    required_trait_type_parameter(program, expected.type_reference, parameters)
                        .and_then(|expected_parameter| {
                            parameters
                                .iter()
                                .position(|candidate| candidate.symbol == expected_parameter.symbol)
                        })
                        .and_then(|index| applied_arguments.get(index).copied())
                        .map(|argument| program.display_type_reference(argument))
                        .unwrap_or_else(|| program.display_type_reference(expected.type_reference));
                diagnostics.push(Diagnostic::error(format!(
                    "{application_label} proposition family `{name}` value parameter {} has type `{}`, but `{}` requires `{}` after substitution",
                    index + 1,
                    program.display_type_reference(actual.type_reference),
                    parameter.name,
                    expected_type,
                )));
            }
        }
        if used_indexed_carrier && indexed_binder_offset != actual_binders.len() {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition family `{name}` has {} proof-static binder(s), but the substituted carrier telescopes consume {}",
                actual_binders.len(),
                indexed_binder_offset,
            )));
        }
    }
}

fn indexed_carrier_parameter_matches(
    program: &dyn crate::CheckingInput,
    expected: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    trait_parameters: &[TypeParameter],
    applied_arguments: &[TypeReferenceHandle],
    proposition_binders: &[typed_trees::proposition::PropositionBinder],
    binder_offset: &mut usize,
) -> Option<bool> {
    let carrier_parameter = required_trait_type_parameter(program, expected, trait_parameters)?;
    let carrier = carrier_data_definition_for_argument(
        program,
        trait_parameters,
        applied_arguments,
        carrier_parameter,
    )?;
    let carrier_telescope = program.data_type_parameters(carrier);
    if carrier_telescope.is_empty() {
        return None;
    }

    let start = *binder_offset;
    let end = start.saturating_add(carrier_telescope.len());
    *binder_offset = end;
    let Some(binder_group) = proposition_binders.get(start..end) else {
        return Some(false);
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program.type_reference_table.type_reference(actual)
    else {
        return Some(false);
    };
    if *base_symbol != carrier.symbol {
        return Some(false);
    }
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    if arguments.len() != binder_group.len() {
        return Some(false);
    }

    Some(
        carrier_telescope
            .iter()
            .zip(binder_group)
            .zip(arguments)
            .all(|((carrier_parameter, proposition_binder), argument)| {
                let kind_matches = match (&carrier_parameter.kind, &proposition_binder.kind) {
                    (
                        TypeParameterKind::Type,
                        typed_trees::proposition::PropositionBinderKind::Type,
                    )
                    | (
                        TypeParameterKind::Machine { .. },
                        typed_trees::proposition::PropositionBinderKind::Machine,
                    ) => true,
                    (
                        TypeParameterKind::Const {
                            type_reference: carrier_type,
                        }
                        | TypeParameterKind::Value {
                            type_reference: carrier_type,
                        },
                        typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference: binder_type,
                        },
                    ) => type_references_match(program, *binder_type, *carrier_type),
                    _ => false,
                };
                kind_matches
                    && matches!(
                        program.type_reference_table.type_reference(*argument),
                        TypeReferenceNode::Named { symbol, name }
                            if ((*symbol).is_valid() && *symbol == proposition_binder.symbol)
                                || name == &proposition_binder.name
                    )
            }),
    )
}
