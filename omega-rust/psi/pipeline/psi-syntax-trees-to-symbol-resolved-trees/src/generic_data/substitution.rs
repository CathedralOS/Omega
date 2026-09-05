//! Member and type-reference substitution.

use super::*;

/// Clone a member with the type parameters substituted. Only reached for a base
/// `base_is_fully_monomorphizable` accepted. A field that IS a parameter points
/// at the argument; a NESTED generic (`a: Box<T>`) becomes a fresh concrete
/// spelling (`Box<i32>`) the fixpoint monomorphizes; a parameter-free field is
/// shared unchanged.
pub(in crate::generic_data) fn substitute_member(
    syntax: &mut SyntaxTrees,
    member: DataMember,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
) -> DataMember {
    match member {
        DataMember::Field(field) => DataMember::Field(substitute_data_field(
            syntax,
            field,
            substitution,
            const_values,
        )),
        DataMember::Variant(mut variant) => {
            let payload = syntax
                .tables
                .items
                .data_payload_fields(variant.payload)
                .to_vec();
            let mut first = Handle::invalid();
            let mut count = 0u32;
            for field in payload {
                let field = substitute_data_field(syntax, field, substitution, const_values);
                let handle = syntax.tables.items.append_data_payload_field(field);
                if count == 0 {
                    first = handle;
                }
                count = count
                    .checked_add(1)
                    .expect("generic sum payload field count overflow");
            }
            variant.payload = HandleSpan::from_parts(first, count);
            DataMember::Variant(variant)
        }
        DataMember::Retired(identity) => DataMember::Retired(identity),
    }
}

pub(in crate::generic_data) fn substitute_data_field(
    syntax: &mut SyntaxTrees,
    mut field: psi_syntax_trees::item::DataField,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
) -> psi_syntax_trees::item::DataField {
    field.type_reference =
        substitute_type_reference(syntax, field.type_reference, substitution, const_values);
    field
}

pub(in crate::generic_data) fn substitute_type_reference(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
) -> TypeReferenceHandle {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Named(name) => substitution
            .get(name.as_str())
            .copied()
            .unwrap_or(type_reference),
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let argument_handles: Vec<TypeReferenceHandle> = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, base_name.as_str());
            let const_bindings: HashMap<String, i128> = substitution
                .iter()
                .filter_map(|(name, argument)| {
                    let TypeReferenceNode::Named(value) =
                        syntax.tables.type_references.type_reference(*argument)
                    else {
                        return None;
                    };
                    Some((name.clone(), value.as_str().parse::<i128>().ok()?))
                })
                .collect();
            let mut substituted_arguments = Vec::with_capacity(argument_handles.len());
            for (index, argument) in argument_handles.into_iter().enumerate() {
                let node = syntax
                    .tables
                    .type_references
                    .type_reference(argument)
                    .clone();
                let substituted = match node {
                    TypeReferenceNode::Named(name) => {
                        substitution.get(name.as_str()).copied().unwrap_or(argument)
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        match evaluate_const_argument_expression(
                            syntax,
                            expression,
                            const_values,
                            &const_bindings,
                            &HashSet::new(),
                            integer_types.get(index).copied().flatten(),
                        )
                        .and_then(EvaluatedConst::into_concrete)
                        {
                            Ok(value) => syntax
                                .tables
                                .type_references
                                .insert_named(Identifier::generated(value.to_string())),
                            Err(_) => argument,
                        }
                    }
                    _ => substitute_type_reference(syntax, argument, substitution, const_values),
                };
                substituted_arguments.push(substituted);
            }
            let new_span = syntax
                .tables
                .type_references
                .insert_type_reference_handles(substituted_arguments);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments: new_span,
                })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let substituted_element =
                substitute_type_reference(syntax, element_type, substitution, const_values);
            let substituted_length = match length {
                FixedArrayLength::ConstParameter(name) => substitution
                    .get(name.as_str())
                    .and_then(|argument| {
                        match syntax.tables.type_references.type_reference(*argument) {
                            TypeReferenceNode::Named(value) => value.as_str().parse::<usize>().ok(),
                            _ => None,
                        }
                    })
                    .map(FixedArrayLength::Literal)
                    .unwrap_or(FixedArrayLength::ConstParameter(name)),
                length => length,
            };
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::FixedArray {
                    element_type: substituted_element,
                    length: substituted_length,
                })
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let referee = substitute_type_reference(syntax, referee, substitution, const_values);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                })
        }
        TypeReferenceNode::Slice { element_type } => {
            let element_type =
                substitute_type_reference(syntax, element_type, substitution, const_values);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Slice { element_type })
        }
        _ => type_reference,
    }
}

/// Whether a type reference mentions any of the substituted parameter names
/// (recursively through composite nodes). Conservative: on an unhandled node
/// shape it returns `true` so the caller rejects rather than silently sharing a
/// parameter-bearing type.
pub(in crate::generic_data) fn type_reference_mentions_parameter(
    syntax: &SyntaxTrees,
    handle: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => substitution.contains_key(name.as_str()),
        TypeReferenceNode::Generic { arguments, .. } => syntax
            .tables
            .type_references
            .type_reference_handles(*arguments)
            .iter()
            .any(|&argument| type_reference_mentions_parameter(syntax, argument, substitution)),
        // The common composite shells recurse precisely, so a parameter-FREE
        // field like `touched: i32 in Wrapping` (Constrained) or
        // `tags: [u8; 4]` shares unchanged instead of refusing the whole
        // container (constraints carry domain names, not type references).
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_mentions_parameter(syntax, *base_type, substitution)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_mentions_parameter(syntax, *element_type, substitution)
        }
        TypeReferenceNode::Slice { element_type } => {
            type_reference_mentions_parameter(syntax, *element_type, substitution)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_mentions_parameter(syntax, *referee, substitution)
        }
        // Anything else: conservative -- possibly parameter-bearing, refuse
        // rather than share a wrong type.
        _ => true,
    }
}
