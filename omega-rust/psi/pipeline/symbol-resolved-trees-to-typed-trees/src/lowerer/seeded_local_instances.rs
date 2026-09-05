//! Validation of normalized, extension-local generic data instances.

use super::exact_top_level_data_symbol;
use symbol_resolved_trees::{SymbolResolvedTrees, types::TypeReference};
use symbols::SymbolHandle;

mod const_arguments;
mod reachability;
mod structured_const_arguments;
mod substitution;

pub(super) fn parameter_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    parameter: &symbol_resolved_trees::data::TypeParameter,
) -> bool {
    const_arguments::parameter_is_supported(source, owner, parameter)
}

pub(super) fn const_parameter_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    parameter: &symbol_resolved_trees::data::TypeParameter,
) -> bool {
    matches!(
        parameter.kind,
        symbol_resolved_trees::data::TypeParameterKind::Const { .. }
    ) && const_arguments::parameter_is_supported(source, owner, parameter)
}

pub(super) fn structured_const_parameter_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    parameter: &symbol_resolved_trees::data::TypeParameter,
) -> bool {
    const_parameter_is_supported(source, owner, parameter)
        && matches!(
            &parameter.kind,
            symbol_resolved_trees::data::TypeParameterKind::Const { type_reference }
                if structured_const_arguments::carrier_is_supported(source, type_reference)
        )
}

pub(super) fn const_declaration_is_supported(
    source: &SymbolResolvedTrees,
    declaration: &symbol_resolved_trees::constant::ConstDeclaration,
) -> bool {
    structured_const_arguments::declaration_is_supported(source, declaration)
}

pub(super) fn array_length_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    owner_parameters: &[symbol_resolved_trees::data::TypeParameter],
    length: &symbol_resolved_trees::types::FixedArrayLength,
) -> bool {
    const_arguments::array_length_is_supported(source, owner, owner_parameters, length)
}

pub(super) fn instance_application_is_supported(
    source: &SymbolResolvedTrees,
    validated_instances: &[SymbolHandle],
    owner_lifetimes: &[symbol_resolved_trees::name::DiagnosticName],
    application: &symbol_resolved_trees::types::GenericTypeReference,
) -> bool {
    if !validated_instances.contains(&application.base_symbol)
        || application.base_name.as_str() != source.symbols.name(application.base_symbol)
        || !source
            .child_type_references(application.arguments)
            .is_empty()
    {
        return false;
    }
    source
        .data_definitions
        .iter()
        .find(|definition| definition.symbol == application.base_symbol)
        .is_some_and(|definition| {
            definition.generic_instance.is_some()
                && exact_top_level_data_symbol(source, definition)
                && !definition.lifetime_parameters.is_empty()
                && definition.lifetime_parameters.len() == application.lifetime_arguments.len()
                && application.lifetime_arguments.iter().all(|argument| {
                    owner_lifetimes
                        .iter()
                        .any(|parameter| parameter.as_str() == argument.as_str())
                })
        })
}

pub(super) fn template_application_is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    owner: SymbolHandle,
    owner_lifetimes: &[symbol_resolved_trees::name::DiagnosticName],
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    application: &symbol_resolved_trees::types::GenericTypeReference,
) -> bool {
    let Some(owner_definition) = source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .find(|definition| definition.symbol == owner)
    else {
        return false;
    };
    let Some(template) = source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .find(|definition| definition.symbol == application.base_symbol)
    else {
        return false;
    };
    let parameters = source.data_type_parameters(template.type_parameters);
    let arguments = source.child_type_references(application.arguments);
    owner_definition.generic_instance.is_none()
        && exact_top_level_data_symbol(source, owner_definition)
        && !owner_type_parameters.is_empty()
        && source.data_type_parameters(owner_definition.type_parameters) == owner_type_parameters
        && application.base_name.as_str() == template.name.as_str()
        && application.lifetime_arguments.len() == template.lifetime_parameters.len()
        && application.lifetime_arguments.iter().all(|argument| {
            owner_lifetimes
                .iter()
                .any(|parameter| parameter.as_str() == argument.as_str())
        })
        && template.generic_instance.is_none()
        && exact_top_level_data_symbol(source, template)
        && !parameters.is_empty()
        && parameters.len() == arguments.len()
        && parameters
            .iter()
            .all(|parameter| parameter_is_supported(source, template.symbol, parameter))
        && parameters
            .iter()
            .zip(arguments)
            .all(|(parameter, argument)| {
                template_argument_is_supported(
                    source,
                    owner,
                    owner_type_parameters,
                    parameter,
                    argument,
                )
            })
}

/// Reconstruct every admitted local instance independently and return the exact
/// instance symbols that ordinary generated-data validation may reference.
///
/// No fixed declaration or use count belongs here. The normalizer's retained
/// origin is sufficient to rejoin each instance to one same-unit template,
/// replay Type and scalar-const substitution through fields or case payloads,
/// and require at least one ordinary-data use. Unsupported synthesis shapes
/// reject the whole candidate transactionally.
pub(super) fn validated_symbols(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
) -> Option<Vec<SymbolHandle>> {
    let instances = source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .filter(|definition| definition.generic_instance.is_some())
        .collect::<Vec<_>>();
    let mut symbols = Vec::with_capacity(instances.len());
    while symbols.len() < instances.len() {
        let before = symbols.len();
        for instance in &instances {
            if symbols.contains(&instance.symbol) {
                continue;
            }
            if validate_instance(source, data_frontier, instance, &symbols) {
                symbols.push(instance.symbol);
            }
        }
        if symbols.len() == before {
            return None;
        }
    }
    if reachability::all_instances_reachable_from_ordinary_data(source, data_frontier, &symbols) {
        Some(symbols)
    } else {
        None
    }
}

fn validate_instance(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    instance: &symbol_resolved_trees::data::DataDefinition,
    validated_instances: &[SymbolHandle],
) -> bool {
    let Some(TypeReference::Generic(origin)) = instance.generic_instance.as_ref() else {
        return false;
    };
    let Some(template) = source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .find(|definition| definition.symbol == origin.base_symbol)
    else {
        return false;
    };
    if template.symbol == instance.symbol
        || !origin.base_symbol.is_valid()
        || origin.base_name.as_str() != template.name.as_str()
        || !origin.lifetime_arguments.is_empty()
        || template.name.source_span().source_id != instance.name.source_span().source_id
        || template.lifetime_parameters != instance.lifetime_parameters
        || template.generic_instance.is_some()
        || !instance.type_parameters.is_empty()
        || template.quotient.is_some()
        || instance.quotient.is_some()
        || !template.where_facts.is_empty()
        || !instance.where_facts.is_empty()
        || template.zero_gated
        || instance.zero_gated
        || !exact_top_level_data_symbol(source, template)
        || !exact_top_level_data_symbol(source, instance)
        || template.is_public != instance.is_public
        || template.supply_mode != instance.supply_mode
        || template.properties != instance.properties
        || template.retired_identities != instance.retired_identities
    {
        return false;
    }

    let parameters = source.data_type_parameters(template.type_parameters);
    let arguments = source.child_type_references(origin.arguments);
    if parameters.is_empty() || parameters.len() != arguments.len() {
        return false;
    }
    let mut substitutions = Vec::with_capacity(parameters.len());
    let mut argument_names = Vec::with_capacity(arguments.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let Some(argument_name) = instance_argument_name(source, parameter, argument) else {
            return false;
        };
        if !parameter_is_supported(source, template.symbol, parameter)
            || !instance_argument_is_supported(
                source,
                validated_instances,
                &instance.lifetime_parameters,
                parameter,
                argument,
            )
        {
            return false;
        }
        substitutions.push((parameter.symbol, argument));
        argument_names.push(argument_name);
    }
    if instance.name.as_str()
        != format!("{}<{}>", template.name.as_str(), argument_names.join(", "))
    {
        return false;
    }

    let template_members = source.data_members(template.members);
    let instance_members = source.data_members(instance.members);
    template_members.len() == instance_members.len()
        && template_members.iter().zip(instance_members).all(
            |(template_member, instance_member)| {
                substitution::member_matches(
                    source,
                    template.symbol,
                    instance.symbol,
                    &substitutions,
                    validated_instances,
                    template_member,
                    instance_member,
                )
            },
        )
}

fn instance_argument_name(
    source: &SymbolResolvedTrees,
    parameter: &symbol_resolved_trees::data::TypeParameter,
    argument: &TypeReference,
) -> Option<String> {
    match (&parameter.kind, argument) {
        (
            symbol_resolved_trees::data::TypeParameterKind::Type,
            TypeReference::Named { name, .. },
        )
        | (
            symbol_resolved_trees::data::TypeParameterKind::Const { .. },
            TypeReference::Named { name, .. },
        ) => Some(name.as_str().to_owned()),
        (
            symbol_resolved_trees::data::TypeParameterKind::Type,
            TypeReference::Generic(application),
        ) if !application.lifetime_arguments.is_empty() => {
            Some(application.base_name.as_str().to_owned())
        }
        (
            symbol_resolved_trees::data::TypeParameterKind::Type,
            TypeReference::Constrained(constrained),
        ) => exact_constrained_argument(source, constrained).map(|argument| match argument {
            ExactConstrainedArgument::Arithmetic {
                carrier_name,
                domain,
            } => format!("{} in {}", carrier_name.as_str(), domain.name()),
            ExactConstrainedArgument::Declared {
                carrier_name,
                domain_name,
                ..
            } => format!("{} in {}", carrier_name.as_str(), domain_name.as_str()),
        }),
        _ => None,
    }
}

pub(super) fn template_argument_is_supported(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    parameter: &symbol_resolved_trees::data::TypeParameter,
    argument: &TypeReference,
) -> bool {
    match parameter.kind {
        symbol_resolved_trees::data::TypeParameterKind::Type => {
            let TypeReference::Named { symbol, name } = argument else {
                return false;
            };
            if !symbol.is_valid() || source.symbols.name(*symbol) != name.as_str() {
                return false;
            }
            match source.symbols.get(*symbol).kind {
                symbols::SymbolKind::TypeParameter => {
                    source.symbols.get(*symbol).parent == owner
                        && owner_type_parameters.iter().any(|candidate| {
                            candidate.symbol == *symbol
                                && matches!(
                                    candidate.kind,
                                    symbol_resolved_trees::data::TypeParameterKind::Type
                                )
                        })
                }
                symbols::SymbolKind::BuiltinType => true,
                symbols::SymbolKind::Data => source
                    .data_definitions
                    .iter()
                    .find(|definition| definition.symbol == *symbol)
                    .is_some_and(|definition| {
                        exact_top_level_data_symbol(source, definition)
                            && definition.lifetime_parameters.is_empty()
                            && definition.type_parameters.is_empty()
                            && definition.generic_instance.is_none()
                    }),
                _ => false,
            }
        }
        symbol_resolved_trees::data::TypeParameterKind::Const { .. } => {
            const_arguments::template_argument_is_supported(
                source,
                owner,
                owner_type_parameters,
                parameter,
                argument,
            )
        }
        symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
        | symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => false,
    }
}

fn instance_argument_is_supported(
    source: &SymbolResolvedTrees,
    validated_instances: &[SymbolHandle],
    owner_lifetimes: &[symbol_resolved_trees::name::DiagnosticName],
    parameter: &symbol_resolved_trees::data::TypeParameter,
    argument: &TypeReference,
) -> bool {
    match parameter.kind {
        symbol_resolved_trees::data::TypeParameterKind::Type => match argument {
            TypeReference::Named { symbol, name } => {
                supported_named_argument(source, validated_instances, *symbol, name.as_str())
            }
            TypeReference::Generic(application) => lifetime_instance_type_argument_is_supported(
                source,
                validated_instances,
                owner_lifetimes,
                application,
            ),
            TypeReference::Constrained(constrained) => {
                exact_constrained_argument(source, constrained).is_some_and(|argument| {
                    let TypeReference::Named { symbol, .. } =
                        source.child_type_reference(constrained.base_type)
                    else {
                        return false;
                    };
                    supported_named_argument(
                        source,
                        validated_instances,
                        *symbol,
                        argument.carrier_name().as_str(),
                    )
                })
            }
            _ => false,
        },
        symbol_resolved_trees::data::TypeParameterKind::Const { .. } => {
            const_arguments::closed_argument_is_supported(source, parameter, argument)
        }
        symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
        | symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactConstrainedArgument<'source> {
    Arithmetic {
        carrier_name: &'source symbol_resolved_trees::name::DiagnosticName,
        domain: numerics::arithmetic::ArithmeticDomain,
    },
    Declared {
        carrier_name: &'source symbol_resolved_trees::name::DiagnosticName,
        domain_name: &'source symbol_resolved_trees::name::DiagnosticName,
        domain_symbol: SymbolHandle,
    },
}

impl<'source> ExactConstrainedArgument<'source> {
    fn carrier_name(self) -> &'source symbol_resolved_trees::name::DiagnosticName {
        match self {
            Self::Arithmetic { carrier_name, .. } | Self::Declared { carrier_name, .. } => {
                carrier_name
            }
        }
    }
}

pub(super) fn exact_constrained_argument<'source>(
    source: &'source SymbolResolvedTrees,
    constrained: &symbol_resolved_trees::types::ConstrainedTypeReference,
) -> Option<ExactConstrainedArgument<'source>> {
    let TypeReference::Named { symbol, name } = source.child_type_reference(constrained.base_type)
    else {
        return None;
    };
    if !symbol.is_valid() || source.symbols.name(*symbol) != name.as_str() {
        return None;
    }
    let [constraint] = source
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints)
    else {
        return None;
    };
    match constraint {
        symbol_resolved_trees::types::TypeConstraint::ArithmeticDomain(domain) => {
            Some(ExactConstrainedArgument::Arithmetic {
                carrier_name: name,
                domain: *domain,
            })
        }
        symbol_resolved_trees::types::TypeConstraint::Domain(domain)
            if domain.arguments.is_empty() =>
        {
            let domain_symbol =
                exact_unindexed_domain_for_named_carrier(source, *symbol, name.as_str(), domain)?;
            Some(ExactConstrainedArgument::Declared {
                carrier_name: name,
                domain_name: &domain.name,
                domain_symbol,
            })
        }
        symbol_resolved_trees::types::TypeConstraint::Named(_)
        | symbol_resolved_trees::types::TypeConstraint::Range { .. }
        | symbol_resolved_trees::types::TypeConstraint::Domain(_) => None,
    }
}

fn exact_unindexed_domain_for_named_carrier(
    source: &SymbolResolvedTrees,
    carrier_symbol: SymbolHandle,
    carrier_name: &str,
    constraint: &symbol_resolved_trees::types::DomainConstraint,
) -> Option<SymbolHandle> {
    if !constraint.name.is_source_backed() || !constraint.arguments.is_empty() {
        return None;
    }
    let matches = source
        .domain_definitions
        .iter()
        .filter(|domain| {
            let declared_name = domain.name.as_str();
            let authored_name = constraint.name.as_str();
            let spelling_matches = declared_name == authored_name
                || declared_name.rsplit("::").next() == Some(authored_name);
            let target_matches = matches!(
                &domain.target_type,
                TypeReference::Named { symbol, name }
                    if *symbol == carrier_symbol && name.as_str() == carrier_name
            );
            domain.symbol.is_valid()
                && source.symbols.get(domain.symbol).kind == symbols::SymbolKind::Domain
                && source
                    .symbols
                    .source_reference_can_see_symbol(constraint.name.source_span(), domain.symbol)
                && spelling_matches
                && target_matches
                && domain.type_parameters.is_empty()
                && domain.index_arguments.is_empty()
                && domain.alias.is_none()
        })
        .map(|domain| domain.symbol)
        .collect::<Vec<_>>();
    let [domain] = matches.as_slice() else {
        return None;
    };
    Some(*domain)
}

fn lifetime_instance_type_argument_is_supported(
    source: &SymbolResolvedTrees,
    validated_instances: &[SymbolHandle],
    owner_lifetimes: &[symbol_resolved_trees::name::DiagnosticName],
    application: &symbol_resolved_trees::types::GenericTypeReference,
) -> bool {
    !owner_lifetimes.is_empty()
        && validated_instances.contains(&application.base_symbol)
        && application.base_symbol.is_valid()
        && application.base_name.as_str() == source.symbols.name(application.base_symbol)
        && source
            .child_type_references(application.arguments)
            .is_empty()
        && application.lifetime_arguments.len() == owner_lifetimes.len()
        && application
            .lifetime_arguments
            .iter()
            .zip(owner_lifetimes)
            .all(|(argument, owner)| argument.as_str() == owner.as_str())
        && source
            .data_definitions
            .iter()
            .find(|definition| definition.symbol == application.base_symbol)
            .is_some_and(|definition| {
                exact_top_level_data_symbol(source, definition)
                    && definition.generic_instance.is_some()
                    && definition.type_parameters.is_empty()
                    && !definition.lifetime_parameters.is_empty()
                    && definition.lifetime_parameters.len() == application.lifetime_arguments.len()
            })
}

fn supported_named_argument(
    source: &SymbolResolvedTrees,
    validated_instances: &[SymbolHandle],
    symbol: SymbolHandle,
    name: &str,
) -> bool {
    if !symbol.is_valid() || source.symbols.name(symbol) != name {
        return false;
    }
    match source.symbols.get(symbol).kind {
        symbols::SymbolKind::BuiltinType => true,
        symbols::SymbolKind::Data => source
            .data_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
            .is_some_and(|definition| {
                exact_top_level_data_symbol(source, definition)
                    && definition.lifetime_parameters.is_empty()
                    && definition.type_parameters.is_empty()
                    && (definition.generic_instance.is_none()
                        || validated_instances.contains(&definition.symbol))
            }),
        _ => false,
    }
}
