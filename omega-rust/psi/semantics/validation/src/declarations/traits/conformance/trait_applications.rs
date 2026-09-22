//! Trait application obligations: machine declaration identity arguments,
//! proposition family arguments and indexed carrier parameters.

use crate::declarations::traits::conformance::bounds::{
    bound_proves_trait_application, concrete_data_type_name,
    conformance_arguments_match_application, generic_argument_symbol, trait_application_label,
};
use crate::declarations::traits::conformance::signature_matching::{
    TraitTypeBinding, TraitTypeBindingTarget, required_trait_type_parameter,
    type_references_match_with_trait_bindings,
};
use crate::declarations::traits::trait_definition_by_symbol;
use crate::value_custody::type_references::type_references_match;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Validate the obligations declared by a generic trait header at one use of
/// that trait. For example, applying `Calling<C>` also proves the declaration
/// site constraint from `trait Calling<C> where C satisfies CallingPolicy`.
///
/// `available_bounds` is the enclosing generic owner's evidence. Concrete
/// arguments instead discharge obligations through standalone nominal
/// conformance items. This keeps header constraints static and dictionary-free
/// while rejecting an invalid relationship at the site that authored it.
pub(crate) fn validate_trait_application_obligations(
    program: &TypedTrees,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    available_bounds: &[typed_trees::machine::GenericConformanceBound],
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program.trait_type_parameters(applied_trait);
    if applied_arguments.len() != parameters.len() {
        return;
    }

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

    for obligation in &applied_trait.conformance_bounds {
        let Some(subject_index) = parameters.iter().position(|parameter| {
            (parameter.symbol.is_valid() && parameter.symbol == obligation.subject)
                || parameter.name == obligation.subject_name
        }) else {
            // Declaration validation reports an unknown header subject.
            continue;
        };
        let actual_subject = applied_arguments[subject_index];

        if let Some(selected) = obligation.selected_conformance_symbol() {
            let Some(declaration) = program
                .conformances()
                .iter()
                .find(|declaration| declaration.symbol == selected)
            else {
                // Declaration validation reports the unresolved selection.
                continue;
            };
            let has_exact_evidence =
                generic_argument_symbol(program, actual_subject).is_some_and(|subject_symbol| {
                    available_bounds.iter().any(|candidate| {
                        candidate.subject == subject_symbol
                            && candidate.selected_conformance_symbol() == Some(selected)
                            && candidate.selected_conformance == obligation.selected_conformance
                    })
                }) || declaration.carrier_name().is_some_and(|carrier| {
                    concrete_data_type_name(program, actual_subject) == Some(carrier.as_str())
                });
            if !has_exact_evidence {
                diagnostics.push(Diagnostic::error(format!(
                    "{application_label} does not meet trait `{}` header obligation `{}` satisfies exact conformance `{}::{}`: argument `{}` is not data `{}`",
                    applied_trait.name,
                    obligation.subject_name,
                    obligation.carrier_name,
                    obligation
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str()),
                    program.display_type_reference(actual_subject),
                    declaration
                        .carrier_name()
                        .map_or("<subjectless>", |name| name.as_str()),
                )));
            }
            continue;
        }

        let Some(required_trait) = trait_definition_by_symbol(program, obligation.carrier) else {
            // Declaration validation reports the unresolved trait.
            continue;
        };
        if obligation.arguments.len() != program.trait_type_parameters(required_trait).len() {
            // Declaration validation reports the arity mismatch.
            continue;
        }
        let evidence_count =
            if let Some(subject_symbol) = generic_argument_symbol(program, actual_subject) {
                available_bounds
                    .iter()
                    .filter(|candidate| {
                        candidate.subject == subject_symbol
                            && bound_proves_trait_application(
                                program,
                                candidate,
                                required_trait,
                                &obligation.arguments,
                                applied_trait,
                                applied_arguments,
                            )
                    })
                    .count()
            } else if let Some(type_name) = concrete_data_type_name(program, actual_subject) {
                program
                    .conformances()
                    .iter()
                    .filter(|candidate| {
                        candidate
                            .carrier_name()
                            .is_some_and(|carrier| carrier.as_str() == type_name)
                            && candidate.trait_name == required_trait.name
                            && conformance_arguments_match_application(
                                program,
                                program
                                    .type_reference_table
                                    .type_reference_handles(candidate.arguments),
                                &obligation.arguments,
                                applied_trait,
                                applied_arguments,
                            )
                    })
                    .count()
            } else {
                0
            };

        match evidence_count {
            1 => {}
            0 => diagnostics.push(Diagnostic::error(format!(
                "{application_label} does not meet trait `{}` header obligation `{} satisfies {}` for argument `{}`",
                applied_trait.name,
                obligation.subject_name,
                trait_application_label(program, required_trait, &obligation.arguments),
                program.display_type_reference(actual_subject),
            ))),
            count => diagnostics.push(Diagnostic::error(format!(
                "{application_label} has {count} matching conformances for trait `{}` header obligation `{} satisfies {}`; select one exact named conformance",
                applied_trait.name,
                obligation.subject_name,
                trait_application_label(program, required_trait, &obligation.arguments),
            ))),
        }
    }
}

fn validate_machine_declaration_identity_arguments(
    program: &TypedTrees,
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
            program.traits().iter().find_map(|trait_definition| {
                program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|requirement| requirement.symbol == *symbol)
                    .map(|requirement| (trait_definition, requirement))
            })
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
    program: &TypedTrees,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program.trait_type_parameters(applied_trait);
    let mut bindings = parameters
        .iter()
        .zip(applied_arguments)
        .map(|(parameter, argument)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(*argument),
        })
        .collect::<Vec<_>>();

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
            program
                .propositions()
                .iter()
                .find(|proposition| proposition.symbol == *symbol)
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
                type_references_match_with_trait_bindings(
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
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    trait_parameters: &[TypeParameter],
    applied_arguments: &[TypeReferenceHandle],
    proposition_binders: &[typed_trees::proposition::PropositionBinder],
    binder_offset: &mut usize,
) -> Option<bool> {
    let carrier_parameter = required_trait_type_parameter(program, expected, trait_parameters)?;
    let carrier_index = trait_parameters
        .iter()
        .position(|parameter| parameter.symbol == carrier_parameter.symbol)?;
    let carrier_argument = *applied_arguments.get(carrier_index)?;
    let TypeReferenceNode::Named {
        symbol: carrier_symbol,
        ..
    } = program
        .type_reference_table
        .type_reference(carrier_argument)
    else {
        return None;
    };
    let carrier = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *carrier_symbol)?;
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
