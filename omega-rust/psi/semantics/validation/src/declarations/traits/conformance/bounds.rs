//! Generic and trait conformance bounds: the requirement a bound states,
//! whether an argument meets it, and the validation of every bound a
//! conformance declares.

use crate::declarations::traits::conformance::signature_matching::{
    TraitTypeBinding, TraitTypeBindingTarget, type_references_match_with_trait_bindings,
};
use crate::declarations::traits::conformance::trait_applications::validate_trait_application_obligations;
use crate::declarations::traits::trait_definition_by_symbol;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) struct GenericBoundRequirement<'program> {
    pub(crate) signature: &'program StateSignature,
    pub(crate) trait_definition: &'program TraitDefinition,
    pub(crate) bound: &'program typed_trees::machine::GenericConformanceBound,
}

pub(crate) fn generic_bound_argument_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    receiver: TypeReferenceHandle,
    requirement: &GenericBoundRequirement<'_>,
) -> bool {
    bound_argument_matches(
        program,
        actual,
        required,
        TraitTypeBindingTarget::Type(receiver),
        requirement,
    )
}

pub(crate) fn named_conformance_argument_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    requirement: &GenericBoundRequirement<'_>,
) -> bool {
    bound_argument_matches(
        program,
        actual,
        required,
        TraitTypeBindingTarget::Parameter(requirement.bound.subject),
        requirement,
    )
}

fn bound_argument_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    subject: TraitTypeBindingTarget,
    requirement: &GenericBoundRequirement<'_>,
) -> bool {
    let mut bindings = vec![TraitTypeBinding {
        parameter_symbol: SymbolHandle::invalid(),
        parameter_name: "Self".to_owned(),
        target: subject,
    }];
    bindings.extend(
        program
            .trait_type_parameters(requirement.trait_definition)
            .iter()
            .zip(requirement.bound.arguments.iter())
            .map(|(parameter, argument)| TraitTypeBinding {
                parameter_symbol: parameter.symbol,
                parameter_name: parameter.name.to_string(),
                target: TraitTypeBindingTarget::Type(*argument),
            }),
    );
    type_references_match_with_trait_bindings(
        program,
        actual,
        required,
        program.trait_type_parameters(requirement.trait_definition),
        &mut bindings,
    )
}

pub(crate) fn generic_bound_requirement_call<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Result<Option<GenericBoundRequirement<'program>>, String> {
    let Some(subject) = generic_subject_symbol(program, receiver_type) else {
        return Ok(None);
    };
    let is_type_parameter = program
        .machine_type_parameters(machine)
        .iter()
        .any(|parameter| parameter.symbol == subject);
    if !is_type_parameter {
        return Ok(None);
    }

    let bounds = machine
        .conformance_bounds
        .iter()
        .filter(|bound| bound.subject == subject)
        .collect::<Vec<_>>();
    if bounds.is_empty() {
        let subject_name = program.symbols.name(subject);
        return Err(format!(
            "machine `{}` cannot call `{target}` through unconstrained generic parameter `{subject_name}`; add `where {subject_name} satisfies Trait`",
            machine.name,
        ));
    }

    let mut trait_names = Vec::new();
    let mut requirements = Vec::new();
    for bound in &bounds {
        let trait_definition = if let Some(selected) = bound.selected_conformance_symbol() {
            let declaration = program
                .conformances()
                .iter()
                .find(|declaration| declaration.symbol == selected)
                .ok_or_else(|| {
                    format!(
                        "machine `{}` generic call uses unresolved conformance `{}::{}`",
                        machine.name,
                        bound.carrier_name,
                        bound
                            .selected_conformance_name()
                            .map_or("<missing>", |name| name.as_str())
                    )
                })?;
            program
                .traits()
                .iter()
                .find(|candidate| candidate.name == declaration.trait_name)
        } else {
            trait_definition_by_symbol(program, bound.carrier)
        }
        .ok_or_else(|| {
            format!(
                "machine `{}` generic call has no resolved trait for bound on `{}`",
                machine.name, bound.subject_name
            )
        })?;
        trait_names.push(trait_definition.name.as_str());
        requirements.extend(
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .filter(|requirement| requirement.name.as_str() == target)
                .map(|signature| GenericBoundRequirement {
                    signature,
                    trait_definition,
                    bound,
                }),
        );
    }

    let Some(requirement) = requirements.pop() else {
        let subject_name = bounds[0].subject_name.as_str();
        if trait_names.len() == 1 {
            return Err(format!(
                "machine `{}` generic parameter `{subject_name}` is bounded by trait `{}`, which has no requirement `{target}`",
                machine.name, trait_names[0],
            ));
        }
        return Err(format!(
            "machine `{}` generic parameter `{subject_name}` is bounded by traits {}, none of which has requirement `{target}`",
            machine.name,
            trait_names
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    };
    if !requirements.is_empty() {
        return Err(format!(
            "machine `{}` generic call `{target}` is ambiguous across its conformance bounds",
            machine.name,
        ));
    }
    Ok(Some(requirement))
}

fn generic_subject_symbol(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => generic_subject_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            generic_subject_symbol(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        _ => None,
    }
}

pub(crate) fn validate_generic_conformance_bounds(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_conformance_bounds(
        program,
        "machine",
        machine.name.as_str(),
        &machine.conformance_bounds,
        diagnostics,
    );
}

pub(crate) fn validate_trait_conformance_bounds(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_conformance_bounds(
        program,
        "trait",
        trait_definition.name.as_str(),
        &trait_definition.conformance_bounds,
        diagnostics,
    );
}

fn validate_conformance_bounds(
    program: &TypedTrees,
    owner_kind: &str,
    owner_name: &str,
    bounds: &[typed_trees::machine::GenericConformanceBound],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bound in bounds {
        if !bound.subject.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound names unknown type parameter `{}`",
                bound.subject_name
            )));
            continue;
        }

        if let Some(selected) = bound.selected_conformance_symbol() {
            let Some(declaration) = program
                .conformances()
                .iter()
                .find(|declaration| declaration.symbol == selected)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner_kind} `{owner_name}` conformance bound selects unknown conformance `{}::{}`",
                    bound.carrier_name,
                    bound
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str())
                )));
                continue;
            };
            let carrier_is_application_parameter = program
                .conformance_type_parameters(declaration)
                .iter()
                .any(|parameter| parameter.symbol == declaration.carrier_symbol);
            if declaration.carrier_symbol != bound.carrier && !carrier_is_application_parameter {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner_kind} `{owner_name}` names conformance `{}::{}`, but that declaration belongs to `{}`",
                    bound.carrier_name,
                    bound
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str()),
                    declaration
                        .carrier_name()
                        .map_or("<subjectless>", |name| name.as_str()),
                )));
            }
            continue;
        }

        if bound.selected_conformance.is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound selects unknown conformance `{}::{}`",
                bound.carrier_name,
                bound
                    .selected_conformance_name()
                    .map_or("<missing>", |name| name.as_str())
            )));
            continue;
        }

        let Some(trait_definition) = trait_definition_by_symbol(program, bound.carrier) else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound names unknown trait `{}`",
                bound.carrier_name
            )));
            continue;
        };
        let expected = program.trait_type_parameters(trait_definition).len();
        if bound.arguments.len() != expected {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound for trait `{}` expects {expected} generic argument(s), got {}",
                bound.carrier_name,
                bound.arguments.len(),
            )));
            continue;
        }

        validate_trait_application_obligations(
            program,
            trait_definition,
            &bound.arguments,
            bounds,
            &format!(
                "{owner_kind} `{owner_name}` conformance bound `{} satisfies {}`",
                bound.subject_name, bound.carrier_name
            ),
            diagnostics,
        );
    }
}

pub(crate) fn bound_proves_trait_application(
    program: &TypedTrees,
    bound: &typed_trees::machine::GenericConformanceBound,
    required_trait: &TraitDefinition,
    required_arguments: &[TypeReferenceHandle],
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
) -> bool {
    if let Some(selected) = bound.selected_conformance_symbol() {
        return program
            .conformances()
            .iter()
            .find(|declaration| declaration.symbol == selected)
            .is_some_and(|declaration| {
                declaration.trait_name == required_trait.name
                    && conformance_arguments_match_application(
                        program,
                        program
                            .type_reference_table
                            .type_reference_handles(declaration.arguments),
                        required_arguments,
                        applied_trait,
                        applied_arguments,
                    )
            });
    }
    bound.carrier == required_trait.symbol
        && conformance_arguments_match_application(
            program,
            &bound.arguments,
            required_arguments,
            applied_trait,
            applied_arguments,
        )
}

pub(crate) fn conformance_arguments_match_application(
    program: &TypedTrees,
    actual: &[TypeReferenceHandle],
    required: &[TypeReferenceHandle],
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
) -> bool {
    let parameters = program.trait_type_parameters(applied_trait);
    let mut bindings = parameters
        .iter()
        .zip(applied_arguments.iter())
        .map(|(parameter, argument)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(*argument),
        })
        .collect::<Vec<_>>();
    actual.len() == required.len()
        && actual
            .iter()
            .zip(required.iter())
            .all(|(actual, required)| {
                type_references_match_with_trait_bindings(
                    program,
                    *actual,
                    *required,
                    parameters,
                    &mut bindings,
                )
            })
}

pub(crate) fn generic_argument_symbol(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => generic_argument_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            generic_argument_symbol(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. }
            if symbol.is_valid()
                && matches!(
                    program.symbols.get(*symbol).kind,
                    symbols::SymbolKind::TypeParameter
                ) =>
        {
            Some(*symbol)
        }
        _ => None,
    }
}

pub(crate) fn concrete_data_type_name(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&str> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            concrete_data_type_name(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && matches!(program.symbols.get(*symbol).kind, symbols::SymbolKind::Data) =>
        {
            Some(name.as_str())
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } if base_symbol.is_valid()
            && matches!(
                program.symbols.get(*base_symbol).kind,
                symbols::SymbolKind::Data
            ) =>
        {
            Some(base_name.as_str())
        }
        _ => None,
    }
}

pub(crate) fn trait_application_label(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    arguments: &[TypeReferenceHandle],
) -> String {
    if arguments.is_empty() {
        return trait_definition.name.to_string();
    }
    format!(
        "{}<{}>",
        trait_definition.name,
        arguments
            .iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
