//! Declared range bounds share exact anonymous arithmetic with static inference.
//!
//! Keep the authored expression and its operator selections intact. Range
//! validation, proof construction and representation readers must agree on the
//! completed integer value; the syntax-only i64 folder truncates fractional
//! intermediates and cannot supply that semantic fact. Consumers with bounded
//! storage convert only the final value, without changing its denotation.

use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// An arithmetic-policy-only integer result changes future operation meaning,
/// not the set of representable payloads. It owes no extra range predicate.
/// Other qualifications must retain their own result obligations.
/// Whether a scalar's storage carries no qualification that owes its own
/// evidence: a bare named type, a runtime selector (`dyn Trait<Args>`, one
/// opaque word), or an integer qualified only by an arithmetic policy, which
/// changes later operations rather than the stored payload.
pub fn is_plain_scalar_storage(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    program
        .type_reference_table
        .contains_type_reference(reference)
        && (matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::DynamicTrait { .. }
        ) || is_arithmetic_policy_only_integer(program, reference))
}

/// The closed (minimum, maximum) of every `Range` constraint on `carrier`'s
/// `Constrained` shells. `strict` decides whether a non-Range constraint node
/// aborts the walk (true) or is skipped (false); a bound that does not close
/// aborts either way.
pub fn constrained_range_bounds(
    program: &TypedTrees,
    carrier: typed_trees::types::TypeReferenceHandle,
    strict: bool,
) -> Option<Vec<(BigInt, BigInt)>> {
    let (ranges, saw_non_range) = constrained_range_constraints(program, carrier);
    if strict && saw_non_range {
        return None;
    }
    let mut bounds = Vec::new();
    for (minimum, maximum, end_inclusive) in ranges {
        bounds.push(
            closed_integer_range_bound(program, minimum).zip(
                closed_integer_range_maximum(program, maximum, end_inclusive),
            )?,
        );
    }
    Some(bounds)
}

/// (minimum, maximum, end_inclusive) for every `Range` constraint on
/// `carrier`'s `Constrained` shells, in declaration order. `saw_non_range`
/// reports whether any constraint node was not a Range — strict walkers
/// reject on it, others skip the node.
pub fn constrained_range_constraints(
    program: &TypedTrees,
    mut carrier: TypeReferenceHandle,
) -> (Vec<(ExpressionHandle, ExpressionHandle, bool)>, bool) {
    let mut ranges = Vec::new();
    let mut saw_non_range = false;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(carrier)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            match constraint {
                TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } => ranges.push((*minimum, *maximum, *end_inclusive)),
                _ => saw_non_range = true,
            }
        }
        carrier = *base_type;
    }
    (ranges, saw_non_range)
}

pub fn is_arithmetic_policy_only_integer(
    program: &TypedTrees,
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
    program: &TypedTrees,
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

/// The exact integer carrier and single authored range, without intersecting
/// multiple declarations or converting symbolic endpoints to guessed values.
pub fn declared_integer_range(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<(symbols::BuiltinTypeAtom, [ExpressionHandle; 2], bool)> {
    if program.arithmetic_domain_for_type_reference(type_reference)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    let mut endpoints = None;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            if let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
                && endpoints
                    .replace(([*minimum, *maximum], *end_inclusive))
                    .is_some()
            {
                return None;
            }
        }
        type_reference = *base_type;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let carrier = program.symbols.builtin_type_atom(*symbol)?;
    use symbols::BuiltinTypeAtom;
    let (endpoints, end_inclusive) = endpoints?;
    matches!(
        carrier,
        BuiltinTypeAtom::U8
            | BuiltinTypeAtom::U16
            | BuiltinTypeAtom::U32
            | BuiltinTypeAtom::U64
            | BuiltinTypeAtom::I8
            | BuiltinTypeAtom::I16
            | BuiltinTypeAtom::I32
            | BuiltinTypeAtom::I64
    )
    .then_some((carrier, endpoints, end_inclusive))
}

/// Read a closed integer range endpoint without interpreting typed
/// computations as anonymous arithmetic. Unknown, fractional and invalid
/// expressions supply no bound, including when an authored operator is selected.
pub fn closed_integer_range_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    program.closed_integer_expression_value(expression)
}

/// Check the authored upper endpoint before normalizing its boundary kind.
/// Exclusive predecessor arithmetic never executes in the endpoint's carrier.
pub fn closed_integer_range_maximum(
    program: &TypedTrees,
    expression: ExpressionHandle,
    end_inclusive: bool,
) -> Option<BigInt> {
    program.closed_integer_range_endpoint(expression, end_inclusive)
}
