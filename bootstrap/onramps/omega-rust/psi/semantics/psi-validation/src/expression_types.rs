use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::fmt;

mod cast_validation;
mod float_cast_proofs;
mod operator_validation;
mod shape_validation;
mod value_classification;

pub(crate) use cast_validation::validate_cast_types;

pub(crate) use operator_validation::{
    report_non_bool_logical_not, report_non_integer_bitwise_not, validate_binary_operand_types,
};

pub(crate) use shape_validation::{
    report_array_scalar_shape_mismatch, report_scalar_data_shape_mismatch,
};

#[allow(unused_imports)]
pub(crate) use value_classification::ValueClass;
pub(crate) use value_classification::{report_cross_class_store, report_data_type_conflict};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExpressionTypeOwner<'program> {
    StateTerminalExpression {
        machine: &'program str,
        state: &'program str,
    },
}

impl fmt::Display for ExpressionTypeOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateTerminalExpression { machine, state } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` terminal expression"
                )
            }
        }
    }
}

pub(crate) fn argument_matches_type_reference_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> bool {
    if let ExpressionNode::Borrow(inner_expression) = program.expression_table.expression(argument)
    {
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        return inner_expression.access == *access
            && argument_matches_type_reference_handle(program, inner_expression.target, *referee);
    }

    let argument_node = program.expression_table.expression(argument);

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } => {
            *access == psi_language_semantics::ReferenceAccess::Shared
                && argument_matches_type_reference_handle(program, argument, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            // A literal may directly establish an owned bounded text carrier
            // (`[u8; N] in Utf8`) in argument and terminal-result positions,
            // just as it already can in field/local writes. Do this before
            // erasing the value-domain constraint: the unconstrained base is
            // an always-full fixed array, which correctly does NOT accept a
            // text literal by itself.
            (matches!(argument_node, ExpressionNode::String(literal)
                if bounded_byte_buffer_capacity(program, type_reference)
                    .is_some_and(|capacity| literal.len() <= capacity))
                || argument_matches_type_reference_handle(program, argument, *base_type))
        }
        TypeReferenceNode::FixedArray { .. } => matches!(
            argument_node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Slice { element_type } => {
            // A string literal is a byte sequence, so it satisfies a `[u8]` slice
            // target (`&[u8] in Utf8 = "..."`) -- the basis for migrating string
            // literals to the `[u8] in Utf8` view. Other element types keep the
            // reference/place forms only.
            let element_is_u8 = matches!(
                program.type_reference_table.primitive_type(*element_type),
                Some(PrimitiveType::U8)
            );
            matches!(
                argument_node,
                ExpressionNode::Call(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
            ) || (element_is_u8 && matches!(argument_node, ExpressionNode::String(_)))
        }
        TypeReferenceNode::Generic { .. } => matches!(
            argument_node,
            ExpressionNode::Binary(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Unary(_)
        ),
        TypeReferenceNode::DynamicTrait { .. } => matches!(
            argument_node,
            ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Named {
            name: type_name, ..
        } => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                return matches!(argument_node, ExpressionNode::Boolean(_))
                    && primitive_type == PrimitiveType::Bool
                    || matches!(argument_node, ExpressionNode::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument_node, ExpressionNode::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(argument_node, ExpressionNode::Unary(unary)
                    if match unary.operator {
                        psi_typed_trees::expression::UnaryOperator::BitwiseNot => {
                            primitive_type.accepts_integer_literal()
                        }
                        psi_typed_trees::expression::UnaryOperator::LogicalNot => {
                            primitive_type == PrimitiveType::Bool
                        }
                    })
                    || matches!(
                        argument_node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Call(_)
                            | ExpressionNode::Cast(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Member(_)
                            | ExpressionNode::Name(_)
                            | ExpressionNode::StructLiteral(_)
                    );
            }

            matches!(
                argument_node,
                ExpressionNode::Binary(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::Unary(_)
            )
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => false,
    }
}

/// Mirror the backend layout classifier for an owned variable-fill text
/// carrier. A named value domain changes `[u8; N]` from an always-full fixed
/// array into `{len, bytes[N]}`; layout-policy domains do not.
fn bounded_byte_buffer_capacity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let has_value_domain = program
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .any(|constraint| match constraint {
            TypeConstraintNode::Domain(name) => {
                !psi_typed_trees::wire::is_layout_domain_constraint(name)
                    && psi_language_semantics::CarryPermission::from_name(name.as_str()).is_none()
            }
            _ => false,
        });
    if !has_value_domain {
        return None;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(*base_type)
    else {
        return None;
    };
    if program.type_reference_table.primitive_type(*element_type) != Some(PrimitiveType::U8) {
        return None;
    }
    match length {
        psi_typed_trees::types::FixedArrayLength::Literal(capacity) => Some(*capacity),
        psi_typed_trees::types::FixedArrayLength::ConstParameter { .. }
        | psi_typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
    }
}

pub(crate) fn validate_expression_type_handle(
    program: &TypedTrees,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ExpressionTypeOwner<'_>,
) {
    if let ExpressionNode::String(literal) = program.expression_table.expression(expression)
        && let Some(capacity) = bounded_byte_buffer_capacity(program, type_reference)
        && literal.len() > capacity
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} constructs {} byte(s), exceeding the {capacity}-byte capacity of `{}`",
            literal.len(),
            program.display_type_reference_with_constraints(type_reference),
        )));
        return;
    }
    if !argument_matches_type_reference_handle(program, expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, expression)
        )));
    }
}

pub(crate) fn expression_type_name_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
) -> &'static str {
    match program.expression_table.expression(argument) {
        ExpressionNode::Atomic(atomic) => expression_type_name_handle(program, atomic.value),
        ExpressionNode::ArrayLiteral(_) => "array literal",
        ExpressionNode::Binary(_) => "binary expression",
        ExpressionNode::Boolean(_) => "bool",
        ExpressionNode::Call(_) => "call expression",
        ExpressionNode::Cast(_) => "cast expression",
        ExpressionNode::Float(_) => "float literal",
        ExpressionNode::Indexed(_) => "indexed value",
        ExpressionNode::Integer(_) => "integer literal",
        ExpressionNode::Member(_) => "member access",
        ExpressionNode::Borrow(inner_expression) => {
            expression_type_name_handle(program, inner_expression.target)
        }
        ExpressionNode::Name(_) => "named value",
        ExpressionNode::Range(_) => "range expression",
        ExpressionNode::StructLiteral(_) => "struct literal",
        ExpressionNode::String(_) => "String",
        ExpressionNode::Unary(unary) => match unary.operator {
            psi_typed_trees::expression::UnaryOperator::BitwiseNot => "integer",
            psi_typed_trees::expression::UnaryOperator::LogicalNot => "bool",
        },
        ExpressionNode::ZeroValue(_) => "zero-value representation observation",
    }
}
