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
/// first typed peer supplies its comparison carrier or a fixed integer
/// arithmetic destination, never its range, reference shell or arithmetic
/// policy. Formation checks the complete exact value independently; observation
/// retains only the canonical bare carrier after builtin selection.
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
        if primitive == PrimitiveType::Addr {
            // A homogeneous address comparison has already selected its typed
            // peer. Retain that carrier while describing completed operands;
            // this is not an address offset/count conversion or a fit proof.
            // The operation-entry formation owner checks target-model fit
            // before this comparison's Boolean result becomes available.
            if !binary.operator.is_comparison()
                || references[1 - index].is_none()
                || !crate::value_custody::literals::has_anonymous_numeric_results(
                    program,
                    [binary.left, binary.right][index],
                )
            {
                return None;
            }
            let mut diagnostics = Vec::new();
            crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_primitive_range(
                program,
                primitive,
                [binary.left, binary.right][index],
                "completed address comparison",
                &mut diagnostics,
            );
            if !diagnostics.is_empty() {
                return None;
            }
        } else {
            if !primitive.is_integer() {
                return None;
            }
            // Fixed integers retain the ordinary exact landing owner. Addr
            // cannot use it: land_integer_value deliberately excludes a
            // target-dependent carrier from fixed-width literal landing.
            crate::value_custody::literals::land_anonymous_integer_expression(
                program,
                [binary.left, binary.right][index],
                primitive,
                &[],
                |expression| {
                    crate::value_custody::literals::has_anonymous_operator_meaning(
                        program, expression,
                    )
                },
            )?;
        }
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
    let observed = readable_observation_type_reference(program, declared)?;
    Some((
        program.primitive_type_reference(observed)?,
        program.arithmetic_domain_for_type_reference(observed),
    ))
}

/// Retain the complete readable value type without treating a reference
/// descriptor's qualifications as qualifications of its referent. Selection
/// still consumes the declared operand before this observation is interpreted.
pub(super) fn readable_observation_type_reference(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
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
            _ => return Some(observed),
        }
    }
    None
}

/// A completed choice may expose the same observation for every possible value
/// without sharing an arithmetic result policy. Keep the ordinary result-type
/// join strict: observing the resulting integer does not erase an arm's policy
/// at assignment, argument or return boundaries. Each arm still forms under
/// its own selected operation before this observation can become a premise.
pub(super) fn completed_observations_satisfy(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    reference: Option<TypeReferenceHandle>,
    expression_type: &mut dyn FnMut(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<TypeReferenceHandle>,
    accepts: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
        Option<TypeReferenceHandle>,
    ) -> Option<()>,
) -> bool {
    if reference.is_some() {
        return accepts(expression, reference).is_some();
    }
    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Atomic(atomic) => {
            let reference = expression_type(atomic.value);
            completed_observations_satisfy(
                program,
                atomic.value,
                reference,
                expression_type,
                accepts,
            )
        }
        typed_trees::expression::ExpressionNode::Match(dispatch) => {
            let arms = program.expression_table.match_arms(dispatch.arms);
            !arms.is_empty()
                && arms.iter().all(|arm| {
                    let reference = expression_type(arm.value);
                    completed_observations_satisfy(
                        program,
                        arm.value,
                        reference,
                        expression_type,
                        accepts,
                    )
                })
        }
        _ => accepts(expression, None).is_some(),
    }
}
