//! Resolve selected structural scalar reads against their authored parameter roots.

use super::*;

pub(super) fn whole_byte_view_length(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedScalarExpression> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.member.as_str() != "len"
        || member.member_symbol.is_valid()
        || member.case_variant.is_some()
    {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression(program, member.receiver)?;
    if !place.segments.is_empty() {
        return None;
    }
    let facts::PlaceRoot::Symbol(symbol) =
        crate::flow::normalized_event_place_root(program, place.root)
    else {
        return None;
    };
    let parameter_position = parameters
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Shared,
        ..
    } = program
        .type_reference_table
        .type_reference(parameters[parameter_position].type_reference)
    else {
        return None;
    };
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    if program.primitive_type_reference(*element_type) != Some(PrimitiveType::U8) {
        return None;
    }
    Some(CheckedScalarExpression::StructuralParameterByteLength {
        parameter_position: u32::try_from(parameter_position).ok()?,
    })
}

/// Empty spelling resolution is builtin only when the exact operand types
/// independently carry the builtin index meaning.
pub(super) fn indexed_read_is_builtin(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    collection_type: TypeReferenceHandle,
    index: ExpressionHandle,
) -> bool {
    use language_core::OperatorSpelling;
    if let Some(selected) = operators.expression_use(expression)
        && (selected.spelling != OperatorSpelling::Index
            || selected.selected_operator_symbol.is_valid()
            || selected.candidate_count != 0
            || !matches!(
                selected.status,
                CheckedOperatorResolutionStatus::Missing
                    | CheckedOperatorResolutionStatus::BuiltinFallback
            ))
    {
        return false;
    }
    let Some((machine, state)) = program.machines().iter().find_map(|machine| {
        program.machine_states(machine).iter().find_map(|state| {
            let authored = program.state_parameters(state);
            (authored.len() == parameters.len()
                && authored
                    .iter()
                    .zip(parameters)
                    .all(|(left, right)| left.symbol == right.symbol))
            .then_some((machine, state))
        })
    }) else {
        return false;
    };
    let operands = [
        Some(collection_type),
        validation::declared_place_type_raw(program, machine, Some(state), index),
    ];
    typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        OperatorSpelling::Index,
        &operands,
    )
    .is_empty()
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            OperatorSpelling::Index,
            &operands,
        )
}

pub(super) fn structural_parameter_field_path(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    fields: &mut Vec<CheckedStructuralPredicatePathSegment>,
) -> Option<u32> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            let name_symbol = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                program
                    .expression_table
                    .name_path_member_symbols(name.member_symbols)
                    .iter()
                    .copied()
                    .find(|symbol| symbol.is_valid())
            });
            let name_text = program
                .expression_table
                .name_path_members(name.members)
                .last();
            parameters
                .iter()
                .position(|parameter| {
                    if let Some(symbol) = name_symbol {
                        parameter.symbol == symbol
                    } else {
                        name_text.is_some_and(|name| parameter.name == *name)
                    }
                })
                .and_then(|position| u32::try_from(position).ok())
        }
        ExpressionNode::Member(member) => {
            let parameter =
                structural_parameter_field_path(program, parameters, member.receiver, fields)?;
            let field_identity = |field: &typed_trees::data::DataField| {
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned())
            };
            if let Some(case_name) = &member.case_variant {
                let (case, field) = program.data_definitions().iter().find_map(|data| {
                    program.data_members(data).iter().find_map(|candidate| {
                        let typed_trees::data::DataMember::Variant(variant) = candidate else {
                            return None;
                        };
                        if variant.name != *case_name {
                            return None;
                        }
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.symbol == member.member_symbol)
                            .map(|field| {
                                (
                                    variant
                                        .identity
                                        .map(|identity| format!("#{identity}"))
                                        .unwrap_or_else(|| variant.name.as_str().to_owned()),
                                    field_identity(field),
                                )
                            })
                    })
                })?;
                fields.push(CheckedStructuralPredicatePathSegment::Case(case));
                fields.push(CheckedStructuralPredicatePathSegment::Field(field));
            } else {
                let identity = if member.member_symbol.is_valid() {
                    program.data_definitions().iter().find_map(|data| {
                        program.data_members(data).iter().find_map(|candidate| {
                            let typed_trees::data::DataMember::Field(field) = candidate else {
                                return None;
                            };
                            (field.symbol == member.member_symbol).then(|| field_identity(field))
                        })
                    })?
                } else {
                    // Contract member expressions can reach this carrier
                    // before their field symbol is retained. Keep the
                    // authored segment in that case: path_type_reference
                    // resolves it against the exact receiver type below,
                    // so this does not perform global name-based selection.
                    member.member.as_str().to_owned()
                };
                fields.push(CheckedStructuralPredicatePathSegment::Field(identity));
            }
            Some(parameter)
        }
        _ => None,
    }
}

pub(crate) fn resolve_structural_parameter_path(
    program: &TypedTrees,
    parameters: &[StateParameter],
    position: u32,
    path: &[CheckedStructuralPredicatePathSegment],
) -> Option<(
    symbols::SymbolHandle,
    Vec<facts::PlaceSegment>,
    TypeReferenceHandle,
)> {
    let parameter = parameters.get(usize::try_from(position).ok()?)?;
    if !parameter.symbol.is_valid() {
        return None;
    }
    let mut receiver = parameter.type_reference;
    let mut segments = Vec::new();
    let mut selected_case = None;
    for segment in path {
        match segment {
            CheckedStructuralPredicatePathSegment::Case(identity) => {
                if selected_case.is_some() {
                    return None;
                }
                let definition = structural_data(program, receiver)?;
                let variant = program.data_members(definition).iter().find_map(|member| {
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        return None;
                    };
                    let actual = variant
                        .identity
                        .map(|number| format!("#{number}"))
                        .unwrap_or_else(|| variant.name.as_str().to_owned());
                    (actual == *identity).then_some(variant)
                })?;
                if !variant.symbol.is_valid() {
                    return None;
                }
                segments.push(facts::PlaceSegment::Case {
                    variant: variant.symbol,
                });
                selected_case = Some(variant);
            }
            CheckedStructuralPredicatePathSegment::Field(identity) => {
                let matches = |field: &&typed_trees::data::DataField| {
                    field
                        .identity
                        .map(|number| format!("#{number}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned())
                        == *identity
                };
                let field = if let Some(variant) = selected_case.take() {
                    program.data_payload_fields(variant).iter().find(matches)?
                } else {
                    let definition = structural_data(program, receiver)?;
                    program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| {
                            let typed_trees::data::DataMember::Field(field) = member else {
                                return None;
                            };
                            Some(field)
                        })
                        .find(matches)?
                };
                if !field.symbol.is_valid() || field.relevance.is_erased() {
                    return None;
                }
                segments.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                receiver = field.type_reference;
            }
        }
    }
    selected_case
        .is_none()
        .then_some((parameter.symbol, segments, receiver))
}

fn structural_data(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    let (symbol, name) = loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, name }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                base_name: name,
                ..
            } => break (*symbol, name),
            _ => return None,
        }
    };
    let symbol = program
        .machines()
        .iter()
        .find(|machine| {
            symbol.is_valid() && machine.symbol == symbol && machine.attached_data_symbol.is_valid()
        })
        .map_or(symbol, |machine| machine.attached_data_symbol);
    program.data_definitions().iter().find(|definition| {
        if symbol.is_valid() {
            definition.symbol == symbol
        } else {
            definition.name == *name
        }
    })
}

pub(super) fn lower_structural_parameter_field(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let (parameter_position, path, type_reference) =
        structural_parameter_place(program, parameters, expression)?;
    if path.is_empty() {
        return None;
    }
    let primitive_type = program.primitive_type_reference(type_reference)?;
    if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
        return None;
    }
    Some((
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        },
        program.arithmetic_domain_for_type_reference(type_reference),
    ))
}

pub(super) fn structural_parameter_place(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(
    u32,
    Vec<CheckedStructuralPredicatePathSegment>,
    TypeReferenceHandle,
)> {
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    let root = crate::flow::normalized_event_place_root(program, place.root);
    let facts::PlaceRoot::Symbol(_) = root else {
        return None;
    };
    let parameter_position = parameters.iter().position(|parameter| {
        crate::flow::normalized_event_place_root(
            program,
            facts::PlaceRoot::Symbol(parameter.symbol),
        ) == root
    })?;
    let mut path = Vec::new();
    for segment in &place.segments {
        path.push(match segment {
            facts::PlaceSegment::Field { symbol } => {
                let field = program.data_definitions().iter().find_map(|definition| {
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if field.symbol == *symbol =>
                            {
                                Some(field)
                            }
                            typed_trees::data::DataMember::Variant(variant) => program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.symbol == *symbol),
                            _ => None,
                        })
                })?;
                CheckedStructuralPredicatePathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                )
            }
            facts::PlaceSegment::Case { variant } => {
                let case = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(candidate) = member else {
                            return None;
                        };
                        (candidate.symbol == *variant).then_some(candidate)
                    })
                })?;
                CheckedStructuralPredicatePathSegment::Case(
                    case.identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| case.name.as_str().to_owned()),
                )
            }
            _ => return None,
        });
    }
    let parameter_position = u32::try_from(parameter_position).ok()?;
    let (resolved_root, segments, type_reference) =
        resolve_structural_parameter_path(program, parameters, parameter_position, &path)?;
    if crate::flow::normalized_event_place_root(program, facts::PlaceRoot::Symbol(resolved_root))
        != root
        || segments != place.segments
    {
        return None;
    }
    Some((parameter_position, path, type_reference))
}
