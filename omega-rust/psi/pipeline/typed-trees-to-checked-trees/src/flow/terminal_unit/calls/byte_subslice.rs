//! Immutable byte ranges retain their source and evaluated endpoint expressions.

use super::*;
use typed_trees::expression::ExpressionHandle;

pub(super) fn argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
    call_ordinal: usize,
    argument_ordinal: usize,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if !exact_borrowed_byte_view(program, target) {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    if range.end_inclusive {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        indexed.collection,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    if !place.segments.is_empty() {
        return None;
    }
    let authored = program.state_parameters(state);
    let position = authored
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    if authored[position].is_mutable {
        return None;
    }
    let source_type = authored[position].type_reference;
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)?;
    let parameter = &parameters[parameter_index];
    let type_identity = byte_sequence_type_identity(program, target, &[], &[])?;
    if parameter.access != CheckedStructuralAccess::SharedBorrow
        || parameter.multiplicity != Multiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || parameter.type_identity != type_identity
        || !exact_borrowed_byte_view(program, source_type)
    {
        return None;
    }
    let spelling = language_core::OperatorSpelling::Range;
    if facts
        .operators
        .expression_use(expression)
        .is_some_and(|selected| {
            selected.spelling != spelling
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                )
        })
    {
        return None;
    }
    let operands = [
        Some(source_type),
        validation::declared_place_type_raw(program, machine, Some(state), range.start),
        validation::declared_place_type_raw(program, machine, Some(state), range.end),
    ];
    if !typed_trees::operator::resolve_indexed_spelling_for_operands(program, spelling, &operands)
        .is_empty()
        || !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &operands,
        )
    {
        return None;
    }
    let call_ordinal = u32::try_from(call_ordinal).ok()?;
    let argument_ordinal = u32::try_from(argument_ordinal).ok()?;
    let statement_ordinal = u32::try_from(statement_index).ok()?;
    let endpoint = |expression, role| {
        let (binding, endpoint) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_ordinal,
            role,
        )?;
        (binding.expression == expression
            && !binding.destination.is_valid()
            && endpoint.primitive_type() == Some(PrimitiveType::U64))
        .then(|| endpoint.clone())
    };
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            parameter_index: u32::try_from(parameter_index).ok()?,
            expression,
            start: if range.start.is_valid() {
                Some(endpoint(
                    range.start,
                    CheckedScalarExpressionRole::ByteSequenceSubsliceStart {
                        call_ordinal,
                        argument_ordinal,
                    },
                )?)
            } else {
                None
            },
            end: if range.end.is_valid() {
                Some(endpoint(
                    range.end,
                    CheckedScalarExpressionRole::ByteSequenceSubsliceEnd {
                        call_ordinal,
                        argument_ordinal,
                    },
                )?)
            } else {
                None
            },
        },
        path: Vec::new(),
        type_identity,
        access: CheckedStructuralAccess::SharedBorrow,
    })
}

fn exact_borrowed_byte_view(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    matches!(program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Slice { element_type }
            if matches!(program.type_reference_table.type_reference(*element_type),
                TypeReferenceNode::Named { .. }))
        && structural_access_for_type_reference(program, reference)
            == Some(CheckedStructuralAccess::SharedBorrow)
        && byte_sequence_carrier(program, reference, &[])
            == Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
}
