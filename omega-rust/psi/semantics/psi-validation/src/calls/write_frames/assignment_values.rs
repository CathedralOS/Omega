//! Assignment-value admission for transparent returned-place relations.
//!
//! Positions carry the available type evidence. Traversal never manufactures
//! that evidence from expression depth, and every effectful call still needs
//! the shared complete, non-rebinding call-tree check.

use super::isolation::{
    struct_literal_field_type, struct_literal_matches_expected_type,
    struct_literal_type_is_caller_isolated, type_is_caller_isolated_local,
};
use super::{
    ExpressionHandle, ExpressionNode, Machine, ParameterRelativeFrameOrigin, StateParameter,
    SymbolHandle, TopLevelSymbols, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
    expression_is_effectful_for_transparent_result, free_machine_entry_state,
    machine_state_by_symbol, value_call_assignment_preserves_transparent_result,
};

#[derive(Clone, Copy)]
enum CallResultRequirement {
    NonReference,
    CallerIsolated,
}

#[derive(Clone, Copy)]
enum ValuePosition {
    Assignment(TypeReferenceHandle),
    AggregateElement(TypeReferenceHandle),
    ComputedOperand(CallResultRequirement),
    MemberReceiver(CallResultRequirement),
    IndexCollection(CallResultRequirement),
    ProjectedArrayElement,
}

impl ValuePosition {
    fn call_result_requirement(self) -> CallResultRequirement {
        match self {
            Self::ComputedOperand(requirement)
            | Self::MemberReceiver(requirement)
            | Self::IndexCollection(requirement) => requirement,
            Self::Assignment(_) | Self::AggregateElement(_) | Self::ProjectedArrayElement => {
                CallResultRequirement::NonReference
            }
        }
    }

    fn computed_operand_requirement(self, program: &TypedTrees) -> Option<CallResultRequirement> {
        match self {
            Self::Assignment(expected_type) => program
                .primitive_type_reference(expected_type)
                .map(|_| CallResultRequirement::CallerIsolated),
            Self::AggregateElement(expected_type) => program
                .primitive_type_reference(expected_type)
                .map(|_| CallResultRequirement::NonReference),
            _ => Some(self.call_result_requirement()),
        }
    }
}

type PendingValue = (ExpressionHandle, ValuePosition);

/// Check every eagerly evaluated child of the finite value expression.
/// Primitive computations and concrete aggregates compose without numeric
/// nesting limits. This is origin-preservation evidence, not a replacement for
/// expression typing or collection of the calls' published may-write paths.
#[allow(clippy::too_many_arguments)]
pub(super) fn value_expression_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    assignment_target_type: Option<TypeReferenceHandle>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    let mut pending = vec![(
        expression,
        ValuePosition::Assignment(assignment_target_type.unwrap_or_default()),
    )];
    while let Some((expression, position)) = pending.pop() {
        // Pure children are neutral after their container's shape is checked.
        // A pure root must still be classified: a reference alias replacement
        // belongs to the parent's origin-rebinding analysis, not this relation.
        if !matches!(position, ValuePosition::Assignment(_))
            && !expression_is_effectful_for_transparent_result(program, expression)
        {
            continue;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(_) => {
                let admitted = match position.call_result_requirement() {
                    CallResultRequirement::NonReference => {
                        value_call_assignment_preserves_transparent_result(
                            program,
                            current_machine,
                            expression,
                            symbols,
                            active_states,
                            parameters,
                            aliases,
                        )
                    }
                    CallResultRequirement::CallerIsolated => {
                        caller_isolated_value_call_assignment_preserves_transparent_result(
                            program,
                            current_machine,
                            expression,
                            symbols,
                            active_states,
                            parameters,
                            aliases,
                        )
                    }
                };
                if !admitted {
                    return false;
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                match position {
                    ValuePosition::Assignment(_) | ValuePosition::MemberReceiver(_) => {}
                    ValuePosition::AggregateElement(expected_type)
                        if struct_literal_matches_expected_type(
                            program,
                            literal,
                            expected_type,
                        ) => {}
                    _ => return false,
                }
                // Includes all declared fields and variants, not only the
                // authored active payload.
                if !struct_literal_type_is_caller_isolated(program, literal) {
                    return false;
                }
                for field in program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .rev()
                {
                    if !expression_is_effectful_for_transparent_result(program, field.value) {
                        continue;
                    }
                    let Some(field_type) =
                        struct_literal_field_type(program, literal, field.name.as_str())
                    else {
                        return false;
                    };
                    pending.push((field.value, ValuePosition::AggregateElement(field_type)));
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let elements = program.expression_table.expression_handles(*elements);
                let element_position = match position {
                    ValuePosition::Assignment(expected_type)
                    | ValuePosition::AggregateElement(expected_type) => {
                        let Some(element_type) =
                            typed_array_element_type(program, expected_type, elements.len())
                        else {
                            return false;
                        };
                        ValuePosition::AggregateElement(element_type)
                    }
                    ValuePosition::IndexCollection(_) | ValuePosition::ProjectedArrayElement => {
                        // The enclosing primitive projection has no contextual
                        // nominal element type. Do not guess one for record
                        // literal elements; arrays, calls and computations keep
                        // their existing independent admission rules.
                        ValuePosition::ProjectedArrayElement
                    }
                    _ => return false,
                };
                pending.extend(
                    elements
                        .iter()
                        .rev()
                        .map(|element| (*element, element_position)),
                );
            }
            ExpressionNode::Binary(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_) => {
                let Some(requirement) = position.computed_operand_requirement(program) else {
                    return false;
                };
                if !push_computed_operands(program, expression, requirement, &mut pending) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn typed_array_element_type(
    program: &TypedTrees,
    expected_type: TypeReferenceHandle,
    element_count: usize,
) -> Option<TypeReferenceHandle> {
    if !type_is_caller_isolated_local(program, expected_type) {
        return None;
    }
    let expected_type = crate::places::unwrapped_type_reference(program, expected_type)?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length: psi_typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(expected_type)
    else {
        return None;
    };
    if *length != element_count {
        return None;
    }
    crate::places::unwrapped_type_reference(program, *element_type)
}

fn push_computed_operands(
    program: &TypedTrees,
    expression: ExpressionHandle,
    requirement: CallResultRequirement,
    pending: &mut Vec<PendingValue>,
) -> bool {
    let operand = ValuePosition::ComputedOperand(requirement);
    // Reverse the push order so the worklist checks siblings in source order.
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            pending.push((binary.right, operand));
            pending.push((binary.left, operand));
        }
        ExpressionNode::Unary(unary) => pending.push((unary.operand, operand)),
        ExpressionNode::Cast(cast)
            if program.primitive_type_reference(cast.target_type).is_some() =>
        {
            pending.push((cast.value, operand));
        }
        ExpressionNode::Member(member) => {
            pending.push((member.receiver, ValuePosition::MemberReceiver(requirement)));
        }
        ExpressionNode::Indexed(indexed) => {
            pending.push((indexed.index, operand));
            pending.push((
                indexed.collection,
                ValuePosition::IndexCollection(requirement),
            ));
        }
        _ => return false,
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn caller_isolated_value_call_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some((_, callee_state)) =
        machine_state_by_symbol(program, call.target_symbol).or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })
    else {
        return false;
    };
    type_is_caller_isolated_local(program, callee_state.return_type)
        && value_call_assignment_preserves_transparent_result(
            program,
            current_machine,
            expression,
            symbols,
            active_states,
            parameters,
            aliases,
        )
}
