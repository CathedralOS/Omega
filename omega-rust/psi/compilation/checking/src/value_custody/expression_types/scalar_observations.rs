//! Scalar observation meaning, separate from a reference descriptor's type.
//!
//! Operator selection consumes the original declared operand types first.
//! After builtin selection, a readable scalar reference observes its referent,
//! with the original arithmetic qualification. This query grants no observed
//! value, preservation, range, availability or loan: the consuming capture or
//! live-subject owner must independently establish those facts.

use numerics::arithmetic::ArithmeticDomain;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Complete the operand references of an already selected builtin operation.
/// Result-type queries intentionally leave anonymous numbers unlanded. Their
/// first typed peer supplies only a fixed integer destination, never its range,
/// reference shell or arithmetic policy. Literal formation independently checks
/// the complete exact value before returning the canonical bare carrier.
/// Float literals keep their existing earlier format-landing owner.
pub(crate) fn builtin_scalar_operand_references(
    program: &dyn crate::CheckingInput,
    binary: &typed_trees::expression::TableBinaryExpression,
    references: [Option<TypeReferenceHandle>; 2],
    expected: Option<TypeReferenceHandle>,
) -> Option<[TypeReferenceHandle; 2]> {
    let reference = |index: usize| {
        if let Some(reference) = references[index] {
            return Some(reference);
        }
        let destination = references[1 - index].or(expected)?;
        let (primitive, _) = scalar_observation_meaning(program, destination)?;
        let primitive = if references[1 - index].is_some() {
            typed_trees::operator::integer_peer_operand_carrier(
                binary.operator,
                primitive,
                index == 1,
            )
        } else {
            primitive
        };
        if !primitive.is_integer() {
            return None;
        }
        crate::value_custody::literals::land_anonymous_integer_expression(
            program,
            [binary.left, binary.right][index],
            primitive,
            &[],
            |expression| {
                crate::value_custody::literals::has_anonymous_operator_meaning(program, expression)
            },
        )?;
        crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
            program,
            typed_trees::types::builtin_type_atom(primitive.landed_landing()?),
        )
    };
    Some([reference(0)?, reference(1)?])
}

pub(crate) fn scalar_observation_meaning(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    let mut reference = declared;
    let mut observed = declared;
    // Every readable crossing matters. A reference to write-only storage
    // cannot borrow a scalar carrier from the type hidden beneath that route.
    for _ in 0..=program.type_reference_table.type_reference_count() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => {
                reference = *referee;
                // Qualifications on a descriptor are not arithmetic policy
                // for the scalar stored through it. Retain the final referent's
                // complete qualified type while resolving its primitive.
                observed = *referee;
            }
            TypeReferenceNode::Reference { .. } => return None,
            _ => {
                return Some((
                    program.primitive_type_reference(reference)?,
                    program.arithmetic_domain_for_type_reference(observed),
                ));
            }
        }
    }
    None
}
