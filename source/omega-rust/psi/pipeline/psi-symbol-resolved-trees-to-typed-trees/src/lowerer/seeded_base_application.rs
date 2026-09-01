//! Bounded retained-base generic applications for seeded generated data.

use super::{
    direct_closed_template_member_type_is_supported,
    direct_closed_wrapper_member_type_is_supported, exact_field_symbol,
    exact_top_level_data_symbol,
};
use psi_symbol_resolved_trees::{SymbolResolvedTrees, types::GenericTypeReference};

/// Admit one exact structural application of a retained-base generic record.
///
/// The generated-unit normalizer cannot clone a template from an earlier
/// source unit, so the application remains an explicit typed `Generic` node.
/// This replay binds both direct uses to one base-owned, methodless template
/// and one exact builtin argument; every broader application stays on the
/// whole-program rebuild path.
pub(super) fn is_supported(source: &SymbolResolvedTrees, data_frontier: usize) -> bool {
    let extension = source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .collect::<Vec<_>>();
    let [wrapper] = extension.as_slice() else {
        return false;
    };
    if !wrapper.lifetime_parameters.is_empty()
        || !wrapper.type_parameters.is_empty()
        || wrapper.generic_instance.is_some()
        || wrapper.quotient.is_some()
        || !wrapper.where_facts.is_empty()
        || wrapper.zero_gated
        || !exact_top_level_data_symbol(source, wrapper)
    {
        return false;
    }

    let wrapper_members = source.data_members(wrapper.members);
    let applications = wrapper_members
        .iter()
        .filter_map(|member| {
            let psi_symbol_resolved_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
                &field.type_reference
            else {
                return None;
            };
            Some((field, application))
        })
        .collect::<Vec<_>>();
    let [(first_field, first), (second_field, second)] = applications.as_slice() else {
        return false;
    };
    if !exact_field_symbol(source, wrapper.symbol, first_field)
        || !exact_field_symbol(source, wrapper.symbol, second_field)
        || !same_direct_builtin_application(source, first, second)
    {
        return false;
    }
    let [argument] = source.child_type_references(first.arguments) else {
        return false;
    };
    let psi_symbol_resolved_trees::types::TypeReference::Named {
        symbol: argument_symbol,
        name: argument_name,
    } = argument
    else {
        return false;
    };
    if !argument_symbol.is_valid()
        || source.symbols.get(*argument_symbol).kind != psi_symbols::SymbolKind::BuiltinType
        || argument_name.as_str() != source.symbols.name(*argument_symbol)
    {
        return false;
    }

    let Some(template) = source
        .data_definitions
        .iter()
        .take(data_frontier)
        .find(|definition| definition.symbol == first.base_symbol)
    else {
        return false;
    };
    let [parameter] = source.data_type_parameters(template.type_parameters) else {
        return false;
    };
    if first.base_name.as_str() != template.name.as_str()
        || !template.lifetime_parameters.is_empty()
        || template.generic_instance.is_some()
        || template.quotient.is_some()
        || !template.where_facts.is_empty()
        || template.zero_gated
        || !exact_top_level_data_symbol(source, template)
        || !parameter.symbol.is_valid()
        || source.symbols.get(parameter.symbol).kind != psi_symbols::SymbolKind::TypeParameter
        || source.symbols.get(parameter.symbol).parent != template.symbol
        || source.symbols.name(parameter.symbol) != parameter.name.as_str()
        || !matches!(
            parameter.kind,
            psi_symbol_resolved_trees::data::TypeParameterKind::Type
        )
        || parameter.bounds != psi_symbol_resolved_trees::data::DataProperties::default()
        || source
            .machines
            .iter()
            .any(|machine| machine.attached_data_symbol == template.symbol)
    {
        return false;
    }
    let template_members = source.data_members(template.members);
    if template_members.is_empty()
        || !template_members.iter().any(|member| {
            matches!(
                member,
                psi_symbol_resolved_trees::data::DataMember::Field(field)
                    if matches!(
                        &field.type_reference,
                        psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name }
                            if *symbol == parameter.symbol
                                && name.as_str() == source.symbols.name(*symbol)
                    )
            )
        })
        || !template_members.iter().all(|member| {
            let psi_symbol_resolved_trees::data::DataMember::Field(field) = member else {
                return false;
            };
            exact_field_symbol(source, template.symbol, field)
                && (matches!(
                    &field.type_reference,
                    psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name }
                        if *symbol == parameter.symbol
                            && name.as_str() == source.symbols.name(*symbol)
                ) || direct_closed_template_member_type_is_supported(
                    source,
                    &field.type_reference,
                ))
        })
    {
        return false;
    }

    wrapper_members.iter().all(|member| {
        let psi_symbol_resolved_trees::data::DataMember::Field(field) = member else {
            return false;
        };
        exact_field_symbol(source, wrapper.symbol, field)
            && (matches!(
                &field.type_reference,
                psi_symbol_resolved_trees::types::TypeReference::Generic(application)
                    if same_direct_builtin_application(source, first, application)
            ) || direct_closed_wrapper_member_type_is_supported(
                source,
                data_frontier,
                &field.type_reference,
            ))
    })
}

fn same_direct_builtin_application(
    source: &SymbolResolvedTrees,
    left: &GenericTypeReference,
    right: &GenericTypeReference,
) -> bool {
    let ([left_argument], [right_argument]) = (
        source.child_type_references(left.arguments),
        source.child_type_references(right.arguments),
    ) else {
        return false;
    };
    left.lifetime_arguments.is_empty()
        && right.lifetime_arguments.is_empty()
        && left.base_symbol.is_valid()
        && left.base_symbol == right.base_symbol
        && left.base_name.as_str() == source.symbols.name(left.base_symbol)
        && right.base_name.as_str() == source.symbols.name(right.base_symbol)
        && matches!(
            (left_argument, right_argument),
            (
                psi_symbol_resolved_trees::types::TypeReference::Named {
                    symbol: left_symbol,
                    name: left_name,
                },
                psi_symbol_resolved_trees::types::TypeReference::Named {
                    symbol: right_symbol,
                    name: right_name,
                },
            ) if left_symbol.is_valid()
                && left_symbol == right_symbol
                && source.symbols.get(*left_symbol).kind
                    == psi_symbols::SymbolKind::BuiltinType
                && left_name.as_str() == source.symbols.name(*left_symbol)
                && right_name.as_str() == source.symbols.name(*right_symbol)
        )
}
