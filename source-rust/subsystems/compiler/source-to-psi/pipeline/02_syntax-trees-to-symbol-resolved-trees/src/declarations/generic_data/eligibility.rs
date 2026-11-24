/*
    Eligible data shapes, recursion and substitutable types.
    Named array extents either belong to the template's binder roster or select
    a closed declaration in their authored scope. Reuse the closed extent reader
    for declaration readiness without rewriting the identifier: substitution and
    receiving typing still need its selected origin. Binder-headed paths cannot
    fall through to a same-spelled global declaration during specialization.
    Generic arguments follow the same recursive substitution rules as fields.
    Closed children keep their existing sharing route; parameter-bearing arrays
    and applications must be substitutable throughout, rather than only at a
    named leaf. Receiving typing still checks each constructor's argument kinds.
*/
use crate::resolution::declarations::declaration_selection;
use crate::resolution::declarations::generic_data::GenericData;
use crate::resolution::declarations::generic_data::GenericDataShape;
use crate::resolution::declarations::generic_data::selected_generic_data;
use crate::resolution::declarations::generic_data::type_reference_mentions_parameter;
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;
use std::collections::HashMap;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::DataMember;
use syntax_trees::types::FixedArrayLength;
use syntax_trees::types::TypeConstraintNode;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;

/*
    Whether every field of a record or sum can be substituted soundly. A
    field may be exactly the parameter, a concrete Named, a parameter-free
    composite, or a nested known generic whose arguments are substitutable.
*/
pub(in crate::resolution::declarations::generic_data) fn base_is_fully_monomorphizable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&declaration_selection::ConstantSelection>,
    base_info: &GenericData,
) -> bool {
    if generic_data_is_recursive(syntax, generic_data, selection, base_info) {
        return false;
    }
    let parameters: HashMap<String, TypeReferenceHandle> = base_info
        .parameter_names
        .iter()
        .map(|name| (name.clone(), TypeReferenceHandle::default()))
        .collect();
    let Some(shape) = generic_data_shape(syntax, base_info) else {
        return false;
    };
    syntax_items(syntax)
        .data_members(base_info.members)
        .iter()
        .all(|member| match member {
            DataMember::Field(field)
                if matches!(shape, GenericDataShape::Record | GenericDataShape::MixedSum) =>
            {
                type_reference_is_substitutable(
                    syntax,
                    generic_data,
                    selection,
                    base_info,
                    field,
                    &parameters,
                )
            }
            DataMember::Variant(variant)
                if matches!(
                    shape,
                    GenericDataShape::PureSum | GenericDataShape::MixedSum
                ) =>
            {
                syntax_items(syntax)
                    .data_payload_fields(variant.payload)
                    .iter()
                    .all(|field| {
                        type_reference_is_substitutable(
                            syntax,
                            generic_data,
                            selection,
                            base_info,
                            field,
                            &parameters,
                        )
                    })
            }
            DataMember::Retired(_) => true,
            _ => false,
        })
}

pub(in crate::resolution::declarations::generic_data) fn generic_data_is_recursive(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&declaration_selection::ConstantSelection>,
    base: &GenericData,
) -> bool {
    fn reaches(
        syntax: &SyntaxTrees,
        templates: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
        selection: Option<&declaration_selection::ConstantSelection>,
        current: &GenericData,
        goal: syntax_trees::item::ItemHandle,
        visited: &mut HashSet<syntax_trees::item::ItemHandle>,
    ) -> bool {
        if !visited.insert(current.declaration) {
            return false;
        }
        generic_inline_data_edges(syntax, current)
            .iter()
            .any(|name| {
                if current
                    .parameter_names
                    .iter()
                    .any(|parameter| parameter == name.as_str(syntax))
                {
                    return false;
                }
                let Some(next) = selected_generic_data(syntax, templates, selection, name) else {
                    return false;
                };
                next.declaration == goal
                    || reaches(syntax, templates, selection, next, goal, visited)
            })
    }
    reaches(
        syntax,
        generic_data,
        selection,
        base,
        base.declaration,
        &mut HashSet::new(),
    )
}

pub(in crate::resolution::declarations::generic_data) fn generic_inline_data_edges(
    syntax: &SyntaxTrees,
    definition: &GenericData,
) -> Vec<Identifier> {
    fn collect(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        edges: &mut Vec<Identifier>,
    ) {
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(name) => {
                edges.push(name.clone());
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                edges.push(base_name.clone());
                for argument in syntax_type_references(syntax).type_reference_handles(*arguments) {
                    collect(syntax, *argument, edges);
                }
            }
            TypeReferenceNode::Constrained { base_type, .. } => collect(syntax, *base_type, edges),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                collect(syntax, *element_type, edges)
            }
            TypeReferenceNode::StaticSelection(selection) => {
                super::uses::visit_static_argument_type_inputs(
                    syntax,
                    std::slice::from_ref(selection),
                    &mut |reference| collect(syntax, reference, edges),
                );
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Callable { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => {}
        }
    }

    let mut edges = Vec::new();
    for member in syntax.tables.items.data_members(definition.members) {
        match member {
            DataMember::Field(field) => collect(syntax, field.type_reference, &mut edges),
            DataMember::Variant(variant) => {
                for field in syntax.tables.items.data_payload_fields(variant.payload) {
                    collect(syntax, field.type_reference, &mut edges);
                }
            }
            DataMember::Retired(_) => {}
        }
    }
    edges
}

pub(in crate::resolution::declarations::generic_data) fn generic_data_shape(
    syntax: &SyntaxTrees,
    base_info: &GenericData,
) -> Option<GenericDataShape> {
    let mut has_fields = false;
    let mut has_variants = false;
    for member in syntax.tables.items.data_members(base_info.members) {
        match member {
            DataMember::Field(_) => has_fields = true,
            DataMember::Variant(_) => has_variants = true,
            DataMember::Retired(_) => {}
        }
    }
    match (has_fields, has_variants) {
        (true, false) | (false, false) => Some(GenericDataShape::Record),
        (false, true) => Some(GenericDataShape::PureSum),
        (true, true) => Some(GenericDataShape::MixedSum),
    }
}

pub(in crate::resolution::declarations::generic_data) fn type_reference_is_substitutable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&declaration_selection::ConstantSelection>,
    base_info: &GenericData,
    field: &syntax_trees::item::DataField,
    parameters: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    type_reference_handle_is_substitutable(
        syntax,
        generic_data,
        selection,
        base_info,
        field.type_reference,
        parameters,
    )
}

pub(in crate::resolution::declarations::generic_data) fn type_reference_handle_is_substitutable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&declaration_selection::ConstantSelection>,
    base_info: &GenericData,
    type_reference: TypeReferenceHandle,
    parameters: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(_) => true,
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let head = base_name
                .as_str(syntax)
                .split_once("::")
                .map_or(base_name.as_str(syntax), |(head, _)| head);
            !parameters.contains_key(head)
                && selected_generic_data(syntax, generic_data, selection, base_name).is_some()
                && syntax_type_references(syntax)
                    .type_reference_handles(*arguments)
                    .iter()
                    .all(|&argument| {
                        matches!(
                            syntax.tables.type_references.type_reference(argument),
                            TypeReferenceNode::ConstExpression(_)
                        ) || !type_reference_mentions_parameter(syntax, argument, parameters)
                            || type_reference_handle_is_substitutable(
                                syntax,
                                generic_data,
                                selection,
                                base_info,
                                argument,
                                parameters,
                            )
                    })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let element_is_substitutable = type_reference_handle_is_substitutable(
                syntax,
                generic_data,
                selection,
                base_info,
                *element_type,
                parameters,
            );
            let length_is_substitutable = match length {
                FixedArrayLength::Literal(_)
                | FixedArrayLength::ConstCall(_)
                | FixedArrayLength::Expression(_) => true,
                FixedArrayLength::ConstParameter(name) => {
                    let head = name
                        .as_str(syntax)
                        .split_once("::")
                        .map_or(name.as_str(syntax), |(head, _)| head);
                    if let Some((parameter_name, parameter_type)) = base_info
                        .parameter_names
                        .iter()
                        .zip(&base_info.const_parameter_types)
                        .find(|(parameter_name, _)| parameter_name.as_str() == head)
                    {
                        parameter_name == name.as_str(syntax) && parameter_type.is_some()
                    } else {
                        super::closed_fixed_array_length(
                            syntax,
                            length,
                            &declaration_selection::GenericApplicationSubstitution::new(),
                            selection,
                        )
                        .is_ok()
                    }
                }
            };
            element_is_substitutable && length_is_substitutable
        }
        TypeReferenceNode::Reference { referee, .. } => type_reference_handle_is_substitutable(
            syntax,
            generic_data,
            selection,
            base_info,
            *referee,
            parameters,
        ),
        TypeReferenceNode::Slice { element_type } => type_reference_handle_is_substitutable(
            syntax,
            generic_data,
            selection,
            base_info,
            *element_type,
            parameters,
        ),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            type_reference_handle_is_substitutable(
                syntax,
                generic_data,
                selection,
                base_info,
                *base_type,
                parameters,
            ) && syntax_type_references(syntax)
                .constraints(*constraints)
                .iter()
                .all(|constraint| match constraint {
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => true,
                    TypeConstraintNode::Domain(domain) => syntax_type_references(syntax)
                        .type_reference_handles(domain.arguments)
                        .iter()
                        .all(|&argument| {
                            matches!(
                                syntax.tables.type_references.type_reference(argument),
                                TypeReferenceNode::Named(_) | TypeReferenceNode::ConstExpression(_)
                            ) || !type_reference_mentions_parameter(syntax, argument, parameters)
                        }),
                })
        }
        _ => !type_reference_mentions_parameter(syntax, type_reference, parameters),
    }
}
