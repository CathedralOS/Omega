//! Exact projected scalar-store admission for ordinary attached Unit bodies.

use super::*;

pub(super) fn build_structural_scalar_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statements: &[StatementNode],
    scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
    selected_scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
) -> Option<CheckedStructuralScalarFieldStorePlan> {
    let result_local = scalar_result_local.or(selected_scalar_result_local);
    let (statement_index, assignment) = match (result_local, statements) {
        (None, [StatementNode::Assignment(assignment)]) => (0, assignment),
        (
            Some(result),
            [
                StatementNode::LocalData(_),
                StatementNode::Assignment(assignment),
            ],
        ) if result.statement_index == 0 && result.binding_ordinal == 0 => (1, assignment),
        _ => return None,
    };
    let [destination] = structural_parameters else {
        return None;
    };
    if destination.is_self
        || destination.position != 0
        || destination.multiplicity == Multiplicity::Linear
        || !matches!(
            destination.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let parameter = source_parameters.first()?;
    if source_parameters.len() != scalar_parameters.len() + 1
        || parameter.is_self
        || parameter.is_const
        || !parameter.is_mutable
    {
        return None;
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let expected_access = match access {
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
        language_semantics::ReferenceAccess::Shared => return None,
    };
    if destination.access != expected_access {
        return None;
    }
    let mut carrier_type = *referee;
    let root_owner = crate::field_domain::data_definition_for_field_type(program, carrier_type)?;
    if !plain_record(root_owner, program) {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    if place.root != facts::PlaceRoot::Symbol(parameter.symbol) {
        return None;
    }
    let (final_segment, carrier_segments) = place.segments.split_last()?;
    let facts::PlaceSegment::Field {
        symbol: field_symbol,
    } = final_segment
    else {
        return None;
    };
    let mut carrier_path = Vec::with_capacity(carrier_segments.len());
    let mut reached_array = false;
    for segment in carrier_segments {
        match segment {
            facts::PlaceSegment::Field { symbol } if !reached_array => {
                let field_owner =
                    crate::field_domain::data_definition_for_field_type(program, carrier_type)?;
                if !plain_record(field_owner, program) {
                    return None;
                }
                let carrier = exact_relevant_field(program, field_owner, *symbol)?;
                if !crate::field_domain::domain_constraint_symbols(program, carrier.type_reference)
                    .is_empty()
                {
                    return None;
                }
                carrier_path.push(CheckedUnitStructuralPathSegment::Field(
                    terminal_field_identity(program, carrier.symbol)?,
                ));
                carrier_type = carrier.type_reference;
            }
            facts::PlaceSegment::FixedIndex { index } if !reached_array => {
                reached_array = true;
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier_type)
                else {
                    return None;
                };
                if *index >= *length {
                    return None;
                }
                carrier_path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                ));
                carrier_type = *element_type;
            }
            _ => return None,
        }
    }
    let field_owner = crate::field_domain::data_definition_for_field_type(program, carrier_type)?;
    if !plain_record(field_owner, program) {
        return None;
    }
    let field = exact_relevant_field(program, field_owner, *field_symbol)?;
    if !crate::field_domain::domain_constraint_symbols(program, field.type_reference).is_empty() {
        return None;
    }
    let primitive_type = program.primitive_type_reference(field.type_reference)?;
    if primitive_type != PrimitiveType::Bool
        && (!primitive_type.accepts_integer_literal() || primitive_type == PrimitiveType::Addr)
    {
        return None;
    }
    let source_path =
        crate::labels::canonical_place_label_from_parts(program, place.root, &place.segments);
    let source_root = crate::labels::canonical_place_label_from_parts(program, place.root, &[]);
    let expected_mutation_path = format!(
        "$P{}{}",
        destination.position,
        source_path.strip_prefix(&source_root)?,
    );
    let array_collection_mutation_path = place
        .segments
        .iter()
        .position(|segment| matches!(segment, facts::PlaceSegment::FixedIndex { .. }))
        .and_then(|first_index| {
            let collection_path = crate::labels::canonical_place_label_from_parts(
                program,
                place.root,
                &place.segments[..first_index],
            );
            Some(format!(
                "$P{}{}",
                destination.position,
                collection_path.strip_prefix(&source_root)?
            ))
        });
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame;
    let exact_frame =
        matches!(frame.complete_paths(), Some([path]) if path == &expected_mutation_path);
    let exact_collection_frame = matches!(
        (frame.complete_paths(), array_collection_mutation_path.as_ref()),
        (Some([path]), Some(collection_path)) if path == collection_path
    );
    // Provider selection resolves the boundary initializer after ordinary
    // mutation analysis, so that initializer leaves the pre-selection frame
    // opaque. The exact selected result, two-statement body, and canonical
    // projected destination are independently rejoined above and below.
    let unresolved_selected_frame = selected_scalar_result_local.is_some()
        && frame.completeness() == facts::WriteFrameCompleteness::Opaque;
    if !exact_frame && !exact_collection_frame && !unresolved_selected_frame {
        return None;
    }
    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let direct_result_is_exact = matches!(
        (result_local, value),
        (
            Some(result),
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type: source_type,
            },
        ) if *source_type == result.primitive_type
            && primitive_type == result.primitive_type
            && matches!(
                result.primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
            && scalar_parameters.is_empty()
    );
    let exact_source = if direct_result_is_exact {
        true
    } else if scalar_parameters.is_empty() {
        match value {
            CheckedScalarExpression::IntegerLiteral { .. } => {
                primitive_type.accepts_integer_literal() && primitive_type != PrimitiveType::Addr
            }
            CheckedScalarExpression::Boolean(boolean) => {
                primitive_type == PrimitiveType::Bool
                    && matches!(
                        boolean.as_ref(),
                        checked_trees::CheckedBooleanExpression::Constant(_)
                    )
            }
            _ => false,
        }
    } else {
        let (position, source_type) = checked_parameter_source(value)?;
        scalar_parameters.get(position).is_some_and(|parameter| {
            Some(parameter.source_position) == authored_scalar_position(position)
                && parameter.primitive_type == primitive_type
                && source_type == primitive_type
        }) && scalar_parameters
            .iter()
            .enumerate()
            .all(|(index, parameter)| {
                Some(parameter.source_position) == authored_scalar_position(index)
            })
    };
    if !exact_source || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }
    Some(CheckedStructuralScalarFieldStorePlan {
        statement_index,
        destination_parameter_position: destination.position,
        carrier_path,
        field_identity: terminal_field_identity(program, field.symbol)?,
        primitive_type,
        value: value.clone(),
    })
}

fn authored_scalar_position(dense_position: usize) -> Option<u32> {
    u32::try_from(dense_position).ok()?.checked_add(1)
}

fn checked_parameter_source(value: &CheckedScalarExpression) -> Option<(usize, PrimitiveType)> {
    match value {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Some((*position, *primitive_type)),
        CheckedScalarExpression::Boolean(boolean) => {
            let checked_trees::CheckedBooleanExpression::Parameter { position } = boolean.as_ref()
            else {
                return None;
            };
            Some((*position, PrimitiveType::Bool))
        }
        _ => None,
    }
}

fn plain_record(data: &typed_trees::data::DataDefinition, program: &TypedTrees) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && data.generic_instance.is_none()
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Record
}

fn exact_relevant_field<'a>(
    program: &'a TypedTrees,
    owner: &'a typed_trees::data::DataDefinition,
    symbol: SymbolHandle,
) -> Option<&'a typed_trees::data::DataField> {
    let fields = program
        .data_members(owner)
        .iter()
        .filter_map(|member| {
            let DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol && !field.relevance.is_erased()).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    Some(*field)
}
