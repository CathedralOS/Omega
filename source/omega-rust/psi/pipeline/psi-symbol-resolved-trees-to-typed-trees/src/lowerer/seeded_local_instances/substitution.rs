use super::super::exact_field_symbol;
use psi_symbol_resolved_trees::{SymbolResolvedTrees, types::TypeReference};
use psi_symbols::SymbolHandle;

pub(super) fn member_matches(
    source: &SymbolResolvedTrees,
    template_owner: SymbolHandle,
    instance_owner: SymbolHandle,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &psi_symbol_resolved_trees::data::DataMember,
    instance: &psi_symbol_resolved_trees::data::DataMember,
) -> bool {
    use psi_symbol_resolved_trees::data::DataMember;
    match (template, instance) {
        (DataMember::Field(template), DataMember::Field(instance)) => field_matches(
            source,
            template_owner,
            instance_owner,
            substitutions,
            validated_instances,
            template,
            instance,
        ),
        (DataMember::Variant(template), DataMember::Variant(instance)) => variant_matches(
            source,
            template_owner,
            instance_owner,
            substitutions,
            validated_instances,
            template,
            instance,
        ),
        _ => false,
    }
}

fn field_matches(
    source: &SymbolResolvedTrees,
    template_owner: SymbolHandle,
    instance_owner: SymbolHandle,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &psi_symbol_resolved_trees::data::DataField,
    instance: &psi_symbol_resolved_trees::data::DataField,
) -> bool {
    template.identity == instance.identity
        && template.name.as_str() == instance.name.as_str()
        && template.relevance == instance.relevance
        && template.symbol != instance.symbol
        && exact_field_symbol(source, template_owner, template)
        && exact_field_symbol(source, instance_owner, instance)
        && type_matches(
            source,
            substitutions,
            validated_instances,
            &template.type_reference,
            &instance.type_reference,
        )
}

fn variant_matches(
    source: &SymbolResolvedTrees,
    template_owner: SymbolHandle,
    instance_owner: SymbolHandle,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &psi_symbol_resolved_trees::data::DataVariant,
    instance: &psi_symbol_resolved_trees::data::DataVariant,
) -> bool {
    let template_payload = source.data_payload_fields(template.payload);
    let instance_payload = source.data_payload_fields(instance.payload);
    template.identity == instance.identity
        && template.name.as_str() == instance.name.as_str()
        && template.retired_payload_identities == instance.retired_payload_identities
        && template.symbol != instance.symbol
        && exact_variant_symbol(source, template_owner, template)
        && exact_variant_symbol(source, instance_owner, instance)
        && template_payload.len() == instance_payload.len()
        && template_payload
            .iter()
            .zip(instance_payload)
            .all(|(template_field, instance_field)| {
                field_matches(
                    source,
                    template.symbol,
                    instance.symbol,
                    substitutions,
                    validated_instances,
                    template_field,
                    instance_field,
                )
            })
}

fn exact_variant_symbol(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    variant: &psi_symbol_resolved_trees::data::DataVariant,
) -> bool {
    variant.symbol.is_valid()
        && source.symbols.get(variant.symbol).kind == psi_symbols::SymbolKind::Variant
        && source.symbols.get(variant.symbol).parent == owner
        && source.symbols.name(variant.symbol) == variant.name.as_str()
}

fn type_matches(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &TypeReference,
    instance: &TypeReference,
) -> bool {
    match template {
        TypeReference::Named { symbol, name }
            if substitutions
                .iter()
                .any(|(parameter, _)| parameter == symbol) =>
        {
            name.as_str() == source.symbols.name(*symbol)
                && substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
                    .is_some_and(|(_, argument)| **argument == *instance)
        }
        TypeReference::Named { symbol, name } => {
            symbol.is_valid()
                && name.as_str() == source.symbols.name(*symbol)
                && template == instance
        }
        TypeReference::Unit => matches!(instance, TypeReference::Unit),
        TypeReference::FixedArray(template_array) => {
            let TypeReference::FixedArray(instance_array) = instance else {
                return false;
            };
            template_array.length == instance_array.length
                && type_matches(
                    source,
                    substitutions,
                    validated_instances,
                    source.child_type_reference(template_array.element_type),
                    source.child_type_reference(instance_array.element_type),
                )
        }
        TypeReference::Reference(template_reference) => {
            let TypeReference::Reference(instance_reference) = instance else {
                return false;
            };
            template_reference.access == instance_reference.access
                && template_reference.lifetime == instance_reference.lifetime
                && type_matches(
                    source,
                    substitutions,
                    validated_instances,
                    source.child_type_reference(template_reference.referee),
                    source.child_type_reference(instance_reference.referee),
                )
        }
        TypeReference::Slice(template_slice) => {
            let TypeReference::Slice(instance_slice) = instance else {
                return false;
            };
            type_matches(
                source,
                substitutions,
                validated_instances,
                source.child_type_reference(template_slice.element_type),
                source.child_type_reference(instance_slice.element_type),
            )
        }
        TypeReference::Generic(template_generic) => {
            let TypeReference::Named { symbol, name } = instance else {
                return false;
            };
            name.as_str() == source.symbols.name(*symbol)
                && validated_instances.contains(symbol)
                && source
                    .data_definitions
                    .iter()
                    .find(|definition| definition.symbol == *symbol)
                    .and_then(|definition| definition.generic_instance.as_ref())
                    .is_some_and(|origin| {
                        let TypeReference::Generic(origin) = origin else {
                            return false;
                        };
                        origin.base_symbol == template_generic.base_symbol
                            && origin.base_name.as_str() == template_generic.base_name.as_str()
                            && origin.lifetime_arguments == template_generic.lifetime_arguments
                            && {
                                let template_arguments =
                                    source.child_type_references(template_generic.arguments);
                                let instance_arguments =
                                    source.child_type_references(origin.arguments);
                                template_arguments.len() == instance_arguments.len()
                                    && template_arguments.iter().zip(instance_arguments).all(
                                        |(template_argument, instance_argument)| {
                                            type_matches(
                                                source,
                                                substitutions,
                                                validated_instances,
                                                template_argument,
                                                instance_argument,
                                            )
                                        },
                                    )
                            }
                    })
        }
        TypeReference::Constrained(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::SelfType { .. } => false,
    }
}
