//! Immutable element-view ranges retain their source and evaluated endpoint
//! expressions.
use crate::execution::terminal_unit::CheckFacts;
use crate::execution::terminal_unit::CheckedScalarExpressionRole;
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
use crate::execution::terminal_unit::CheckedUnitStructuralParameterPlan;
use crate::execution::terminal_unit::ExpressionNode;
use crate::execution::terminal_unit::Multiplicity;
use crate::execution::terminal_unit::PrimitiveType;
use crate::execution::terminal_unit::TypeReferenceHandle;
use crate::execution::terminal_unit::TypeReferenceNode;
use crate::execution::terminal_unit::TypedTrees;
use crate::execution::terminal_unit::types::{
    borrowed_slice_view_element, borrowed_slice_view_type_identity,
    structural_access_for_type_reference,
};
use typed_trees::expression::ExpressionHandle;

/// Exclusive `s[a..b]` call argument over a borrowed element view. The byte
/// subslice's scalar roles name the same endpoints here: range bounds are
/// u64 element ordinals on either view kind.
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
    let (parameter_index, type_identity) = source(
        program,
        facts,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
    )?;
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
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
        source: CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            parameter_index,
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

/// Shared call and state-edge admission for an exact builtin borrowed element
/// range. Mirrors `byte_subslice::source` with the view carrier checks: the
/// source parameter must be the immutable whole `&[T]` view whose element
/// identity the target repeats.
pub(in crate::execution::terminal_unit) fn source(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
) -> Option<(u32, String)> {
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
    shape(
        program,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
    )
}

/// Element-view range admission without the operator-resolution veto. Lanes
/// that replay emitted subslice endpoint bindings inherit that veto through
/// the bindings' presence: no builtin range means no endpoints to rebind.
pub(in crate::execution) fn shape(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
) -> Option<(u32, String)> {
    borrowed_slice_view_element(program, target, &[])?;
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
    let type_identity = borrowed_slice_view_type_identity(program, target, &[], &[]);
    if parameter.access != CheckedStructuralAccess::SharedBorrow
        || parameter.multiplicity != Multiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || parameter.type_identity != type_identity
        || !exact_borrowed_element_view(program, source_type)
    {
        return None;
    }
    if !validation::has_builtin_subslice_meaning(program, machine, Some(state), expression) {
        return None;
    }
    Some((u32::try_from(parameter_index).ok()?, type_identity))
}

fn exact_borrowed_element_view(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Slice { .. }
    ) && structural_access_for_type_reference(program, reference)
        == Some(CheckedStructuralAccess::SharedBorrow)
        && borrowed_slice_view_element(program, reference, &[]).is_some()
}
