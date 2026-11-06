//! Declared range bounds share exact anonymous arithmetic with static inference.
//!
//! Keep the authored expression and its operator selections intact. Range
//! validation, proof construction and representation readers must agree on the
//! completed integer value; the syntax-only i64 folder truncates fractional
//! intermediates and cannot supply that semantic fact. Consumers with bounded
//! storage convert only the final value, without changing its denotation.

use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// An arithmetic-policy-only integer result changes future operation meaning,
/// not the set of representable payloads. It owes no extra range predicate.
/// Other qualifications must retain their own result obligations.
/// Whether a scalar's storage carries no qualification that owes its own
/// evidence: a bare named type, a runtime selector (`dyn Trait<Args>`, one
/// opaque word), or an integer qualified only by an arithmetic policy, which
/// changes later operations rather than the stored payload.
pub fn is_plain_scalar_storage(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    program
        .type_reference_table
        .contains_type_reference(reference)
        && (matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::DynamicTrait { .. }
        ) || is_arithmetic_policy_only_integer(program, reference))
}

pub fn is_arithmetic_policy_only_integer(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    let table = &program.type_reference_table;
    if !table.contains_type_reference(reference) {
        return false;
    }
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = table.type_reference(reference)
    else {
        return false;
    };
    if !table.contains_type_reference(*base_type)
        || !matches!(
            table.constraint_span(*constraints),
            Some([TypeConstraintNode::ArithmeticDomain(_)])
        )
    {
        return false;
    }
    let TypeReferenceNode::Named { symbol, name } = table.type_reference(*base_type) else {
        return false;
    };
    program
        .symbols
        .builtin_type_atom(*symbol)
        .is_some_and(|atom| name.as_str() == atom.symbol_name() && atom.is_fixed_integer())
}

/// The builtin primitive a place stores when its declaration adds no
/// membership restriction: a bare builtin, or an integer qualified only by an
/// arithmetic policy (`i32 in Wrapping`), which governs later operations on
/// the payload rather than which payloads it holds.
pub fn unrestricted_builtin_primitive(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    let table = &program.type_reference_table;
    if !table.contains_type_reference(reference) {
        return None;
    }
    let unrestricted = match table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, name } => program
            .symbols
            .builtin_type_atom(*symbol)
            .is_some_and(|atom| atom.symbol_name() == name.as_str()),
        // A selector is one unrestricted opaque word.
        TypeReferenceNode::DynamicTrait { .. } => true,
        _ => is_arithmetic_policy_only_integer(program, reference),
    };
    unrestricted
        .then(|| program.primitive_type_reference(reference))
        .flatten()
}

/// Read a closed integer range endpoint without interpreting typed
/// computations as anonymous arithmetic. Unknown, fractional and invalid
/// expressions supply no bound, including when an authored operator is selected.
pub fn closed_integer_range_bound(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    program.closed_integer_expression_value(expression)
}

/// Check the authored upper endpoint before normalizing its boundary kind.
/// Exclusive predecessor arithmetic never executes in the endpoint's carrier.
pub fn closed_integer_range_maximum(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    end_inclusive: bool,
) -> Option<BigInt> {
    program.closed_integer_range_endpoint(expression, end_inclusive)
}
