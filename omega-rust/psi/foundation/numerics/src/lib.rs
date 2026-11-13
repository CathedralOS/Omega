/*
We provide target-neutral exact numerics, integer overflow policies, captured
interval bounds, scalar predicate evaluation, IEEE float semantics, and literal
payload parsing for Psi. Our foundation types (`BigInt`, `BigRational`,
`ExactFloat`, `ArithmeticDomain`, `IntegerBoundStep`, `PredicateNode`, and
`FloatFormat`) give every compiler stage one dependency-free mathematical
substrate.
*/

/*
Fact evaluation, range proofs, and anonymous constant folding must never
silently wrap or truncate to host machine widths: a coefficient that overflows
a fixed-width integer would weaken or falsify a proof obligation, and a float
literal rounded before its landing format is known would double-round. We solve
this by keeping sign-magnitude arbitrary-precision integers and reduced
rationals alongside explicit policy bridges (`integer_policy_bridge`,
`ArithmeticPolicyAdapter`) and format-exact IEEE rounding rules.
*/

/*
We keep `arithmetic`, `integer_bounds`, `integer_policy`, `scalar_predicates`,
`float_representation`, and `float_projection` as public modules re-exported
from this root so source checking, proof verification, and Terminal Psi
lowering share the exact numeric laws without duplicating interval or predicate
evaluators.
*/

#![forbid(unsafe_code)]

pub(crate) use optimization::digest_encoder;

pub mod bignum;
pub mod float_semantics;
pub mod integer_carrier;
pub mod literals;

pub use float_semantics as float_semantics_catalog;

pub mod arithmetic {
    use crate::float_semantics::FloatFormat;

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum ArithmeticDomain {
        Exact,
        Wrapping,
        Saturating,
        Trapping,
    }

    impl ArithmeticDomain {
        pub fn from_name(name: &str) -> Option<Self> {
            match name {
                "Wrapping" => Some(Self::Wrapping),
                "Saturating" => Some(Self::Saturating),
                "Trapping" => Some(Self::Trapping),
                _ => None,
            }
        }

        pub fn name(self) -> &'static str {
            match self {
                Self::Exact => "Exact",
                Self::Wrapping => "Wrapping",
                Self::Saturating => "Saturating",
                Self::Trapping => "Trapping",
            }
        }

        /// The operator suffix that selects this policy for one operation:
        /// `a +% b` wraps, `a +| b` saturates, `a +! b` traps. The bare
        /// operator is Exact and has no suffix.
        pub const fn operator_suffix(self) -> &'static str {
            match self {
                Self::Exact => "",
                Self::Wrapping => "%",
                Self::Saturating => "|",
                Self::Trapping => "!",
            }
        }

        pub fn combine(self, other: Self) -> Self {
            match (self, other) {
                (Self::Exact, domain) | (domain, Self::Exact) => domain,
                (left, _) => left,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ArithmeticPolicyAdapter {
        #[default]
        None,
        FloatSaturatingOverflowOnly {
            format: FloatFormat,
        },
        FloatTrappingNonFinite {
            format: FloatFormat,
        },
    }

    impl ArithmeticPolicyAdapter {
        pub const fn float_format(self) -> Option<FloatFormat> {
            match self {
                Self::None => None,
                Self::FloatSaturatingOverflowOnly { format }
                | Self::FloatTrappingNonFinite { format } => Some(format),
            }
        }
    }
}

pub mod integer_bounds {
    use crate::bignum::BigInt;

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum IntegerBoundStep<Observation> {
        #[default]
        Missing,
        Bounds {
            low: BigInt,
            high: BigInt,
        },
        Observation(Observation),
        Add,
        Subtract,
        Multiply,
        Union,
        Interval,
        Intersect,
    }

    impl<Observation> IntegerBoundStep<Observation> {
        pub fn map_observation<Mapped, Error>(
            &self,
            observe: &mut impl FnMut(&Observation) -> Result<Mapped, Error>,
        ) -> Result<IntegerBoundStep<Mapped>, Error> {
            Ok(match self {
                Self::Missing => IntegerBoundStep::Missing,
                Self::Bounds { low, high } => IntegerBoundStep::Bounds {
                    low: low.clone(),
                    high: high.clone(),
                },
                Self::Observation(observation) => {
                    IntegerBoundStep::Observation(observe(observation)?)
                }
                Self::Add => IntegerBoundStep::Add,
                Self::Subtract => IntegerBoundStep::Subtract,
                Self::Multiply => IntegerBoundStep::Multiply,
                Self::Union => IntegerBoundStep::Union,
                Self::Interval => IntegerBoundStep::Interval,
                Self::Intersect => IntegerBoundStep::Intersect,
            })
        }
    }

    pub fn evaluate<Observation>(
        steps: &[IntegerBoundStep<Observation>],
        observe: &mut impl FnMut(&Observation) -> Result<BigInt, String>,
    ) -> Result<(BigInt, BigInt), String> {
        let mut values: Vec<(BigInt, BigInt)> = Vec::new();
        for step in steps {
            match step {
                IntegerBoundStep::Missing => return Err("missing integer bound step".to_owned()),
                IntegerBoundStep::Bounds { low, high } => {
                    if low > high {
                        return Err("inverted captured integer bounds".to_owned());
                    }
                    values.push((low.clone(), high.clone()));
                }
                IntegerBoundStep::Observation(observation) => {
                    let value = observe(observation)?;
                    values.push((value.clone(), value));
                }
                operation => {
                    let (right_low, right_high) = values
                        .pop()
                        .ok_or_else(|| "missing right integer bounds".to_owned())?;
                    let (left_low, left_high) = values
                        .pop()
                        .ok_or_else(|| "missing left integer bounds".to_owned())?;
                    values.push(match operation {
                        IntegerBoundStep::Add => {
                            (left_low.add(&right_low), left_high.add(&right_high))
                        }
                        IntegerBoundStep::Subtract => {
                            (left_low.sub(&right_high), left_high.sub(&right_low))
                        }
                        IntegerBoundStep::Interval => {
                            if left_low > right_high {
                                return Err("inverted contract bounds".to_owned());
                            }
                            (left_low, right_high)
                        }
                        IntegerBoundStep::Intersect => {
                            let low = left_low.max(right_low);
                            let high = left_high.min(right_high);
                            if low > high {
                                return Err("inconsistent captured contract bounds".to_owned());
                            }
                            (low, high)
                        }
                        IntegerBoundStep::Multiply => {
                            let corners = [
                                left_low.mul(&right_low),
                                left_low.mul(&right_high),
                                left_high.mul(&right_low),
                                left_high.mul(&right_high),
                            ];
                            let low = corners
                                .iter()
                                .min()
                                .cloned()
                                .ok_or_else(|| "missing product corners".to_owned())?;
                            let high = corners
                                .iter()
                                .max()
                                .cloned()
                                .ok_or_else(|| "missing product corners".to_owned())?;
                            (low, high)
                        }
                        IntegerBoundStep::Union => {
                            (left_low.min(right_low), left_high.max(right_high))
                        }
                        _ => return Err("invalid integer range operation".to_owned()),
                    });
                }
            }
        }
        if values.len() != 1 {
            return Err("integer range recipe does not produce one value".to_owned());
        }
        values
            .pop()
            .ok_or_else(|| "integer range recipe has no result".to_owned())
    }

    #[cfg(test)]
    mod tests {
        use super::{IntegerBoundStep, evaluate};
        use crate::bignum::BigInt;

        fn bounds(low: i64, high: i64) -> IntegerBoundStep<u32> {
            IntegerBoundStep::Bounds {
                low: BigInt::from_i64(low),
                high: BigInt::from_i64(high),
            }
        }

        #[test]
        fn preserves_signed_products_and_intersections() {
            let steps = [
                bounds(-3, 2),
                bounds(-4, 5),
                IntegerBoundStep::Multiply,
                bounds(-10, 11),
                IntegerBoundStep::Intersect,
            ];
            let result = evaluate(&steps, &mut |_| Err("unexpected observation".into()));
            assert_eq!(result, Ok((BigInt::from_i64(-10), BigInt::from_i64(11))));
        }

        #[test]
        fn joins_preserve_every_branch_bound() {
            let steps = [
                bounds(2, 4),
                bounds(10, 12),
                IntegerBoundStep::Union,
                bounds(3, 5),
                IntegerBoundStep::Subtract,
            ];
            let result = evaluate(&steps, &mut |_| Err("unexpected observation".into()));
            assert_eq!(result, Ok((BigInt::from_i64(-3), BigInt::from_i64(9))));
        }

        #[test]
        fn missing_observations_do_not_disappear_at_union() {
            let steps = [
                bounds(7, 7),
                IntegerBoundStep::Observation(42),
                IntegerBoundStep::Union,
            ];
            let result = evaluate(&steps, &mut |observation| {
                Err(format!("missing {observation}"))
            });
            assert_eq!(result, Err("missing 42".into()));
        }

        #[test]
        fn mapping_changes_only_observation_identity() {
            let source = [
                IntegerBoundStep::Observation(7u32),
                bounds(1, 1),
                IntegerBoundStep::Add,
            ];
            let mapped = source
                .iter()
                .map(|step| {
                    step.map_observation(&mut |observation| {
                        Ok::<_, String>(u64::from(*observation) + 100)
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .expect("mapped observation");
            let result = evaluate(&mapped, &mut |observation| {
                assert_eq!(*observation, 107);
                Ok(BigInt::from_i64(16))
            });
            assert_eq!(result, Ok((BigInt::from_i64(17), BigInt::from_i64(17))));
        }

        #[test]
        fn malformed_stacks_and_inconsistent_bounds_refuse() {
            for steps in [
                vec![],
                vec![IntegerBoundStep::Add],
                vec![bounds(0, 0), bounds(1, 1)],
                vec![bounds(1, 0)],
                vec![bounds(0, 1), bounds(2, 3), IntegerBoundStep::Intersect],
                vec![bounds(5, 6), bounds(1, 2), IntegerBoundStep::Interval],
            ] {
                assert!(evaluate(&steps, &mut |_| Err("unexpected observation".into())).is_err());
            }
        }
    }
}

pub mod integer_policy {
    use crate::arithmetic::ArithmeticDomain;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum IntegerPolicyPrimitive {
        Add,
        Subtract,
        Multiply,
        Divide,
        Remainder,
        ShiftLeft,
        ShiftRight,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum IntegerResultLaw {
        Mathematical,
        WrapToCarrier,
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
    const NONZERO_DIVISOR: &[IntegerFormationCondition] =
        &[IntegerFormationCondition::NonZeroDivisor];
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
            (
                IntegerPolicyPrimitive::Divide | IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Exact,
            ) => EXACT_DIVIDE,
            (
                IntegerPolicyPrimitive::Divide | IntegerPolicyPrimitive::Remainder,
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
            (
                IntegerPolicyPrimitive::Divide | IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Trapping,
            ) => TRAP_DIVIDE,
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
        use super::{
            ArithmeticDomain, EXACT_DIVIDE, EXACT_SHIFT, IntegerPolicyBridge,
            IntegerPolicyPrimitive, IntegerResultLaw, IntegerTrapPredicate, NONZERO_DIVISOR,
            SHIFT_COUNT, ShiftCountLaw, TRAP_DIVIDE, TRAP_OVERFLOW, TRAP_SHIFT,
            integer_policy_bridge,
        };

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
        fn division_and_remainder_keep_definedness_and_traps_policy_specific() {
            for primitive in [
                IntegerPolicyPrimitive::Divide,
                IntegerPolicyPrimitive::Remainder,
            ] {
                assert_eq!(
                    integer_policy_bridge(primitive, ArithmeticDomain::Exact).formation_conditions,
                    EXACT_DIVIDE,
                );
                for policy in [ArithmeticDomain::Wrapping, ArithmeticDomain::Saturating] {
                    assert_eq!(
                        integer_policy_bridge(primitive, policy).formation_conditions,
                        NONZERO_DIVISOR
                    );
                }
                let trapping = integer_policy_bridge(primitive, ArithmeticDomain::Trapping);
                assert!(trapping.formation_conditions.is_empty());
                assert_eq!(trapping.trap_predicates, TRAP_DIVIDE);
                assert_ne!(trapping.trap_predicates, TRAP_OVERFLOW);
            }
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
}

pub mod scalar_predicates {
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ScalarValue {
        #[default]
        Unknown,
        Integer(crate::bignum::BigInt),
        Boolean(bool),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PredicateValueKind {
        Integer,
        Boolean,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum PredicateOperator {
        And,
        Or,
        Equal,
        NotEqual,
        Less,
        LessOrEqual,
        Greater,
        GreaterOrEqual,
    }

    impl PredicateOperator {
        pub fn evaluate_comparison<T: Ord>(self, left: &T, right: &T) -> Option<bool> {
            Some(match self {
                Self::Equal => left == right,
                Self::NotEqual => left != right,
                Self::Less => left < right,
                Self::LessOrEqual => left <= right,
                Self::Greater => left > right,
                Self::GreaterOrEqual => left >= right,
                Self::And | Self::Or => return None,
            })
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum PredicateNode<Reference, Leaf> {
        #[default]
        Missing,
        Value(ScalarValue),
        Leaf(Leaf),
        Forward(Reference),
        Not(Reference),
        Binary {
            operator: PredicateOperator,
            left: Reference,
            right: Reference,
        },
    }

    impl<Reference: Copy, Leaf> PredicateNode<Reference, Leaf> {
        pub fn value_kind(
            &self,
            reference: &mut impl FnMut(Reference) -> Result<PredicateValueKind, &'static str>,
            leaf: &mut impl FnMut(&Leaf) -> Result<PredicateValueKind, &'static str>,
        ) -> Result<PredicateValueKind, &'static str> {
            use PredicateValueKind::{Boolean, Integer};
            match self.try_map(reference, leaf)? {
                PredicateNode::Missing | PredicateNode::Value(ScalarValue::Unknown) => {
                    Err("scalar predicate contains an absent value")
                }
                PredicateNode::Value(ScalarValue::Boolean(_)) => Ok(Boolean),
                PredicateNode::Value(ScalarValue::Integer(_)) => Ok(Integer),
                PredicateNode::Leaf(kind) | PredicateNode::Forward(kind) => Ok(kind),
                PredicateNode::Not(Boolean) => Ok(Boolean),
                PredicateNode::Not(Integer) => {
                    Err("scalar predicate negation requires a Boolean operand")
                }
                PredicateNode::Binary {
                    operator,
                    left,
                    right,
                } => {
                    let formed = match operator {
                        PredicateOperator::And | PredicateOperator::Or => {
                            left == Boolean && right == Boolean
                        }
                        PredicateOperator::Equal | PredicateOperator::NotEqual => left == right,
                        PredicateOperator::Less
                        | PredicateOperator::LessOrEqual
                        | PredicateOperator::Greater
                        | PredicateOperator::GreaterOrEqual => left == Integer && right == Integer,
                    };
                    if formed {
                        Ok(Boolean)
                    } else {
                        Err("scalar predicate operator has incompatible operand kinds")
                    }
                }
            }
        }

        pub fn try_map<MappedReference, MappedLeaf, Error>(
            &self,
            reference: &mut impl FnMut(Reference) -> Result<MappedReference, Error>,
            leaf: &mut impl FnMut(&Leaf) -> Result<MappedLeaf, Error>,
        ) -> Result<PredicateNode<MappedReference, MappedLeaf>, Error> {
            Ok(match self {
                Self::Missing => PredicateNode::Missing,
                Self::Value(value) => PredicateNode::Value(value.clone()),
                Self::Leaf(value) => PredicateNode::Leaf(leaf(value)?),
                Self::Forward(child) => PredicateNode::Forward(reference(*child)?),
                Self::Not(child) => PredicateNode::Not(reference(*child)?),
                Self::Binary {
                    operator,
                    left,
                    right,
                } => PredicateNode::Binary {
                    operator: *operator,
                    left: reference(*left)?,
                    right: reference(*right)?,
                },
            })
        }
    }

    pub fn evaluate_nodes<Reference: Copy + PartialEq, Leaf>(
        root: Reference,
        node: &mut impl FnMut(Reference) -> Option<PredicateNode<Reference, Leaf>>,
        resolve_leaf: &mut impl FnMut(Leaf) -> Option<ScalarValue>,
        comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
    ) -> Option<ScalarValue> {
        enum Step<Reference> {
            Enter(Reference),
            Leave,
            Not,
            Left(PredicateOperator, Reference),
            Right(PredicateOperator, ScalarValue),
        }
        let mut pending = vec![Step::Enter(root)];
        let mut active = Vec::new();
        let mut value = None;
        while let Some(step) = pending.pop() {
            match step {
                Step::Enter(reference) => {
                    if active.contains(&reference) {
                        return None;
                    }
                    active.push(reference);
                    pending.push(Step::Leave);
                    match node(reference)? {
                        PredicateNode::Missing => return None,
                        PredicateNode::Value(result) => value = Some(result),
                        PredicateNode::Leaf(leaf) => value = Some(resolve_leaf(leaf)?),
                        PredicateNode::Forward(child) => pending.push(Step::Enter(child)),
                        PredicateNode::Not(child) => {
                            pending.push(Step::Not);
                            pending.push(Step::Enter(child));
                        }
                        PredicateNode::Binary {
                            operator,
                            left,
                            right,
                        } => {
                            pending.push(Step::Left(operator, right));
                            pending.push(Step::Enter(left));
                        }
                    }
                }
                Step::Leave => {
                    active.pop()?;
                }
                Step::Not => {
                    let ScalarValue::Boolean(operand) = value.take()? else {
                        return None;
                    };
                    value = Some(ScalarValue::Boolean(!operand));
                }
                Step::Left(operator, right) => {
                    let left = value.take()?;
                    match (&left, operator) {
                        (ScalarValue::Boolean(false), PredicateOperator::And)
                        | (ScalarValue::Boolean(true), PredicateOperator::Or) => value = Some(left),
                        _ => {
                            pending.push(Step::Right(operator, left));
                            pending.push(Step::Enter(right));
                        }
                    }
                }
                Step::Right(operator, left) => {
                    let right = value.take()?;
                    if !matches!(operator, PredicateOperator::And | PredicateOperator::Or)
                        && !comparison_is_admitted(&left, &right)
                    {
                        return None;
                    }
                    let result = match (left, right) {
                        (ScalarValue::Boolean(left), ScalarValue::Boolean(right)) => match operator
                        {
                            PredicateOperator::And => left && right,
                            PredicateOperator::Or => left || right,
                            PredicateOperator::Equal => left == right,
                            PredicateOperator::NotEqual => left != right,
                            _ => return None,
                        },
                        (ScalarValue::Integer(left), ScalarValue::Integer(right)) => {
                            operator.evaluate_comparison(&left, &right)?
                        }
                        _ => return None,
                    };
                    value = Some(ScalarValue::Boolean(result));
                }
            }
        }
        value
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::bignum::BigInt;

        #[test]
        fn scalar_predicates_preserve_selected_comparisons_and_authority() {
            let wide = BigInt::from_u64(1).shl_bits(260);
            for (operator, expected) in [
                (PredicateOperator::Equal, false),
                (PredicateOperator::NotEqual, true),
                (PredicateOperator::Less, true),
                (PredicateOperator::LessOrEqual, true),
                (PredicateOperator::Greater, false),
                (PredicateOperator::GreaterOrEqual, false),
            ] {
                let nodes: [PredicateNode<usize, ()>; 3] = [
                    PredicateNode::Value(ScalarValue::Integer(wide.negate())),
                    PredicateNode::Value(ScalarValue::Integer(wide.clone())),
                    PredicateNode::Binary {
                        operator,
                        left: 0,
                        right: 1,
                    },
                ];
                assert_eq!(
                    evaluate_nodes(
                        2,
                        &mut |reference| nodes.get(reference).cloned(),
                        &mut |_| None,
                        &|_, _| true
                    ),
                    Some(ScalarValue::Boolean(expected))
                );
                assert_eq!(
                    evaluate_nodes(
                        2,
                        &mut |reference| nodes.get(reference).cloned(),
                        &mut |_| None,
                        &|_, _| false
                    ),
                    None
                );
            }
        }

        #[test]
        fn scalar_predicates_short_circuit_without_observing_skipped_leaves() {
            for (operator, left, skipped) in [
                (PredicateOperator::And, false, true),
                (PredicateOperator::And, true, false),
                (PredicateOperator::Or, true, true),
                (PredicateOperator::Or, false, false),
            ] {
                let nodes = [
                    PredicateNode::Value(ScalarValue::Boolean(left)),
                    PredicateNode::Leaf(73u32),
                    PredicateNode::Binary {
                        operator,
                        left: 0usize,
                        right: 1,
                    },
                ];
                let mut observed = Vec::new();
                let result = evaluate_nodes(
                    2,
                    &mut |reference| nodes.get(reference).cloned(),
                    &mut |leaf| {
                        observed.push(leaf);
                        None
                    },
                    &|_, _| false,
                );
                assert_eq!(observed.is_empty(), skipped);
                assert_eq!(result, skipped.then_some(ScalarValue::Boolean(left)));
            }
        }

        #[test]
        fn scalar_predicates_do_not_infer_boolean_comparison_authority() {
            let nodes: [PredicateNode<usize, ()>; 4] = [
                PredicateNode::Value(ScalarValue::Boolean(true)),
                PredicateNode::Value(ScalarValue::Boolean(false)),
                PredicateNode::Binary {
                    operator: PredicateOperator::NotEqual,
                    left: 0,
                    right: 1,
                },
                PredicateNode::Not(2),
            ];
            assert_eq!(
                evaluate_nodes(
                    3,
                    &mut |reference| nodes.get(reference).cloned(),
                    &mut |_| None,
                    &|_, _| true
                ),
                Some(ScalarValue::Boolean(false))
            );
            assert_eq!(
                evaluate_nodes(
                    3,
                    &mut |reference| nodes.get(reference).cloned(),
                    &mut |_| None,
                    &|_, _| false
                ),
                None
            );
        }

        #[test]
        fn scalar_predicates_refuse_missing_cyclic_and_mismatched_values() {
            let cyclic: [PredicateNode<usize, ()>; 2] =
                [PredicateNode::Forward(1), PredicateNode::Not(0)];
            assert_eq!(
                evaluate_nodes(
                    0,
                    &mut |reference| cyclic.get(reference).cloned(),
                    &mut |_| None,
                    &|_, _| true
                ),
                None
            );
            assert_eq!(
                evaluate_nodes(
                    7,
                    &mut |reference| cyclic.get(reference).cloned(),
                    &mut |_| None,
                    &|_, _| true
                ),
                None
            );
            let mixed: [PredicateNode<usize, ()>; 3] = [
                PredicateNode::Value(ScalarValue::Integer(BigInt::from_u64(1))),
                PredicateNode::Value(ScalarValue::Boolean(true)),
                PredicateNode::Binary {
                    operator: PredicateOperator::Equal,
                    left: 0,
                    right: 1,
                },
            ];
            assert_eq!(
                evaluate_nodes(
                    2,
                    &mut |reference| mixed.get(reference).cloned(),
                    &mut |_| None,
                    &|_, _| true
                ),
                None
            );
        }

        #[test]
        fn scalar_predicate_publication_maps_both_children_before_evaluation() {
            let selected: PredicateNode<u32, ()> = PredicateNode::Binary {
                operator: PredicateOperator::And,
                left: 11,
                right: 12,
            };
            let mut mapped = Vec::new();
            let result = selected.try_map(
                &mut |child| {
                    mapped.push(child);
                    if child == 12 {
                        Err("missing formation")
                    } else {
                        Ok(child + 100)
                    }
                },
                &mut |leaf| {
                    let _: () = *leaf;
                    Ok(())
                },
            );
            assert_eq!(mapped, vec![11, 12]);
            assert_eq!(result, Err("missing formation"));
        }
    }
}

pub mod float_representation {
    pub fn binary32_from_bits(bits: u32) -> f64 {
        let value = f32::from_bits(bits);
        if !value.is_nan() {
            return value as f64;
        }
        let sign = (u64::from(bits) >> 31) << 63;
        let payload = (u64::from(bits) & 0x007f_ffff) << 29;
        f64::from_bits(sign | 0x7ff0_0000_0000_0000 | payload)
    }

    pub fn binary32_to_bits(value: f64) -> u32 {
        if !value.is_nan() {
            return (value as f32).to_bits();
        }
        let bits = value.to_bits();
        let sign = ((bits >> 63) as u32) << 31;
        let mut payload = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32;
        payload &= 0x007f_ffff;
        if payload == 0 {
            payload = 0x0040_0000;
        }
        sign | 0x7f80_0000 | payload
    }
}

pub mod float_projection {
    use crate::float_semantics::{FloatFormat, FloatMeaning};

    pub const FLOAT_PROJECTION_CORE_SOURCE: &str = "float_operations.omg";
    pub const FLOAT_MEANING_CORE_SOURCE: &str = "float_meaning.omg";
    pub const FLOAT_PROJECTION_CATALOG_VERSION: u16 = 1;
    pub const FLOAT_MEANING_RESULT_IDENTITY: &str = "toolchain::FloatMeaning";
    const FLOAT_PROJECTION_CONTRACT_DOMAIN: &[u8] = b"psi-float-projection-contract\0";

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct FloatProjectionContractIdentity {
        pub format: u16,
        pub operation: u8,
        pub declaration: u8, // Rooted-checker declaration tag for the recognized projection.
        pub catalog_version: u16,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum FloatProjectionOperation {
        Meaning32,
        Meaning64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FloatProjectionRule {
        pub source_format: FloatFormat,
        pub finite_nonzero_is_exact_rational: bool,
        pub preserves_signed_zero: bool,
        pub preserves_signed_infinity: bool,
        pub erases_nan_payload: bool,
    }

    impl FloatProjectionOperation {
        pub fn contract_identity(self) -> FloatProjectionContractIdentity {
            let (format, operation, declaration, source_carrier) = match self {
                Self::Meaning32 => (32_u16, 1_u8, 1_u8, "f32"),
                Self::Meaning64 => (64_u16, 2_u8, 2_u8, "f64"),
            };
            let operator_identity = format!(
                "toolchain::{}::{}",
                self.source_namespace(),
                self.source_name(),
            );
            let mut hasher =
                crate::digest_encoder::DomainDigestEncoder::new(FLOAT_PROJECTION_CONTRACT_DOMAIN);
            hasher.raw(&format.to_le_bytes());
            hasher.byte(operation);
            hasher.byte(declaration);
            hasher.raw(&FLOAT_PROJECTION_CATALOG_VERSION.to_le_bytes());
            for owner in [
                FLOAT_PROJECTION_CORE_SOURCE,
                FLOAT_MEANING_CORE_SOURCE,
                operator_identity.as_str(),
                "private-contract-free-ordinary-tokenless-one-parameter",
                source_carrier,
                FLOAT_MEANING_RESULT_IDENTITY,
            ] {
                hasher.u64(owner.len() as u64);
                hasher.raw(owner.as_bytes());
            }
            FloatProjectionContractIdentity {
                format,
                operation,
                declaration,
                catalog_version: FLOAT_PROJECTION_CATALOG_VERSION,
                commitment: hasher.finish(),
            }
        }

        pub const fn source_namespace(self) -> &'static str {
            "Float"
        }

        pub const fn source_name(self) -> &'static str {
            match self {
                Self::Meaning32 => "meaning32",
                Self::Meaning64 => "meaning64",
            }
        }

        pub fn from_source_identity(namespace: &str, name: &str) -> Option<Self> {
            match (namespace, name) {
                ("Float", "meaning32") => Some(Self::Meaning32),
                ("Float", "meaning64") => Some(Self::Meaning64),
                _ => None,
            }
        }

        pub const fn rule(self) -> FloatProjectionRule {
            FloatProjectionRule {
                source_format: match self {
                    Self::Meaning32 => FloatFormat::BINARY32,
                    Self::Meaning64 => FloatFormat::BINARY64,
                },
                finite_nonzero_is_exact_rational: true,
                preserves_signed_zero: true,
                preserves_signed_infinity: true,
                erases_nan_payload: true,
            }
        }

        pub fn project_f32(self, value: f32) -> Option<FloatMeaning> {
            (self == Self::Meaning32).then(|| FloatMeaning::from_f32(value))
        }

        pub fn project_f64(self, value: f64) -> Option<FloatMeaning> {
            (self == Self::Meaning64).then(|| FloatMeaning::from_f64(value))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            FLOAT_PROJECTION_CATALOG_VERSION, FloatFormat, FloatMeaning, FloatProjectionOperation,
        };

        #[test]
        fn projection_rows_are_format_exact() {
            assert_eq!(
                FloatProjectionOperation::Meaning32.rule().source_format,
                FloatFormat::BINARY32
            );
            assert_eq!(
                FloatProjectionOperation::Meaning64.rule().source_format,
                FloatFormat::BINARY64
            );
            assert!(
                FloatProjectionOperation::Meaning32
                    .project_f64(1.0)
                    .is_none()
            );
            assert!(
                FloatProjectionOperation::Meaning64
                    .project_f32(1.0)
                    .is_none()
            );
        }

        #[test]
        fn source_identities_are_exact_and_closed() {
            for operation in [
                FloatProjectionOperation::Meaning32,
                FloatProjectionOperation::Meaning64,
            ] {
                assert_eq!(
                    FloatProjectionOperation::from_source_identity(
                        operation.source_namespace(),
                        operation.source_name(),
                    ),
                    Some(operation),
                );
            }
            assert_eq!(
                FloatProjectionOperation::from_source_identity("Other", "meaning32"),
                None,
            );
            assert_eq!(
                FloatProjectionOperation::from_source_identity("Float", "meaning"),
                None,
            );
            assert_ne!(
                FloatProjectionOperation::Meaning32.contract_identity(),
                FloatProjectionOperation::Meaning64.contract_identity(),
            );
            assert_eq!(
                FloatProjectionOperation::Meaning32
                    .contract_identity()
                    .catalog_version,
                FLOAT_PROJECTION_CATALOG_VERSION,
            );
            let binary32 = FloatProjectionOperation::Meaning32.contract_identity();
            assert_eq!(
                (binary32.format, binary32.operation, binary32.declaration),
                (32, 1, 1),
            );
            let binary64 = FloatProjectionOperation::Meaning64.contract_identity();
            assert_eq!(
                (binary64.format, binary64.operation, binary64.declaration),
                (64, 2, 2),
            );
        }

        #[test]
        fn projections_preserve_signs_but_erase_nan_payloads() {
            let projection = FloatProjectionOperation::Meaning32;
            assert_ne!(projection.project_f32(0.0), projection.project_f32(-0.0));
            assert_ne!(
                projection.project_f32(f32::INFINITY),
                projection.project_f32(f32::NEG_INFINITY)
            );
            assert_eq!(
                projection.project_f32(f32::from_bits(0x7fc0_0001)),
                projection.project_f32(f32::from_bits(0x7fff_ffff))
            );
        }

        #[test]
        fn finite_nonzero_projection_is_exact_and_format_specific() {
            let binary32 = FloatProjectionOperation::Meaning32
                .project_f32(0.1)
                .expect("binary32 projection");
            let binary64 = FloatProjectionOperation::Meaning64
                .project_f64(0.1)
                .expect("binary64 projection");
            assert!(matches!(binary32, FloatMeaning::FiniteNonZero(_)));
            assert!(matches!(binary64, FloatMeaning::FiniteNonZero(_)));
            assert_ne!(binary32, binary64, "each landed format projects exactly");
        }
    }
}
