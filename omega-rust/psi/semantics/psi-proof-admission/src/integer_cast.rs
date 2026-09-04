//! Independently checked normalization for contiguous integer conversion words.
//!
//! This producer-visible prerequisite binds an ordered word of exact semantic
//! equalities to its root, target, carrier sequence, and exact surviving root
//! interval. It accepts no proof authority and is not a proof rule.

use psi_core::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerCastChainWitness {
    pub root: ScalarTerm,
    pub target: ScalarTerm,
    pub definition_axioms: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerCastChain {
    root: ScalarTerm,
    target: ScalarTerm,
    carriers: Vec<IntegerType>,
    definition_axioms: Vec<usize>,
    surviving_root_interval: Option<(i128, i128)>,
}

impl CheckedIntegerCastChain {
    pub const fn root(&self) -> &ScalarTerm {
        &self.root
    }

    pub const fn target(&self) -> &ScalarTerm {
        &self.target
    }

    pub fn carriers(&self) -> &[IntegerType] {
        &self.carriers
    }

    pub fn definition_axioms(&self) -> &[usize] {
        &self.definition_axioms
    }

    /// Exact mathematical root values that inhabit every carrier in the word.
    /// `None` denotes an empty intersection, not an unchecked failure.
    pub const fn surviving_root_interval(&self) -> Option<(i128, i128)> {
        self.surviving_root_interval
    }
}

pub fn check_integer_cast_chain_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerCastChainWitness,
) -> Result<CheckedIntegerCastChain, IntegerCastChainWitnessError> {
    if !matches!(witness.root, ScalarTerm::Value { .. }) {
        return Err(IntegerCastChainWitnessError::RootNotValue);
    }
    let ScalarType::Integer(root_type) = witness.root.scalar_type() else {
        return Err(IntegerCastChainWitnessError::RootNotInteger);
    };
    let mut interval = fixed_native_interval(root_type)
        .ok_or(IntegerCastChainWitnessError::UnsupportedCarrier(root_type))?;
    if witness.definition_axioms.is_empty() {
        return Err(IntegerCastChainWitnessError::EmptyCastChain);
    }
    if witness
        .definition_axioms
        .windows(2)
        .any(|indices| indices[0] >= indices[1])
    {
        return Err(IntegerCastChainWitnessError::NonCanonicalDefinitionOrder);
    }

    let mut current = witness.root.clone();
    let mut current_type = root_type;
    let mut carriers = vec![root_type];
    let mut values = vec![current.clone()];
    for &index in &witness.definition_axioms {
        let proposition = semantic_axioms
            .get(index)
            .ok_or(IntegerCastChainWitnessError::UnknownSemanticAxiom(index))?;
        context
            .validate(proposition)
            .map_err(IntegerCastChainWitnessError::MalformedProposition)?;
        let Proposition::Equal(next, definition) = proposition else {
            return Err(IntegerCastChainWitnessError::DefinitionNotEquality(index));
        };
        if !matches!(next, ScalarTerm::Value { .. }) {
            return Err(IntegerCastChainWitnessError::DefinitionTargetNotValue(
                index,
            ));
        }
        let (source_type, target_type, operand, exact_cast) = match definition {
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => (source_type, target_type, operand, true),
            ScalarTerm::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => (source_type, target_type, operand, false),
            _ => {
                return Err(IntegerCastChainWitnessError::DefinitionNotExactCast(index));
            }
        };
        if operand.as_ref() != &current
            || *source_type != current_type
            || next.scalar_type() != ScalarType::Integer(*target_type)
        {
            return Err(IntegerCastChainWitnessError::CastChainMismatch(index));
        }
        if exact_cast && !partial_fixed_native_cast(*source_type, *target_type) {
            return Err(IntegerCastChainWitnessError::NonPartialCastEdge {
                index,
                source: *source_type,
                target: *target_type,
            });
        }
        if !exact_cast && !strict_fixed_native_widen(*source_type, *target_type) {
            return Err(IntegerCastChainWitnessError::InvalidWidenEdge {
                index,
                source: *source_type,
                target: *target_type,
            });
        }
        if values.contains(next) {
            return Err(IntegerCastChainWitnessError::CyclicValue(index));
        }
        let target_interval = fixed_native_interval(*target_type).ok_or(
            IntegerCastChainWitnessError::UnsupportedCarrier(*target_type),
        )?;
        interval.0 = interval.0.max(target_interval.0);
        interval.1 = interval.1.min(target_interval.1);
        current = next.clone();
        current_type = *target_type;
        carriers.push(*target_type);
        values.push(current.clone());
    }
    if current != witness.target {
        return Err(IntegerCastChainWitnessError::TargetMismatch);
    }

    Ok(CheckedIntegerCastChain {
        root: witness.root.clone(),
        target: witness.target.clone(),
        carriers,
        definition_axioms: witness.definition_axioms.clone(),
        surviving_root_interval: (interval.0 <= interval.1).then_some(interval),
    })
}

fn partial_fixed_native_cast(source: IntegerType, target: IntegerType) -> bool {
    fixed_native_interval(source).is_some()
        && fixed_native_interval(target).is_some()
        && source != target
        && source.can_exact_cast_to(target)
        && !source.can_widen_to(target)
}

fn strict_fixed_native_widen(source: IntegerType, target: IntegerType) -> bool {
    fixed_native_interval(source).is_some()
        && fixed_native_interval(target).is_some()
        && source != target
        && source.can_widen_to(target)
}

fn fixed_native_interval(integer_type: IntegerType) -> Option<(i128, i128)> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    Some((
        integer_value_as_i128(integer_type, integer_type.minimum_value())?,
        integer_value_as_i128(integer_type, integer_type.maximum_value())?,
    ))
}

fn integer_value_as_i128(integer_type: IntegerType, value: IntegerValue) -> Option<i128> {
    match (integer_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => Some(value),
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => i128::try_from(value).ok(),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerCastChainWitnessError {
    RootNotValue,
    RootNotInteger,
    UnsupportedCarrier(IntegerType),
    EmptyCastChain,
    NonCanonicalDefinitionOrder,
    UnknownSemanticAxiom(usize),
    MalformedProposition(psi_core::PropositionError),
    DefinitionNotEquality(usize),
    DefinitionTargetNotValue(usize),
    DefinitionNotExactCast(usize),
    CastChainMismatch(usize),
    NonPartialCastEdge {
        index: usize,
        source: IntegerType,
        target: IntegerType,
    },
    InvalidWidenEdge {
        index: usize,
        source: IntegerType,
        target: IntegerType,
    },
    CyclicValue(usize),
    TargetMismatch,
}

impl std::fmt::Display for IntegerCastChainWitnessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IntegerCastChainWitnessError {}

/// Check an exact partial-cast-chain image of an independently proved root bound.
///
/// Every checked cast definition preserves the mathematical integer. The
/// conclusion must retain the root bound's orientation and express the same
/// endpoint in the chain's final carrier.
pub fn check_integer_cast_bound_conversion(
    chain: &CheckedIntegerCastChain,
    root_bound: &Proposition,
    conclusion: &Proposition,
) -> Result<(), IntegerCastBoundConversionError> {
    let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
        return Err(IntegerCastBoundConversionError::RootBoundNotLessOrEqual);
    };
    let (root_literal, root_is_left) = if root_left == chain.root() {
        (root_right, true)
    } else if root_right == chain.root() {
        (root_left, false)
    } else {
        return Err(IntegerCastBoundConversionError::RootBoundMismatch);
    };
    let source_type = chain.carriers()[0];
    let Some(root_value) = typed_integer_as_i128(root_literal, source_type) else {
        return Err(IntegerCastBoundConversionError::RootBoundNotTypedLiteral);
    };

    let Proposition::LessOrEqual(target_left, target_right) = conclusion else {
        return Err(IntegerCastBoundConversionError::ConclusionNotLessOrEqual);
    };
    let target_literal = if root_is_left {
        if target_left != chain.target() {
            return Err(IntegerCastBoundConversionError::ConclusionTargetMismatch);
        }
        target_right
    } else {
        if target_right != chain.target() {
            return Err(IntegerCastBoundConversionError::ConclusionTargetMismatch);
        }
        target_left
    };
    let target_type = *chain
        .carriers()
        .last()
        .expect("a checked cast chain retains its target carrier");
    let Some(target_value) = typed_integer_as_i128(target_literal, target_type) else {
        return Err(IntegerCastBoundConversionError::ConclusionNotTypedLiteral);
    };
    if target_value != root_value {
        return Err(IntegerCastBoundConversionError::ConclusionLiteralMismatch);
    }
    Ok(())
}

/// Exact target-carrier endpoint facts implied by every successfully produced
/// value of one checked partial-cast word.
pub fn integer_cast_truth_bounds(
    chain: &CheckedIntegerCastChain,
) -> Result<Vec<Proposition>, IntegerCastBoundConversionError> {
    let (minimum, maximum) = chain
        .surviving_root_interval()
        .ok_or(IntegerCastBoundConversionError::EmptySurvivingInterval)?;
    let target_type = *chain
        .carriers()
        .last()
        .expect("a checked cast chain retains its target carrier");
    let literal = |value| {
        let value = match target_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(value),
            IntegerSign::Unsigned => IntegerValue::Unsigned(
                u128::try_from(value)
                    .map_err(|_| IntegerCastBoundConversionError::ConclusionNotTypedLiteral)?,
            ),
        };
        ScalarTerm::integer(target_type, value)
            .map_err(|_| IntegerCastBoundConversionError::ConclusionNotTypedLiteral)
    };
    Ok(vec![
        Proposition::LessOrEqual(literal(minimum)?, chain.target().clone()),
        Proposition::LessOrEqual(chain.target().clone(), literal(maximum)?),
    ])
}

fn typed_integer_as_i128(term: &ScalarTerm, expected: IntegerType) -> Option<i128> {
    let (actual, value) = term.integer_value()?;
    (actual == expected)
        .then(|| integer_value_as_i128(actual, value))
        .flatten()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerCastBoundConversionError {
    RootBoundNotLessOrEqual,
    RootBoundMismatch,
    RootBoundNotTypedLiteral,
    ConclusionNotLessOrEqual,
    ConclusionTargetMismatch,
    ConclusionNotTypedLiteral,
    ConclusionLiteralMismatch,
    EmptySurvivingInterval,
}

impl std::fmt::Display for IntegerCastBoundConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IntegerCastBoundConversionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::ValueId;

    fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
        IntegerType::new(sign, bits).expect("fixed integer type")
    }

    fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(integer_type),
        )
    }

    fn cast_definition(
        target: ScalarTerm,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    ) -> Proposition {
        Proposition::Equal(
            target,
            ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                .expect("fixed exact cast term"),
        )
    }

    #[test]
    fn normalizes_single_and_multi_cast_cores_with_exact_carrier_intersection() {
        let i64_type = integer_type(IntegerSign::Signed, 64);
        let u64_type = integer_type(IntegerSign::Unsigned, 64);
        let i32_type = integer_type(IntegerSign::Signed, 32);
        let u8_type = integer_type(IntegerSign::Unsigned, 8);
        let root = value(1, i64_type);
        let first = value(2, u64_type);
        let second = value(3, i32_type);
        let target = value(4, u8_type);
        let axioms = vec![
            cast_definition(first.clone(), i64_type, u64_type, root.clone()),
            cast_definition(second.clone(), u64_type, i32_type, first.clone()),
            cast_definition(target.clone(), i32_type, u8_type, second),
        ];
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i64_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(u64_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(u8_type)),
        ])
        .unwrap();

        let single = check_integer_cast_chain_witness(
            &context,
            &axioms,
            &IntegerCastChainWitness {
                root: root.clone(),
                target: first,
                definition_axioms: vec![0],
            },
        )
        .expect("one partial cast is the shared sandwich core");
        assert_eq!(single.surviving_root_interval(), Some((0, i64::MAX.into())));

        let checked = check_integer_cast_chain_witness(
            &context,
            &axioms,
            &IntegerCastChainWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms: vec![0, 1, 2],
            },
        )
        .expect("ordered partial cast chain");
        assert_eq!(checked.root(), &root);
        assert_eq!(checked.target(), &target);
        assert_eq!(checked.carriers(), &[i64_type, u64_type, i32_type, u8_type],);
        assert_eq!(checked.definition_axioms(), &[0, 1, 2]);
        assert_eq!(checked.surviving_root_interval(), Some((0, u8::MAX.into())));
    }

    #[test]
    fn maps_contiguous_cast_bounds_and_rejects_shape_or_endpoint_drift() {
        let i16_type = integer_type(IntegerSign::Signed, 16);
        let i8_type = integer_type(IntegerSign::Signed, 8);
        let root = value(1, i16_type);
        let target = value(2, i8_type);
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        ])
        .unwrap();
        let chain = check_integer_cast_chain_witness(
            &context,
            &[cast_definition(
                target.clone(),
                i16_type,
                i8_type,
                root.clone(),
            )],
            &IntegerCastChainWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms: vec![0],
            },
        )
        .expect("single partial cast");
        let i16_one = ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).unwrap();
        let i8_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
        let root_bound = Proposition::LessOrEqual(i16_one, root);
        let conclusion = Proposition::LessOrEqual(i8_one.clone(), target.clone());
        assert_eq!(
            check_integer_cast_bound_conversion(&chain, &root_bound, &conclusion),
            Ok(()),
        );
        assert_eq!(
            check_integer_cast_bound_conversion(
                &chain,
                &root_bound,
                &Proposition::LessOrEqual(target, i8_one),
            ),
            Err(IntegerCastBoundConversionError::ConclusionTargetMismatch),
        );

        let i32_type = integer_type(IntegerSign::Signed, 32);
        let wide_root = value(3, i32_type);
        let middle = value(1, i16_type);
        let target = value(2, i8_type);
        let context = PropositionContext::from_value_types([
            (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        ])
        .unwrap();
        let chain = check_integer_cast_chain_witness(
            &context,
            &[
                cast_definition(middle.clone(), i32_type, i16_type, wide_root.clone()),
                cast_definition(target.clone(), i16_type, i8_type, middle),
            ],
            &IntegerCastChainWitness {
                root: wide_root.clone(),
                target,
                definition_axioms: vec![0, 1],
            },
        )
        .expect("two partial casts");
        let wide_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).unwrap(),
            wide_root,
        );
        assert_eq!(
            check_integer_cast_bound_conversion(&chain, &wide_bound, &conclusion),
            Ok(()),
        );
        assert_eq!(
            check_integer_cast_bound_conversion(
                &chain,
                &wide_bound,
                &Proposition::LessOrEqual(
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).unwrap(),
                    value(2, i8_type),
                ),
            ),
            Err(IntegerCastBoundConversionError::ConclusionLiteralMismatch),
        );
    }

    #[test]
    fn rejects_widening_stale_reordered_reversed_and_discontinuous_claims() {
        let i16_type = integer_type(IntegerSign::Signed, 16);
        let u16_type = integer_type(IntegerSign::Unsigned, 16);
        let u8_type = integer_type(IntegerSign::Unsigned, 8);
        let i8_type = integer_type(IntegerSign::Signed, 8);
        let root = value(1, i16_type);
        let first = value(2, u16_type);
        let target = value(3, i8_type);
        let axioms = vec![
            cast_definition(first.clone(), i16_type, u16_type, root.clone()),
            cast_definition(target.clone(), u16_type, i8_type, first.clone()),
        ];
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(u16_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(i8_type)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(u8_type)),
        ])
        .unwrap();
        let witness = |definition_axioms| IntegerCastChainWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms,
        };
        assert_eq!(
            check_integer_cast_chain_witness(&context, &axioms, &witness(vec![2])),
            Err(IntegerCastChainWitnessError::UnknownSemanticAxiom(2)),
        );
        assert_eq!(
            check_integer_cast_chain_witness(&context, &axioms, &witness(vec![1, 0])),
            Err(IntegerCastChainWitnessError::NonCanonicalDefinitionOrder),
        );
        assert_eq!(
            check_integer_cast_chain_witness(
                &context,
                &[Proposition::Equal(
                    ScalarTerm::integer_exact_cast(i16_type, u16_type, root.clone(),).unwrap(),
                    first.clone(),
                )],
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: first.clone(),
                    definition_axioms: vec![0],
                },
            ),
            Err(IntegerCastChainWitnessError::DefinitionTargetNotValue(0)),
        );
        assert_eq!(
            check_integer_cast_chain_witness(
                &context,
                &[cast_definition(
                    value(4, u8_type),
                    i16_type,
                    u8_type,
                    root.clone(),
                )],
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: value(4, u8_type),
                    definition_axioms: vec![0],
                },
            ),
            Ok(CheckedIntegerCastChain {
                root: root.clone(),
                target: value(4, u8_type),
                carriers: vec![i16_type, u8_type],
                definition_axioms: vec![0],
                surviving_root_interval: Some((0, u8::MAX.into())),
            }),
            "a narrowing edge is the intended partial core",
        );

        let u8_root = value(4, u8_type);
        let widened = value(1, i16_type);
        assert_eq!(
            check_integer_cast_chain_witness(
                &context,
                &[cast_definition(
                    widened.clone(),
                    u8_type,
                    i16_type,
                    u8_root.clone(),
                )],
                &IntegerCastChainWitness {
                    root: u8_root,
                    target: widened,
                    definition_axioms: vec![0],
                },
            ),
            Err(IntegerCastChainWitnessError::NonPartialCastEdge {
                index: 0,
                source: u8_type,
                target: i16_type,
            }),
        );
        assert_eq!(
            check_integer_cast_chain_witness(
                &context,
                &axioms,
                &IntegerCastChainWitness {
                    root,
                    target,
                    definition_axioms: vec![1],
                },
            ),
            Err(IntegerCastChainWitnessError::CastChainMismatch(1)),
        );
        assert_eq!(
            check_integer_cast_chain_witness(
                &context,
                &axioms,
                &IntegerCastChainWitness {
                    root: value(1, i16_type),
                    target: value(3, i8_type),
                    definition_axioms: vec![0],
                },
            ),
            Err(IntegerCastChainWitnessError::TargetMismatch),
        );
    }

    #[test]
    fn rejects_non_native_and_cyclic_cast_words() {
        let address_type = IntegerType::address(64).expect("address");
        let address_root = value(1, address_type);
        let address_context = PropositionContext::from_value_types([(
            ValueId::new(1).unwrap(),
            ScalarType::Integer(address_type),
        )])
        .unwrap();
        assert_eq!(
            check_integer_cast_chain_witness(
                &address_context,
                &[],
                &IntegerCastChainWitness {
                    root: address_root.clone(),
                    target: address_root,
                    definition_axioms: vec![0],
                },
            ),
            Err(IntegerCastChainWitnessError::UnsupportedCarrier(
                address_type,
            )),
        );

        let i128_type = integer_type(IntegerSign::Signed, 128);
        let u128_type = integer_type(IntegerSign::Unsigned, 128);
        let unsupported_root = value(1, i128_type);
        let unsupported_target = value(2, u128_type);
        let unsupported_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i128_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(u128_type)),
        ])
        .unwrap();
        assert_eq!(
            check_integer_cast_chain_witness(
                &unsupported_context,
                &[cast_definition(
                    unsupported_target.clone(),
                    i128_type,
                    u128_type,
                    unsupported_root.clone(),
                )],
                &IntegerCastChainWitness {
                    root: unsupported_root,
                    target: unsupported_target,
                    definition_axioms: vec![0],
                },
            ),
            Err(IntegerCastChainWitnessError::UnsupportedCarrier(i128_type)),
        );

        let i16_type = integer_type(IntegerSign::Signed, 16);
        let u16_type = integer_type(IntegerSign::Unsigned, 16);
        let root = value(1, i16_type);
        let middle = value(2, u16_type);
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(u16_type)),
        ])
        .unwrap();
        assert_eq!(
            check_integer_cast_chain_witness(
                &context,
                &[
                    cast_definition(middle.clone(), i16_type, u16_type, root.clone()),
                    cast_definition(root.clone(), u16_type, i16_type, middle),
                ],
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: root,
                    definition_axioms: vec![0, 1],
                },
            ),
            Err(IntegerCastChainWitnessError::CyclicValue(1)),
        );
    }
}
