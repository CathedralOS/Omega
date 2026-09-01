//! Validation of normalized, extension-local generic data instances.

use super::exact_top_level_data_symbol;
use psi_symbol_resolved_trees::{SymbolResolvedTrees, types::TypeReference};
use psi_symbols::SymbolHandle;

mod reachability;
mod substitution;

pub(super) fn template_application_is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    owner: SymbolHandle,
    owner_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    application: &psi_symbol_resolved_trees::types::GenericTypeReference,
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
        && application.lifetime_arguments.is_empty()
        && template.lifetime_parameters.is_empty()
        && template.generic_instance.is_none()
        && exact_top_level_data_symbol(source, template)
        && !parameters.is_empty()
        && parameters.len() == arguments.len()
        && parameters.iter().all(|parameter| {
            parameter.symbol.is_valid()
                && source.symbols.get(parameter.symbol).kind
                    == psi_symbols::SymbolKind::TypeParameter
                && source.symbols.get(parameter.symbol).parent == template.symbol
                && source.symbols.name(parameter.symbol) == parameter.name.as_str()
                && matches!(
                    parameter.kind,
                    psi_symbol_resolved_trees::data::TypeParameterKind::Type
                )
        })
        && arguments.iter().all(|argument| {
            let TypeReference::Named { symbol, name } = argument else {
                return false;
            };
            if !symbol.is_valid() || source.symbols.name(*symbol) != name.as_str() {
                return false;
            }
            match source.symbols.get(*symbol).kind {
                psi_symbols::SymbolKind::TypeParameter => {
                    source.symbols.get(*symbol).parent == owner
                        && owner_type_parameters
                            .iter()
                            .any(|parameter| parameter.symbol == *symbol)
                }
                psi_symbols::SymbolKind::BuiltinType => true,
                psi_symbols::SymbolKind::Data => source
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
        })
}

/// Reconstruct every admitted local instance independently and return the exact
/// instance symbols that ordinary generated-data validation may reference.
///
/// No fixed declaration or use count belongs here. The normalizer's retained
/// origin is sufficient to rejoin each instance to one same-unit template,
/// replay Type-parameter substitution through fields or case payloads, and require at least one
/// ordinary-data use. Unsupported synthesis shapes reject the whole candidate
/// transactionally.
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
    instance: &psi_symbol_resolved_trees::data::DataDefinition,
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
        || !template.lifetime_parameters.is_empty()
        || !instance.lifetime_parameters.is_empty()
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
        let TypeReference::Named {
            symbol: argument_symbol,
            name: argument_name,
        } = argument
        else {
            return false;
        };
        if !parameter.symbol.is_valid()
            || source.symbols.get(parameter.symbol).kind != psi_symbols::SymbolKind::TypeParameter
            || source.symbols.get(parameter.symbol).parent != template.symbol
            || source.symbols.name(parameter.symbol) != parameter.name.as_str()
            || !matches!(
                parameter.kind,
                psi_symbol_resolved_trees::data::TypeParameterKind::Type
            )
            || !supported_named_argument(
                source,
                validated_instances,
                *argument_symbol,
                argument_name.as_str(),
            )
        {
            return false;
        }
        substitutions.push((parameter.symbol, argument));
        argument_names.push(argument_name.as_str());
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
        psi_symbols::SymbolKind::BuiltinType => true,
        psi_symbols::SymbolKind::Data => source
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
