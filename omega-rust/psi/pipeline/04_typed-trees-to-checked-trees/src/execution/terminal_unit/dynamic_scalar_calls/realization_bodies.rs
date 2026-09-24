//! Realization scalar bodies, field store plans and service reach.

use crate::execution::terminal_unit::types::terminal_field_identity;
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitCallCoordinate, CheckedUnitStructuralPathSegment, PrimitiveType, StatementNode,
    SymbolHandle, TransitionExit, TransitionGuardNode, TransitionTargetNode, TypeReferenceNode,
    TypedTrees,
};

pub(crate) struct CheckedRealizationScalarBody {
    pub(crate) structural_scalar_field_stores:
        Vec<checked_trees::CheckedStructuralScalarFieldStorePlan>,
    pub(crate) return_expression: CheckedScalarExpression,
}

pub(crate) fn checked_realization_scalar_body(
    program: &TypedTrees,
    facts: &CheckFacts,
    realization_machine: &typed_trees::machine::Machine,
    realization_state: &typed_trees::state::State,
    result_type: PrimitiveType,
) -> Option<CheckedRealizationScalarBody> {
    let statements = program
        .statement_table
        .statements(realization_state.statement_nodes);
    let (statement, prefix) = statements.split_last()?;
    if prefix.len() > 3 {
        return None;
    }
    let mut stores_with_paths = Vec::with_capacity(prefix.len());
    for (statement_index, statement) in prefix.iter().enumerate() {
        let StatementNode::Assignment(assignment) = statement else {
            return None;
        };
        stores_with_paths.push(checked_realization_structural_scalar_field_store_plan(
            program,
            facts,
            realization_machine,
            realization_state,
            statement_index,
            assignment,
        )?);
    }
    if !stores_with_paths.is_empty() {
        let mut expected_mutation_paths = stores_with_paths
            .iter()
            .map(|(_, path)| path.clone())
            .collect::<Vec<_>>();
        expected_mutation_paths.sort();
        expected_mutation_paths.dedup();
        if expected_mutation_paths.len() != stores_with_paths.len() {
            return None;
        }
        let mutation_paths = facts
            .mutation
            .for_machine(realization_machine.symbol)?
            .state_write_frames
            .iter()
            .find(|frame| frame.state == realization_state.symbol)?
            .frame
            .complete_paths()?;
        if mutation_paths != expected_mutation_paths {
            return None;
        }
    }
    let return_statement_index = prefix.len();
    let structural_scalar_field_stores = stores_with_paths
        .into_iter()
        .map(|(store, _)| store)
        .collect();
    let (expression, value_role) = match statement {
        StatementNode::Expression(expression) => (
            *expression,
            checked_trees::CheckedValueStatementRole::Expression,
        ),
        StatementNode::Transition(transition)
            if transition.exit == TransitionExit::Ordinary
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid() =>
        {
            let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            (
                *expression,
                checked_trees::CheckedValueStatementRole::TransitionTargetValue,
            )
        }
        _ => return None,
    };
    let checked_values = facts
        .values
        .expression_values(expression)
        .filter(|(_, value)| {
            value.origin
                == checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: realization_machine.symbol,
                    state_symbol: realization_state.symbol,
                    statement_index: return_statement_index,
                    role: value_role,
                }
                && value.primitive_type == Some(result_type)
        })
        .collect::<Vec<_>>();
    let [_checked_value] = checked_values.as_slice() else {
        return None;
    };

    if let Some(checked) = facts.values.scalar_expressions.expression_at(
        realization_state.symbol,
        u32::try_from(return_statement_index).ok()?,
        CheckedScalarExpressionRole::Return,
    ) {
        return Some(CheckedRealizationScalarBody {
            structural_scalar_field_stores,
            return_expression: checked.clone(),
        });
    }

    let return_expression = checked_direct_self_field_return(
        program,
        realization_machine,
        realization_state,
        return_statement_index,
        expression,
        result_type,
    )?;
    Some(CheckedRealizationScalarBody {
        structural_scalar_field_stores,
        return_expression,
    })
}

fn checked_realization_structural_scalar_field_store_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    realization_machine: &typed_trees::machine::Machine,
    realization_state: &typed_trees::state::State,
    statement_index: usize,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<(checked_trees::CheckedStructuralScalarFieldStorePlan, String)> {
    let self_parameters = program
        .state_parameters(realization_state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self)
        .collect::<Vec<_>>();
    let [(self_position, self_parameter)] = self_parameters.as_slice() else {
        return None;
    };
    let TypeReferenceNode::Reference { access, .. } = program
        .type_reference_table
        .type_reference(self_parameter.type_reference)
    else {
        return None;
    };
    if self_parameter.is_const
        || !self_parameter.is_mutable
        || *access != language_semantics::ReferenceAccess::Mutable
        || !realization_machine.attached_data_symbol.is_valid()
    {
        return None;
    }

    let destination = crate::flow::canonical_place_from_expression_in_state(
        program,
        realization_state.symbol,
        statement_index,
        assignment.target,
    )?;
    if destination.root != facts::PlaceRoot::Symbol(self_parameter.symbol) {
        return None;
    }

    let attachments = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == realization_machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return None;
    };
    let (final_segment, carrier_segments) = destination.segments.split_last()?;
    let facts::PlaceSegment::Field {
        symbol: field_symbol,
    } = final_segment
    else {
        return None;
    };
    if !field_symbol.is_valid()
        || carrier_segments
            .iter()
            .any(|segment| !matches!(segment, facts::PlaceSegment::Field { symbol } if symbol.is_valid()))
    {
        return None;
    }
    let mut field_owner = *attachment;
    let mut carrier_path = Vec::with_capacity(carrier_segments.len());
    for segment in carrier_segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let carrier_fields = program
            .data_members(field_owner)
            .iter()
            .filter_map(|candidate| {
                let typed_trees::data::DataMember::Field(field) = candidate else {
                    return None;
                };
                (field.symbol == *symbol).then_some(field)
            })
            .collect::<Vec<_>>();
        let [carrier_field] = carrier_fields.as_slice() else {
            return None;
        };
        if carrier_field.relevance.is_erased() {
            return None;
        }
        carrier_path.push(CheckedUnitStructuralPathSegment::Field(
            terminal_field_identity(program, carrier_field.symbol)?,
        ));
        field_owner = crate::facts::field_domain::data_definition_for_field_type(
            program,
            carrier_field.type_reference,
        )?;
    }
    let fields = program
        .data_members(field_owner)
        .iter()
        .filter_map(|candidate| {
            let typed_trees::data::DataMember::Field(field) = candidate else {
                return None;
            };
            (field.symbol == *field_symbol).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(field.type_reference)?;
    if field.relevance.is_erased()
        || !(primitive_type == PrimitiveType::Bool
            || (primitive_type.accepts_integer_literal() && primitive_type != PrimitiveType::Addr))
    {
        return None;
    }

    let expected_mutation_path =
        facts::canonical_place_label_from_parts(program, destination.root, &destination.segments);
    let value = facts.values.scalar_expressions.expression_at(
        realization_state.symbol,
        u32::try_from(statement_index).ok()?,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(expression.as_ref(), CheckedBooleanExpression::Constant(_))
        );
    if !direct_literal || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }

    Some((
        checked_trees::CheckedStructuralScalarFieldStorePlan {
            statement_index: u32::try_from(statement_index).ok()?,
            destination: checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
                position: u32::try_from(*self_position).ok()?,
            },
            carrier_path,
            field_identity: terminal_field_identity(program, field.symbol)?,
            primitive_type,
            value: checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(value.clone()),
        },
        expected_mutation_path,
    ))
}

fn checked_direct_self_field_return(
    program: &TypedTrees,
    realization_machine: &typed_trees::machine::Machine,
    realization_state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    result_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let self_parameters = program
        .state_parameters(realization_state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self)
        .collect::<Vec<_>>();
    let [(self_position, self_parameter)] = self_parameters.as_slice() else {
        return None;
    };
    if !realization_machine.attached_data_symbol.is_valid() {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        realization_state.symbol,
        statement_index,
        expression,
    )?;
    let [
        facts::PlaceSegment::Field {
            symbol: field_symbol,
        },
    ] = place.segments.as_slice()
    else {
        return None;
    };
    if place.root != facts::PlaceRoot::Symbol(self_parameter.symbol) || !field_symbol.is_valid() {
        return None;
    }
    let attachment = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == realization_machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [attachment] = attachment.as_slice() else {
        return None;
    };
    let fields = program
        .data_members(attachment)
        .iter()
        .filter_map(|candidate| {
            let typed_trees::data::DataMember::Field(field) = candidate else {
                return None;
            };
            (field.symbol == *field_symbol).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    if field.relevance.is_erased()
        || program.primitive_type_reference(field.type_reference) != Some(result_type)
    {
        return None;
    }
    let parameter_position = u32::try_from(*self_position).ok()?;
    let path = vec![checked_trees::CheckedStructuralPredicatePathSegment::Field(
        terminal_field_identity(program, field.symbol)?,
    )];
    Some(if result_type == PrimitiveType::Bool {
        CheckedScalarExpression::Boolean(Box::new(
            checked_trees::CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            },
        ))
    } else {
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type: result_type,
        }
    })
}

pub(crate) fn checked_call_service_reach(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    flow_call: &checked_trees::FlowCallFact,
    coordinate: CheckedUnitCallCoordinate,
) -> Option<language_semantics::ServiceReachSummary> {
    let state = facts.service_reaches.for_state(caller_state)?;
    let calls = facts
        .service_reaches
        .calls_for(state)
        .iter()
        .filter(|call| {
            u32::try_from(call.statement_index).ok() == Some(coordinate.statement_index)
                && u32::try_from(call.call_ordinal).ok() == Some(coordinate.call_ordinal)
                && call.target_state == flow_call.target_symbol
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        return None;
    };
    let summary = language_semantics::ServiceReachSummary {
        direct: call.inferred_direct,
        transitive: call.inferred_transitive,
    };
    (summary == flow_call.service_reach).then_some(summary)
}
