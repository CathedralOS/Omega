//! Independently checked normalization for exact integer-shift words.
//!
//! The checked form retains every exact operation definition and every
//! independently landed count fact. It accepts no proof authority and is not a
//! proof rule.

use psi_core::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerShiftDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerShiftStepWitness {
    pub definition_axiom: usize,
    /// Exact prior equality that lands a non-closed count term.
    pub count_axiom: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerShiftChainWitness {
    pub root: ScalarTerm,
    pub target: ScalarTerm,
    pub steps: Vec<IntegerShiftStepWitness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerShiftStep {
    direction: IntegerShiftDirection,
    count_type: IntegerType,
    count: u128,
    definition_axiom: usize,
    count_axiom: Option<usize>,
}

impl CheckedIntegerShiftStep {
    pub const fn direction(&self) -> IntegerShiftDirection {
        self.direction
    }

    pub const fn count_type(&self) -> IntegerType {
        self.count_type
    }

    pub const fn count(&self) -> u128 {
        self.count
    }

    pub const fn definition_axiom(&self) -> usize {
        self.definition_axiom
    }

    pub const fn count_axiom(&self) -> Option<usize> {
        self.count_axiom
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerShiftChain {
    root: ScalarTerm,
    target: ScalarTerm,
    value_type: IntegerType,
    steps: Vec<CheckedIntegerShiftStep>,
}

impl CheckedIntegerShiftChain {
    pub const fn root(&self) -> &ScalarTerm {
        &self.root
    }

    pub const fn target(&self) -> &ScalarTerm {
        &self.target
    }

    pub const fn value_type(&self) -> IntegerType {
        self.value_type
    }

    pub fn steps(&self) -> &[CheckedIntegerShiftStep] {
        &self.steps
    }
}

pub fn check_integer_shift_chain_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerShiftChainWitness,
) -> Result<CheckedIntegerShiftChain, IntegerShiftChainWitnessError> {
    if !matches!(witness.root, ScalarTerm::Value { .. }) {
        return Err(IntegerShiftChainWitnessError::RootNotValue);
    }
    let ScalarType::Integer(value_type) = witness.root.scalar_type() else {
        return Err(IntegerShiftChainWitnessError::RootNotInteger);
    };
    if !fixed_native(value_type) {
        return Err(IntegerShiftChainWitnessError::UnsupportedValueCarrier(
            value_type,
        ));
    }
    if witness.steps.is_empty() {
        return Err(IntegerShiftChainWitnessError::EmptyShiftChain);
    }
    if witness
        .steps
        .windows(2)
        .any(|steps| steps[0].definition_axiom >= steps[1].definition_axiom)
    {
        return Err(IntegerShiftChainWitnessError::NonCanonicalDefinitionOrder);
    }

    let mut current = witness.root.clone();
    let mut values = vec![current.clone()];
    let mut checked_steps = Vec::with_capacity(witness.steps.len());
    for step in &witness.steps {
        let index = step.definition_axiom;
        let proposition = checked_axiom(context, semantic_axioms, index)?;
        let Proposition::Equal(next, definition) = proposition else {
            return Err(IntegerShiftChainWitnessError::DefinitionNotEquality(index));
        };
        if !matches!(next, ScalarTerm::Value { .. }) {
            return Err(IntegerShiftChainWitnessError::DefinitionTargetNotValue(
                index,
            ));
        }
        let (direction, nested_value_type, count_type, operand, count_term) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => (
                IntegerShiftDirection::Left,
                *value_type,
                *count_type,
                value,
                count,
            ),
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => (
                IntegerShiftDirection::Right,
                *value_type,
                *count_type,
                value,
                count,
            ),
            _ => {
                return Err(IntegerShiftChainWitnessError::DefinitionNotExactShift(
                    index,
                ));
            }
        };
        if nested_value_type != value_type
            || operand.as_ref() != &current
            || next.scalar_type() != ScalarType::Integer(value_type)
        {
            return Err(IntegerShiftChainWitnessError::ShiftChainMismatch(index));
        }
        if !fixed_native(count_type) {
            return Err(IntegerShiftChainWitnessError::UnsupportedCountCarrier {
                index,
                count_type,
            });
        }
        let count = checked_count(
            context,
            semantic_axioms,
            index,
            step.count_axiom,
            count_type,
            count_term,
        )?;
        if count >= u128::from(value_type.bits()) {
            return Err(IntegerShiftChainWitnessError::CountOutsideValueWidth {
                index,
                count,
                width: value_type.bits(),
            });
        }
        if values.contains(next) {
            return Err(IntegerShiftChainWitnessError::CyclicValue(index));
        }
        current = next.clone();
        values.push(current.clone());
        checked_steps.push(CheckedIntegerShiftStep {
            direction,
            count_type,
            count,
            definition_axiom: index,
            count_axiom: step.count_axiom,
        });
    }
    if current != witness.target {
        return Err(IntegerShiftChainWitnessError::TargetMismatch);
    }

    Ok(CheckedIntegerShiftChain {
        root: witness.root.clone(),
        target: witness.target.clone(),
        value_type,
        steps: checked_steps,
    })
}

fn checked_count(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_axiom: usize,
    count_axiom: Option<usize>,
    count_type: IntegerType,
    count_term: &ScalarTerm,
) -> Result<u128, IntegerShiftChainWitnessError> {
    let value = if let Some((actual_type, value)) = count_term.integer_value() {
        if count_axiom.is_some() {
            return Err(IntegerShiftChainWitnessError::UnexpectedCountAxiom(
                definition_axiom,
            ));
        }
        if actual_type != count_type {
            return Err(IntegerShiftChainWitnessError::CountTypeMismatch(
                definition_axiom,
            ));
        }
        value
    } else {
        let count_axiom = count_axiom.ok_or(IntegerShiftChainWitnessError::MissingCountAxiom(
            definition_axiom,
        ))?;
        if count_axiom >= definition_axiom {
            return Err(IntegerShiftChainWitnessError::CountAxiomNotPrior {
                definition_axiom,
                count_axiom,
            });
        }
        let proposition = checked_axiom(context, semantic_axioms, count_axiom)?;
        let Proposition::Equal(left, right) = proposition else {
            return Err(IntegerShiftChainWitnessError::CountAxiomNotEquality(
                count_axiom,
            ));
        };
        if left != count_term {
            return Err(IntegerShiftChainWitnessError::CountAxiomMismatch(
                count_axiom,
            ));
        }
        let Some((actual_type, value)) = right.integer_value() else {
            return Err(IntegerShiftChainWitnessError::CountAxiomMismatch(
                count_axiom,
            ));
        };
        if actual_type != count_type {
            return Err(IntegerShiftChainWitnessError::CountTypeMismatch(
                definition_axiom,
            ));
        }
        value
    };
    match (count_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => u128::try_from(value)
            .map_err(|_| IntegerShiftChainWitnessError::NegativeCount(definition_axiom)),
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => Ok(value),
        _ => Err(IntegerShiftChainWitnessError::CountTypeMismatch(
            definition_axiom,
        )),
    }
}

fn checked_axiom<'a>(
    context: &PropositionContext,
    semantic_axioms: &'a [Proposition],
    index: usize,
) -> Result<&'a Proposition, IntegerShiftChainWitnessError> {
    let proposition = semantic_axioms
        .get(index)
        .ok_or(IntegerShiftChainWitnessError::UnknownSemanticAxiom(index))?;
    context
        .validate(proposition)
        .map_err(IntegerShiftChainWitnessError::MalformedProposition)?;
    Ok(proposition)
}

fn fixed_native(integer_type: IntegerType) -> bool {
    !integer_type.is_address() && matches!(integer_type.bits(), 8 | 16 | 32 | 64)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerShiftChainWitnessError {
    RootNotValue,
    RootNotInteger,
    UnsupportedValueCarrier(IntegerType),
    EmptyShiftChain,
    NonCanonicalDefinitionOrder,
    UnknownSemanticAxiom(usize),
    MalformedProposition(psi_core::PropositionError),
    DefinitionNotEquality(usize),
    DefinitionTargetNotValue(usize),
    DefinitionNotExactShift(usize),
    ShiftChainMismatch(usize),
    UnsupportedCountCarrier {
        index: usize,
        count_type: IntegerType,
    },
    MissingCountAxiom(usize),
    UnexpectedCountAxiom(usize),
    CountAxiomNotPrior {
        definition_axiom: usize,
        count_axiom: usize,
    },
    CountAxiomNotEquality(usize),
    CountAxiomMismatch(usize),
    CountTypeMismatch(usize),
    NegativeCount(usize),
    CountOutsideValueWidth {
        index: usize,
        count: u128,
        width: u16,
    },
    CyclicValue(usize),
    TargetMismatch,
}

impl std::fmt::Display for IntegerShiftChainWitnessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for IntegerShiftChainWitnessError {}

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

    fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
        let value = match integer_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(value),
            IntegerSign::Unsigned => IntegerValue::Unsigned(value.try_into().expect("unsigned")),
        };
        ScalarTerm::integer(integer_type, value).expect("integer term")
    }

    fn shift_definition(
        target: ScalarTerm,
        direction: IntegerShiftDirection,
        value_type: IntegerType,
        count_type: IntegerType,
        operand: ScalarTerm,
        count: ScalarTerm,
    ) -> Proposition {
        let definition = match direction {
            IntegerShiftDirection::Left => {
                ScalarTerm::exact_integer_shift_left(value_type, count_type, operand, count)
            }
            IntegerShiftDirection::Right => {
                ScalarTerm::exact_integer_shift_right(value_type, count_type, operand, count)
            }
        }
        .expect("exact shift");
        Proposition::Equal(target, definition)
    }

    #[test]
    fn normalizes_mixed_directions_and_heterogeneous_independently_landed_counts() {
        let i32_type = integer_type(IntegerSign::Signed, 32);
        let u8_type = integer_type(IntegerSign::Unsigned, 8);
        let u16_type = integer_type(IntegerSign::Unsigned, 16);
        let i8_type = integer_type(IntegerSign::Signed, 8);
        let root = value(1, i32_type);
        let left = value(2, i32_type);
        let right_count = value(3, u16_type);
        let right = value(4, i32_type);
        let final_count = value(5, i8_type);
        let target = value(6, i32_type);
        let axioms = vec![
            shift_definition(
                left.clone(),
                IntegerShiftDirection::Left,
                i32_type,
                u8_type,
                root.clone(),
                integer(u8_type, 2),
            ),
            Proposition::Equal(right_count.clone(), integer(u16_type, 1)),
            shift_definition(
                right.clone(),
                IntegerShiftDirection::Right,
                i32_type,
                u16_type,
                left,
                right_count,
            ),
            Proposition::Equal(final_count.clone(), integer(i8_type, 3)),
            shift_definition(
                target.clone(),
                IntegerShiftDirection::Left,
                i32_type,
                i8_type,
                right,
                final_count,
            ),
        ];
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(u16_type)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(5).unwrap(), ScalarType::Integer(i8_type)),
            (ValueId::new(6).unwrap(), ScalarType::Integer(i32_type)),
        ])
        .unwrap();
        let checked = check_integer_shift_chain_witness(
            &context,
            &axioms,
            &IntegerShiftChainWitness {
                root: root.clone(),
                target: target.clone(),
                steps: vec![
                    IntegerShiftStepWitness {
                        definition_axiom: 0,
                        count_axiom: None,
                    },
                    IntegerShiftStepWitness {
                        definition_axiom: 2,
                        count_axiom: Some(1),
                    },
                    IntegerShiftStepWitness {
                        definition_axiom: 4,
                        count_axiom: Some(3),
                    },
                ],
            },
        )
        .expect("complete mixed shift word");
        assert_eq!(checked.root(), &root);
        assert_eq!(checked.target(), &target);
        assert_eq!(checked.value_type(), i32_type);
        assert_eq!(
            checked
                .steps()
                .iter()
                .map(|step| (step.direction(), step.count_type(), step.count()))
                .collect::<Vec<_>>(),
            vec![
                (IntegerShiftDirection::Left, u8_type, 2),
                (IntegerShiftDirection::Right, u16_type, 1),
                (IntegerShiftDirection::Left, i8_type, 3),
            ],
        );
        assert_eq!(checked.steps()[1].definition_axiom(), 2);
        assert_eq!(checked.steps()[1].count_axiom(), Some(1));
    }

    #[test]
    fn rejects_missing_late_reversed_negative_and_out_of_range_count_facts() {
        let i32_type = integer_type(IntegerSign::Signed, 32);
        let i8_type = integer_type(IntegerSign::Signed, 8);
        let root = value(1, i32_type);
        let count = value(2, i8_type);
        let target = value(3, i32_type);
        let definition = shift_definition(
            target.clone(),
            IntegerShiftDirection::Left,
            i32_type,
            i8_type,
            root.clone(),
            count.clone(),
        );
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
        ])
        .unwrap();
        let witness = |definition_axiom, count_axiom| IntegerShiftChainWitness {
            root: root.clone(),
            target: target.clone(),
            steps: vec![IntegerShiftStepWitness {
                definition_axiom,
                count_axiom,
            }],
        };
        assert_eq!(
            check_integer_shift_chain_witness(&context, &[definition.clone()], &witness(0, None),),
            Err(IntegerShiftChainWitnessError::MissingCountAxiom(0)),
        );
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &[
                    definition.clone(),
                    Proposition::Equal(count.clone(), integer(i8_type, 1))
                ],
                &witness(0, Some(1)),
            ),
            Err(IntegerShiftChainWitnessError::CountAxiomNotPrior {
                definition_axiom: 0,
                count_axiom: 1,
            }),
        );
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &[
                    Proposition::Equal(integer(i8_type, 1), count.clone()),
                    definition.clone(),
                ],
                &witness(1, Some(0)),
            ),
            Err(IntegerShiftChainWitnessError::CountAxiomMismatch(0)),
        );
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &[
                    Proposition::Equal(count.clone(), integer(i8_type, -1)),
                    definition.clone(),
                ],
                &witness(1, Some(0)),
            ),
            Err(IntegerShiftChainWitnessError::NegativeCount(1)),
        );
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &[Proposition::Equal(count, integer(i8_type, 32)), definition,],
                &witness(1, Some(0)),
            ),
            Err(IntegerShiftChainWitnessError::CountOutsideValueWidth {
                index: 1,
                count: 32,
                width: 32,
            }),
        );
    }

    #[test]
    fn rejects_reordered_discontinuous_nonexact_and_target_drifted_words() {
        let i32_type = integer_type(IntegerSign::Signed, 32);
        let u8_type = integer_type(IntegerSign::Unsigned, 8);
        let root = value(1, i32_type);
        let middle = value(2, i32_type);
        let target = value(3, i32_type);
        let axioms = vec![
            shift_definition(
                middle.clone(),
                IntegerShiftDirection::Left,
                i32_type,
                u8_type,
                root.clone(),
                integer(u8_type, 1),
            ),
            shift_definition(
                target.clone(),
                IntegerShiftDirection::Right,
                i32_type,
                u8_type,
                middle,
                integer(u8_type, 1),
            ),
        ];
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(i32_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
        ])
        .unwrap();
        let witness = |target, steps| IntegerShiftChainWitness {
            root: root.clone(),
            target,
            steps,
        };
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &axioms,
                &witness(
                    target.clone(),
                    vec![
                        IntegerShiftStepWitness {
                            definition_axiom: 1,
                            count_axiom: None,
                        },
                        IntegerShiftStepWitness {
                            definition_axiom: 0,
                            count_axiom: None,
                        },
                    ],
                ),
            ),
            Err(IntegerShiftChainWitnessError::NonCanonicalDefinitionOrder),
        );
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &axioms,
                &witness(
                    target.clone(),
                    vec![IntegerShiftStepWitness {
                        definition_axiom: 1,
                        count_axiom: None,
                    }],
                ),
            ),
            Err(IntegerShiftChainWitnessError::ShiftChainMismatch(1)),
        );
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &[Proposition::Equal(target.clone(), root.clone())],
                &witness(
                    target.clone(),
                    vec![IntegerShiftStepWitness {
                        definition_axiom: 0,
                        count_axiom: None,
                    }],
                ),
            ),
            Err(IntegerShiftChainWitnessError::DefinitionNotExactShift(0)),
        );
        assert_eq!(
            check_integer_shift_chain_witness(
                &context,
                &axioms,
                &witness(
                    target,
                    vec![IntegerShiftStepWitness {
                        definition_axiom: 0,
                        count_axiom: None,
                    }],
                ),
            ),
            Err(IntegerShiftChainWitnessError::TargetMismatch),
        );
    }
}
