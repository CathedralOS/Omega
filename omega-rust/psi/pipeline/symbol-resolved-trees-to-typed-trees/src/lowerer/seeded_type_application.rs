//! Append-only validation for resolved generic types in generated data.

use super::{exact_top_level_data_symbol, plain_type_is_supported};
use symbol_resolved_trees::{SymbolResolvedTrees, types::GenericTypeReference};

/// Validate a resolved type application without synthesizing a declaration.
///
/// Seeded typing already owns the exact resolved base symbol and every child
/// argument. Keeping that structure as a typed `Generic` node is sufficient:
/// checked lowering and layout consume the same explicit application. This
/// validator therefore checks declaration identity, binder identity and the
/// complete recursive type-argument graph, but does not impose an incidental
/// number of uses, wrappers, templates, or builtin-only arguments.
pub(super) fn is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    owner: symbols::SymbolHandle,
    owner_lifetimes: &[symbol_resolved_trees::name::DiagnosticName],
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    application: &GenericTypeReference,
) -> bool {
    if !application.base_symbol.is_valid()
        || source.symbols.get(application.base_symbol).kind != symbols::SymbolKind::Data
        || application.base_name.as_str() != source.symbols.name(application.base_symbol)
    {
        return false;
    }
    let Some((definition_index, definition)) = source
        .data_definitions
        .iter()
        .enumerate()
        .find(|(_, definition)| definition.symbol == application.base_symbol)
    else {
        return false;
    };
    if definition.generic_instance.is_some()
        || !exact_top_level_data_symbol(source, definition)
        || definition.lifetime_parameters.len() != application.lifetime_arguments.len()
        || !application.lifetime_arguments.iter().all(|argument| {
            owner_lifetimes
                .iter()
                .any(|parameter| parameter.as_str() == argument.as_str())
        })
    {
        return false;
    }

    let parameters = source.data_type_parameters(definition.type_parameters);
    let arguments = source.child_type_references(application.arguments);
    if definition_index >= data_frontier {
        if parameters.is_empty() {
            return !definition.lifetime_parameters.is_empty() && arguments.is_empty();
        }
        return parameters.len() == arguments.len()
            && parameters.iter().all(|parameter| {
                super::seeded_local_instances::structured_const_parameter_is_supported(
                    source,
                    definition.symbol,
                    parameter,
                )
            })
            && parameters
                .iter()
                .zip(arguments)
                .all(|(parameter, argument)| {
                    super::seeded_local_instances::template_argument_is_supported(
                        source,
                        owner,
                        owner_type_parameters,
                        parameter,
                        argument,
                    )
                });
    }
    parameters.len() == arguments.len()
        && parameters
            .iter()
            .zip(arguments)
            .all(|(parameter, argument)| {
                parameter.symbol.is_valid()
                    && source.symbols.get(parameter.symbol).kind
                        == symbols::SymbolKind::TypeParameter
                    && source.symbols.get(parameter.symbol).parent == definition.symbol
                    && source.symbols.name(parameter.symbol) == parameter.name.as_str()
                    && match parameter.kind {
                        symbol_resolved_trees::data::TypeParameterKind::Type => {
                            plain_type_is_supported(
                                source,
                                data_frontier,
                                local_instances,
                                owner,
                                owner_lifetimes,
                                owner_type_parameters,
                                argument,
                            )
                        }
                        symbol_resolved_trees::data::TypeParameterKind::Const { .. } => {
                            super::seeded_local_instances::structured_const_parameter_is_supported(
                                source,
                                definition.symbol,
                                parameter,
                            ) && super::seeded_local_instances::template_argument_is_supported(
                                source,
                                owner,
                                owner_type_parameters,
                                parameter,
                                argument,
                            )
                        }
                        symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
                        | symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                            false
                        }
                    }
            })
}
