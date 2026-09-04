//! Exact projected scalar-store admission for ordinary attached Unit bodies.

use super::*;

pub(super) fn build_structural_scalar_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statements: &[StatementNode],
) -> Option<CheckedStructuralScalarFieldStorePlan> {
    let [StatementNode::Assignment(assignment)] = statements else {
        return None;
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
        || scalar_parameters.len() > 1
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
        psi_language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        psi_language_semantics::ReferenceAccess::WriteOnly => {
            CheckedStructuralAccess::WriteOnlyBorrow
        }
        psi_language_semantics::ReferenceAccess::Shared => return None,
    };
    if destination.access != expected_access {
        return None;
    }
    let mut field_owner = crate::field_domain::data_definition_for_field_type(program, *referee)?;
    if !plain_record(field_owner, program) {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        0,
        assignment.target,
    )?;
    if place.root != psi_facts::PlaceRoot::Symbol(parameter.symbol) {
        return None;
    }
    let (final_segment, carrier_segments) = place.segments.split_last()?;
    let psi_facts::PlaceSegment::Field {
        symbol: field_symbol,
    } = final_segment
    else {
        return None;
    };
    let mut carrier_path = Vec::with_capacity(carrier_segments.len());
    for segment in carrier_segments {
        let psi_facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let carrier = exact_relevant_field(program, field_owner, *symbol)?;
        if !crate::field_domain::domain_constraint_symbols(program, carrier.type_reference)
            .is_empty()
        {
            return None;
        }
        carrier_path.push(CheckedUnitStructuralPathSegment::Field(
            terminal_field_identity(program, carrier.symbol)?,
        ));
        field_owner =
            crate::field_domain::data_definition_for_field_type(program, carrier.type_reference)?;
        if !plain_record(field_owner, program) {
            return None;
        }
    }
    let field = exact_relevant_field(program, field_owner, *field_symbol)?;
    if !crate::field_domain::domain_constraint_symbols(program, field.type_reference).is_empty() {
        return None;
    }
    let primitive_type = program.primitive_type_reference(field.type_reference)?;
    if !primitive_type.accepts_integer_literal() || primitive_type == PrimitiveType::Addr {
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
    let mutation_paths = facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame
        .complete_paths()?;
    if !matches!(mutation_paths, [path] if path == &expected_mutation_path) {
        return None;
    }
    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        0,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let exact_source = match scalar_parameters {
        [] => matches!(value, CheckedScalarExpression::IntegerLiteral { .. }),
        [scalar_parameter] => {
            scalar_parameter.source_position == 1
                && scalar_parameter.primitive_type == primitive_type
                && matches!(
                    value,
                    CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type: source_type,
                    } if *source_type == primitive_type
                )
        }
        _ => false,
    };
    if !exact_source || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }
    Some(CheckedStructuralScalarFieldStorePlan {
        statement_index: 0,
        destination_parameter_position: destination.position,
        carrier_path,
        field_identity: terminal_field_identity(program, field.symbol)?,
        primitive_type,
        value: value.clone(),
    })
}

fn plain_record(data: &psi_typed_trees::data::DataDefinition, program: &TypedTrees) -> bool {
    data.supply_mode == psi_language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && data.generic_instance.is_none()
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && psi_typed_trees::data::DataDefinition::shape_kind_from_members(
            program.data_members(data),
        ) == DataShapeKind::Record
}

fn exact_relevant_field<'a>(
    program: &'a TypedTrees,
    owner: &'a psi_typed_trees::data::DataDefinition,
    symbol: SymbolHandle,
) -> Option<&'a psi_typed_trees::data::DataField> {
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
