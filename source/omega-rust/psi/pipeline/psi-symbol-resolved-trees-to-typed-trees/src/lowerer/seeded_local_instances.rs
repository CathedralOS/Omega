//! Validation of normalized, extension-local generic record instances.

use super::{exact_field_symbol, exact_top_level_data_symbol};
use psi_symbol_resolved_trees::{SymbolResolvedTrees, types::TypeReference};
use psi_symbols::SymbolHandle;

/// Reconstruct every simple local instance independently and return the exact
/// instance symbols that ordinary generated-data validation may reference.
///
/// No fixed declaration or use count belongs here. The normalizer's retained
/// origin is sufficient to rejoin each instance to one same-unit template,
/// replay direct Type-parameter substitution, and require at least one
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
    for instance in &instances {
        if !validate_instance(source, data_frontier, instance) || symbols.contains(&instance.symbol)
        {
            return None;
        }
        symbols.push(instance.symbol);
    }
    if symbols.iter().all(|symbol| {
        source
            .data_definitions
            .iter()
            .skip(data_frontier)
            .filter(|definition| definition.generic_instance.is_none())
            .any(|definition| {
                source
                    .data_members(definition.members)
                    .iter()
                    .any(|member| {
                        let psi_symbol_resolved_trees::data::DataMember::Field(field) = member
                        else {
                            return false;
                        };
                        type_references_symbol(source, &field.type_reference, *symbol)
                    })
            })
    }) {
        Some(symbols)
    } else {
        None
    }
}

fn validate_instance(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    instance: &psi_symbol_resolved_trees::data::DataDefinition,
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
            || parameter.bounds != psi_symbol_resolved_trees::data::DataProperties::default()
            || !supported_named_argument(source, *argument_symbol, argument_name.as_str())
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
                substituted_field_matches(
                    source,
                    template.symbol,
                    instance.symbol,
                    &substitutions,
                    template_member,
                    instance_member,
                )
            },
        )
}

fn supported_named_argument(
    source: &SymbolResolvedTrees,
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
                    && definition.generic_instance.is_none()
            }),
        _ => false,
    }
}

fn substituted_field_matches(
    source: &SymbolResolvedTrees,
    template_owner: SymbolHandle,
    instance_owner: SymbolHandle,
    substitutions: &[(SymbolHandle, &TypeReference)],
    template: &psi_symbol_resolved_trees::data::DataMember,
    instance: &psi_symbol_resolved_trees::data::DataMember,
) -> bool {
    let (
        psi_symbol_resolved_trees::data::DataMember::Field(template),
        psi_symbol_resolved_trees::data::DataMember::Field(instance),
    ) = (template, instance)
    else {
        return false;
    };
    template.identity == instance.identity
        && template.name.as_str() == instance.name.as_str()
        && template.relevance == instance.relevance
        && template.symbol != instance.symbol
        && exact_field_symbol(source, template_owner, template)
        && exact_field_symbol(source, instance_owner, instance)
        && match &template.type_reference {
            TypeReference::Named { symbol, name }
                if substitutions
                    .iter()
                    .any(|(parameter, _)| parameter == symbol) =>
            {
                name.as_str() == source.symbols.name(*symbol)
                    && substitutions
                        .iter()
                        .find(|(parameter, _)| parameter == symbol)
                        .is_some_and(|(_, argument)| **argument == instance.type_reference)
            }
            TypeReference::Named { symbol, name } => {
                symbol.is_valid()
                    && name.as_str() == source.symbols.name(*symbol)
                    && template.type_reference == instance.type_reference
            }
            TypeReference::Unit => matches!(instance.type_reference, TypeReference::Unit),
            _ => false,
        }
}

fn type_references_symbol(
    source: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    symbol: SymbolHandle,
) -> bool {
    match type_reference {
        TypeReference::Named {
            symbol: candidate, ..
        } => *candidate == symbol,
        TypeReference::Reference(reference) => type_references_symbol(
            source,
            source.child_type_reference(reference.referee),
            symbol,
        ),
        TypeReference::Slice(slice) => type_references_symbol(
            source,
            source.child_type_reference(slice.element_type),
            symbol,
        ),
        TypeReference::FixedArray(array) => type_references_symbol(
            source,
            source.child_type_reference(array.element_type),
            symbol,
        ),
        TypeReference::Generic(generic) => {
            generic.base_symbol == symbol
                || source
                    .child_type_references(generic.arguments)
                    .iter()
                    .any(|argument| type_references_symbol(source, argument, symbol))
        }
        TypeReference::Constrained(constrained) => type_references_symbol(
            source,
            source.child_type_reference(constrained.base_type),
            symbol,
        ),
        TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => false,
    }
}
