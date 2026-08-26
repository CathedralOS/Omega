//! Closed denotation catalog for fixed-width integer policy bridges.
//!
//! This is semantic vocabulary, not an evaluator. Producers and independent
//! verifiers use the same closed row key, but must establish their own
//! operands, carrier bounds, formation evidence, and runtime crash edges.

use crate::arithmetic::ArithmeticDomain;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerPolicyPrimitive {
    Add,
    Subtract,
    Multiply,
    Divide,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerResultLaw {
    /// On formation or normal return, the embedded result is the unbounded
    /// mathematical result.
    Mathematical,
    /// Reduce the mathematical result into the selected carrier interval.
    WrapToCarrier,
    /// Clamp the mathematical result to the selected carrier interval.
    ClampToCarrier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerFormationCondition {
    NonZeroDivisor,
    ShiftCountWithinWidth,
    ResultRepresentable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerTrapPredicate {
    ResultOutsideCarrier,
    ZeroDivisor,
    SignedMinimumDividedByNegativeOne,
    ShiftCountOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShiftCountLaw {
    NotApplicable,
    MustBeWithinWidth,
    EuclideanModuloWidth,
    TrapsWhenOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerPolicyBridge {
    pub result_law: IntegerResultLaw,
    pub formation_conditions: &'static [IntegerFormationCondition],
    pub trap_predicates: &'static [IntegerTrapPredicate],
    pub shift_count_law: ShiftCountLaw,
}

const REPRESENTABLE: &[IntegerFormationCondition] =
    &[IntegerFormationCondition::ResultRepresentable];
const NONZERO_DIVISOR: &[IntegerFormationCondition] = &[IntegerFormationCondition::NonZeroDivisor];
const EXACT_DIVIDE: &[IntegerFormationCondition] = &[
    IntegerFormationCondition::NonZeroDivisor,
    IntegerFormationCondition::ResultRepresentable,
];
const SHIFT_COUNT: &[IntegerFormationCondition] =
    &[IntegerFormationCondition::ShiftCountWithinWidth];
const EXACT_SHIFT: &[IntegerFormationCondition] = &[
    IntegerFormationCondition::ShiftCountWithinWidth,
    IntegerFormationCondition::ResultRepresentable,
];
const TRAP_OVERFLOW: &[IntegerTrapPredicate] = &[IntegerTrapPredicate::ResultOutsideCarrier];
const TRAP_DIVIDE: &[IntegerTrapPredicate] = &[
    IntegerTrapPredicate::ZeroDivisor,
    IntegerTrapPredicate::SignedMinimumDividedByNegativeOne,
];
const TRAP_SHIFT: &[IntegerTrapPredicate] = &[
    IntegerTrapPredicate::ShiftCountOutOfRange,
    IntegerTrapPredicate::ResultOutsideCarrier,
];

/// Return the one settled bridge row for a fixed-width integer primitive and
/// arithmetic policy. The catalog deliberately names division and shift
/// exceptions instead of collapsing Trapping into a generic range test.
pub const fn integer_policy_bridge(
    primitive: IntegerPolicyPrimitive,
    policy: ArithmeticDomain,
) -> IntegerPolicyBridge {
    let result_law = match policy {
        ArithmeticDomain::Exact | ArithmeticDomain::Trapping => IntegerResultLaw::Mathematical,
        ArithmeticDomain::Wrapping => IntegerResultLaw::WrapToCarrier,
        ArithmeticDomain::Saturating => IntegerResultLaw::ClampToCarrier,
    };
    let formation_conditions = match (primitive, policy) {
        (IntegerPolicyPrimitive::Add, ArithmeticDomain::Exact)
        | (IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact)
        | (IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Exact) => REPRESENTABLE,
        (IntegerPolicyPrimitive::Divide, ArithmeticDomain::Exact) => EXACT_DIVIDE,
        (
            IntegerPolicyPrimitive::Divide,
            ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating,
        ) => NONZERO_DIVISOR,
        (IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Exact) => EXACT_SHIFT,
        (IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Saturating) => SHIFT_COUNT,
        (
            IntegerPolicyPrimitive::ShiftRight,
            ArithmeticDomain::Exact | ArithmeticDomain::Saturating,
        ) => SHIFT_COUNT,
        _ => &[],
    };
    let trap_predicates = match (primitive, policy) {
        (
            IntegerPolicyPrimitive::Add
            | IntegerPolicyPrimitive::Subtract
            | IntegerPolicyPrimitive::Multiply,
            ArithmeticDomain::Trapping,
        ) => TRAP_OVERFLOW,
        (IntegerPolicyPrimitive::Divide, ArithmeticDomain::Trapping) => TRAP_DIVIDE,
        (IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Trapping) => TRAP_SHIFT,
        (IntegerPolicyPrimitive::ShiftRight, ArithmeticDomain::Trapping) => {
            &[IntegerTrapPredicate::ShiftCountOutOfRange]
        }
        _ => &[],
    };
    let shift_count_law = match (primitive, policy) {
        (
            IntegerPolicyPrimitive::ShiftLeft | IntegerPolicyPrimitive::ShiftRight,
            ArithmeticDomain::Exact | ArithmeticDomain::Saturating,
        ) => ShiftCountLaw::MustBeWithinWidth,
        (
            IntegerPolicyPrimitive::ShiftLeft | IntegerPolicyPrimitive::ShiftRight,
            ArithmeticDomain::Wrapping,
        ) => ShiftCountLaw::EuclideanModuloWidth,
        (
            IntegerPolicyPrimitive::ShiftLeft | IntegerPolicyPrimitive::ShiftRight,
            ArithmeticDomain::Trapping,
        ) => ShiftCountLaw::TrapsWhenOutOfRange,
        _ => ShiftCountLaw::NotApplicable,
    };
    IntegerPolicyBridge {
        result_law,
        formation_conditions,
        trap_predicates,
        shift_count_law,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_policies_publish_distinct_result_laws() {
        let primitive = IntegerPolicyPrimitive::Add;
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Exact).result_law,
            IntegerResultLaw::Mathematical
        );
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Wrapping).result_law,
            IntegerResultLaw::WrapToCarrier
        );
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Saturating).result_law,
            IntegerResultLaw::ClampToCarrier
        );
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Trapping).result_law,
            IntegerResultLaw::Mathematical
        );
    }

    #[test]
    fn division_keeps_definedness_and_traps_policy_specific() {
        assert_eq!(
            integer_policy_bridge(IntegerPolicyPrimitive::Divide, ArithmeticDomain::Exact)
                .formation_conditions,
            EXACT_DIVIDE
        );
        for policy in [ArithmeticDomain::Wrapping, ArithmeticDomain::Saturating] {
            assert_eq!(
                integer_policy_bridge(IntegerPolicyPrimitive::Divide, policy).formation_conditions,
                NONZERO_DIVISOR
            );
        }
        let trapping =
            integer_policy_bridge(IntegerPolicyPrimitive::Divide, ArithmeticDomain::Trapping);
        assert!(trapping.formation_conditions.is_empty());
        assert_eq!(trapping.trap_predicates, TRAP_DIVIDE);
        assert_ne!(trapping.trap_predicates, TRAP_OVERFLOW);
    }

    #[test]
    fn shift_count_policy_matches_the_settled_catalog() {
        let primitive = IntegerPolicyPrimitive::ShiftLeft;
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Exact),
            IntegerPolicyBridge {
                result_law: IntegerResultLaw::Mathematical,
                formation_conditions: EXACT_SHIFT,
                trap_predicates: &[],
                shift_count_law: ShiftCountLaw::MustBeWithinWidth,
            }
        );
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Wrapping).shift_count_law,
            ShiftCountLaw::EuclideanModuloWidth
        );
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Saturating).formation_conditions,
            SHIFT_COUNT
        );
        assert_eq!(
            integer_policy_bridge(primitive, ArithmeticDomain::Trapping).trap_predicates,
            TRAP_SHIFT
        );

        let right = IntegerPolicyPrimitive::ShiftRight;
        assert_eq!(
            integer_policy_bridge(right, ArithmeticDomain::Exact).formation_conditions,
            SHIFT_COUNT
        );
        assert_eq!(
            integer_policy_bridge(right, ArithmeticDomain::Wrapping).shift_count_law,
            ShiftCountLaw::EuclideanModuloWidth
        );
        assert_eq!(
            integer_policy_bridge(right, ArithmeticDomain::Trapping).trap_predicates,
            &[IntegerTrapPredicate::ShiftCountOutOfRange]
        );
    }
}
