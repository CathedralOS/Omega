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
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// An arithmetic-policy-only integer result changes future operation meaning,
/// not the set of representable payloads. It owes no extra range predicate.
/// Other qualifications must retain their own result obligations.
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
    use symbols::BuiltinTypeAtom;
    program
        .symbols
        .builtin_type_atom(*symbol)
        .is_some_and(|atom| {
            name.as_str() == atom.symbol_name()
                && matches!(
                    atom,
                    BuiltinTypeAtom::U8
                        | BuiltinTypeAtom::U16
                        | BuiltinTypeAtom::U32
                        | BuiltinTypeAtom::U64
                        | BuiltinTypeAtom::I8
                        | BuiltinTypeAtom::I16
                        | BuiltinTypeAtom::I32
                        | BuiltinTypeAtom::I64
                )
        })
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
        _ => is_arithmetic_policy_only_integer(program, reference),
    };
    unrestricted
        .then(|| program.primitive_type_reference(reference))
        .flatten()
}

/// Describe a closed scalar output refinement without granting its proposition.
/// Consumers must publish the corresponding result guarantee and prove every
/// returning path. Other qualifications cannot disappear behind a range query.
pub fn closed_scalar_result_range(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(
    typed_trees::types::PrimitiveType,
    numerics::literals::IntegerLiteral,
    numerics::literals::IntegerLiteral,
)> {
    let mut current = reference;
    let mut count = 0usize;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(current)
    {
        let declared = program.type_reference_table.constraints(*constraints);
        if declared.len() != constraints.len() {
            return None;
        }
        for constraint in declared {
            if !matches!(constraint, TypeConstraintNode::Range { .. }) {
                return None;
            }
            count = count.checked_add(1)?;
        }
        current = *base_type;
    }
    if count != 1 {
        return None;
    }
    let primitive = program.primitive_type_reference(reference)?;
    let (_, [minimum, maximum], inclusive) = declared_integer_range(program, reference)?;
    let minimum = closed_integer_range_bound(program, minimum)?;
    let maximum = closed_integer_range_maximum(program, maximum, inclusive)?;
    if minimum > maximum {
        return None;
    }
    Some((
        primitive,
        crate::land_integer_value(&minimum, primitive)?,
        crate::land_integer_value(&maximum, primitive)?,
    ))
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

#[cfg(test)]
mod policy_tests {
    use super::{
        TypeReferenceHandle, TypeReferenceNode, TypedTrees, is_arithmetic_policy_only_integer,
    };

    fn field(spelling: &str) -> (TypedTrees, TypeReferenceHandle) {
        let source = format!("data Carrier {{ value: {spelling}; }}");
        let typed = crate::front_end::typed_program(&source);
        let [typed_trees::data::DataMember::Field(field)] =
            typed.data_members(&typed.data_definitions()[0])
        else {
            panic!("one field");
        };
        let reference = field.type_reference;
        (typed, reference)
    }

    #[test]
    fn arithmetic_policy_only_results_do_not_erase_value_refinements() {
        for (spelling, expected) in [
            ("u64 in Wrapping", true),
            ("i32 in Saturating", true),
            ("u8 in Trapping", true),
            ("u64", false),
            ("u64[0..=15]", false),
            ("u64[0..=15] in Wrapping", false),
            ("[u64 in Wrapping; 2]", false),
            ("&u64 in Wrapping", false),
        ] {
            let (typed, reference) = field(spelling);
            assert_eq!(
                is_arithmetic_policy_only_integer(&typed, reference),
                expected,
                "{spelling}"
            );
        }
    }

    #[test]
    fn arithmetic_policy_result_requires_live_exact_builtin_carrier() {
        let (mut typed, reference) = field("u64 in Wrapping");
        assert!(!is_arithmetic_policy_only_integer(
            &typed,
            TypeReferenceHandle::invalid()
        ));
        let TypeReferenceNode::Constrained { base_type, .. } =
            *typed.type_reference_table.type_reference(reference)
        else {
            panic!("policy shell");
        };
        let TypeReferenceNode::Named { name, .. } =
            typed.type_reference_table.type_reference(base_type).clone()
        else {
            panic!("integer carrier");
        };
        let carrier_symbol = typed.data_definitions()[0].symbol;
        typed.type_reference_table.substitute_node(
            base_type,
            TypeReferenceNode::Named {
                name,
                symbol: carrier_symbol,
            },
        );
        assert!(!is_arithmetic_policy_only_integer(&typed, reference));
    }
}
