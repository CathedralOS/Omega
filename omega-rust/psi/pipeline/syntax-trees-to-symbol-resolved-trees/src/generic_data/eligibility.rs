//! Eligible data shapes, recursion and substitutable types.

use super::*;

/// Whether every field of a record or sum can be substituted soundly. A
/// field may be exactly the parameter, a concrete Named, a parameter-free
/// composite, or a nested known generic whose arguments are substitutable.
pub(in crate::generic_data) fn base_is_fully_monomorphizable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&constant_selection::ConstantSelection>,
    base_info: &GenericData,
) -> bool {
    // Recursive inline data is proof-only. Keep its generic identity intact so
    // the structural entailment tier continues to see the authored generic
    // constructors and recursive applications; closed-instance synthesis is
    // an executable-layout transform, not a proof-data transform.
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
    syntax
        .tables
        .items
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
                syntax
                    .tables
                    .items
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

pub(in crate::generic_data) fn generic_data_is_recursive(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&constant_selection::ConstantSelection>,
    base: &GenericData,
) -> bool {
    fn reaches(
        syntax: &SyntaxTrees,
        templates: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
        selection: Option<&constant_selection::ConstantSelection>,
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
                // A template binder shadows a same-spelled nominal declaration.
                if current
                    .parameter_names
                    .iter()
                    .any(|parameter| parameter == name.as_str())
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

pub(in crate::generic_data) fn generic_inline_data_edges(
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
                for argument in syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments)
                {
                    collect(syntax, *argument, edges);
                }
            }
            TypeReferenceNode::Constrained { base_type, .. } => collect(syntax, *base_type, edges),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                collect(syntax, *element_type, edges)
            }
            // References and slices are indirection and therefore break the
            // inline-containment cycle, matching proof-only classification.
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::SelfType
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

pub(in crate::generic_data) fn generic_data_shape(
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

pub(in crate::generic_data) fn type_reference_is_substitutable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&constant_selection::ConstantSelection>,
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

pub(in crate::generic_data) fn type_reference_handle_is_substitutable(
    syntax: &SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&constant_selection::ConstantSelection>,
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
            selected_generic_data(syntax, generic_data, selection, base_name).is_some()
                && syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments)
                    .iter()
                    .all(|&argument| {
                        matches!(
                            syntax.tables.type_references.type_reference(argument),
                            TypeReferenceNode::Named(_) | TypeReferenceNode::ConstExpression(_)
                        ) || !type_reference_mentions_parameter(syntax, argument, parameters)
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
                FixedArrayLength::Literal(_) | FixedArrayLength::ConstCall(_) => true,
                FixedArrayLength::ConstParameter(name) => base_info
                    .parameter_names
                    .iter()
                    .zip(&base_info.const_parameter_types)
                    .any(|(parameter_name, parameter_type)| {
                        parameter_type.is_some() && parameter_name == name.as_str()
                    }),
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
        _ => !type_reference_mentions_parameter(syntax, type_reference, parameters),
    }
}
