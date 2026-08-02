use std::collections::BTreeMap;

use crate::{PropositionId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerSign {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegerType {
    sign: IntegerSign,
    bits: u16,
}

impl IntegerType {
    pub fn new(sign: IntegerSign, bits: u16) -> Result<Self, PropositionError> {
        if !(1..=128).contains(&bits) {
            return Err(PropositionError::InvalidIntegerWidth(bits));
        }
        Ok(Self { sign, bits })
    }

    pub const fn sign(self) -> IntegerSign {
        self.sign
    }

    pub const fn bits(self) -> u16 {
        self.bits
    }

    pub fn admits(self, value: IntegerValue) -> bool {
        match (self.sign, value) {
            (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                self.bits == 128 || {
                    let limit = 1_i128 << (self.bits - 1);
                    (-limit..limit).contains(&value)
                }
            }
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                self.bits == 128 || value < (1_u128 << self.bits)
            }
            _ => false,
        }
    }

    /// Add two admitted values modulo this exact integer width.
    ///
    /// Signed results use two's-complement interpretation of the reduced bit
    /// pattern. A sign/value mismatch or out-of-range input is rejected rather
    /// than silently reinterpreted.
    pub fn wrapping_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let mask = if self.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << self.bits) - 1
        };
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.wrapping_add(right) & mask)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let bits = (left as u128).wrapping_add(right as u128) & mask;
                let value = if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                    bits as i128
                } else {
                    (bits | !mask) as i128
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Subtract two admitted values modulo this exact integer width.
    ///
    /// Signed results use two's-complement interpretation of the reduced bit
    /// pattern. A sign/value mismatch or out-of-range input is rejected rather
    /// than silently reinterpreted.
    pub fn wrapping_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let mask = if self.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << self.bits) - 1
        };
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.wrapping_sub(right) & mask)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let bits = (left as u128).wrapping_sub(right as u128) & mask;
                let value = if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                    bits as i128
                } else {
                    (bits | !mask) as i128
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Add two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => {
                let maximum = if self.bits == 128 {
                    u128::MAX
                } else {
                    (1_u128 << self.bits) - 1
                };
                Some(IntegerValue::Unsigned(
                    left.saturating_add(right).min(maximum),
                ))
            }
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let value = if self.bits == 128 {
                    left.saturating_add(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    (left + right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Subtract two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.saturating_sub(right))),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let value = if self.bits == 128 {
                    left.saturating_sub(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    (left - right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerValue {
    Signed(i128),
    Unsigned(u128),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarType {
    Boolean,
    Integer(IntegerType),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarTerm {
    Value {
        id: ValueId,
        scalar_type: ScalarType,
    },
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    WrappingIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
}

impl ScalarTerm {
    pub fn value(id: ValueId, scalar_type: ScalarType) -> Self {
        Self::Value { id, scalar_type }
    }

    pub const fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    pub fn integer(
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Result<Self, PropositionError> {
        if !scalar_type.admits(value) {
            return Err(PropositionError::IntegerLiteralOutsideType { scalar_type, value });
        }
        Ok(Self::Integer { scalar_type, value })
    }

    pub fn wrapping_integer_add(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::WrappingIntegerAddTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::WrappingIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_add(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::SaturatingIntegerAddTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::SaturatingIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::WrappingIntegerSubtractTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::WrappingIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::SaturatingIntegerSubtractTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::SaturatingIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Value { scalar_type, .. } => *scalar_type,
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. }
            | Self::WrappingIntegerAdd { scalar_type, .. }
            | Self::SaturatingIntegerAdd { scalar_type, .. }
            | Self::WrappingIntegerSubtract { scalar_type, .. }
            | Self::SaturatingIntegerSubtract { scalar_type, .. } => {
                ScalarType::Integer(*scalar_type)
            }
        }
    }

    pub fn integer_value(&self) -> Option<(IntegerType, IntegerValue)> {
        match self {
            Self::Integer { scalar_type, value } => Some((*scalar_type, *value)),
            Self::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_add(left, right)?))
            }
            Self::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_add(left, right)?))
            }
            Self::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_sub(left, right)?))
            }
            Self::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_sub(left, right)?))
            }
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), PropositionError> {
        match self {
            Self::Value { .. } | Self::Boolean(_) => Ok(()),
            Self::Integer { scalar_type, value } => {
                if scalar_type.admits(*value) {
                    Ok(())
                } else {
                    Err(PropositionError::IntegerLiteralOutsideType {
                        scalar_type: *scalar_type,
                        value: *value,
                    })
                }
            }
            Self::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::WrappingIntegerAddTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::SaturatingIntegerAddTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::WrappingIntegerSubtractTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::SaturatingIntegerSubtractTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
        }
    }
}

/// The initial terminal-Psi proposition vocabulary.
///
/// This is intentionally small but not source-shaped. It refers only to
/// stable terminal values or closed literals. Operation-specific predicates
/// will extend this vocabulary together with their execution semantics and
/// obligation schemas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Proposition {
    Truth,
    Falsehood,
    Atom(PropositionId),
    Equal(ScalarTerm, ScalarTerm),
    LessThan(ScalarTerm, ScalarTerm),
    LessOrEqual(ScalarTerm, ScalarTerm),
    Conjunction(Vec<Proposition>),
    Implication {
        premise: Box<Proposition>,
        conclusion: Box<Proposition>,
    },
}

impl Proposition {
    pub fn validate(&self) -> Result<(), PropositionError> {
        match self {
            Self::Truth | Self::Falsehood => Ok(()),
            Self::Atom(_) => Ok(()),
            Self::Equal(left, right) => require_same_type(left, right),
            Self::LessThan(left, right) | Self::LessOrEqual(left, right) => {
                require_same_integer_type(left, right)
            }
            Self::Conjunction(conjuncts) => {
                if conjuncts.len() < 2 {
                    return Err(PropositionError::NonCanonicalConjunctionArity(
                        conjuncts.len(),
                    ));
                }
                for conjunct in conjuncts {
                    conjunct.validate()?;
                }
                Ok(())
            }
            Self::Implication {
                premise,
                conclusion,
            } => {
                premise.validate()?;
                conclusion.validate()
            }
        }
    }
}

/// Terminal-module typing context used by the proof kernel.
///
/// Value terms repeat their scalar type so propositions remain locally
/// inspectable, but the kernel checks that annotation against this unique
/// module-owned table. A proof cannot reinterpret one stable value identity at
/// another type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PropositionContext {
    value_types: BTreeMap<ValueId, ScalarType>,
}

impl PropositionContext {
    pub fn from_value_types(
        value_types: impl IntoIterator<Item = (ValueId, ScalarType)>,
    ) -> Result<Self, PropositionError> {
        let mut context = Self::default();
        for (id, scalar_type) in value_types {
            if let Some(previous) = context.value_types.insert(id, scalar_type)
                && previous != scalar_type
            {
                return Err(PropositionError::ConflictingValueType {
                    id,
                    first: previous,
                    second: scalar_type,
                });
            }
        }
        Ok(context)
    }

    pub fn validate(&self, proposition: &Proposition) -> Result<(), PropositionError> {
        proposition.validate()?;
        self.validate_value_terms(proposition)
    }

    fn validate_value_terms(&self, proposition: &Proposition) -> Result<(), PropositionError> {
        match proposition {
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                self.validate_term(left)?;
                self.validate_term(right)
            }
            Proposition::Conjunction(conjuncts) => {
                for conjunct in conjuncts {
                    self.validate_value_terms(conjunct)?;
                }
                Ok(())
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                self.validate_value_terms(premise)?;
                self.validate_value_terms(conclusion)
            }
        }
    }

    fn validate_term(&self, term: &ScalarTerm) -> Result<(), PropositionError> {
        match term {
            ScalarTerm::Value { id, scalar_type } => {
                let Some(expected) = self.value_types.get(id) else {
                    return Err(PropositionError::UnknownValue(*id));
                };
                if expected != scalar_type {
                    return Err(PropositionError::ValueTypeMismatch {
                        id: *id,
                        expected: *expected,
                        actual: *scalar_type,
                    });
                }
            }
            ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => {
                self.validate_term(left)?;
                self.validate_term(right)?;
            }
            ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
        Ok(())
    }
}

fn require_same_type(left: &ScalarTerm, right: &ScalarTerm) -> Result<(), PropositionError> {
    left.validate()?;
    right.validate()?;
    if left.scalar_type() != right.scalar_type() {
        return Err(PropositionError::MismatchedScalarTypes {
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn require_same_integer_type(
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    require_same_type(left, right)?;
    if !matches!(left.scalar_type(), ScalarType::Integer(_)) {
        return Err(PropositionError::OrderedComparisonRequiresIntegers(
            left.scalar_type(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropositionError {
    InvalidIntegerWidth(u16),
    IntegerLiteralOutsideType {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    MismatchedScalarTypes {
        left: ScalarType,
        right: ScalarType,
    },
    WrappingIntegerAddTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    SaturatingIntegerAddTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    WrappingIntegerSubtractTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    SaturatingIntegerSubtractTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    OrderedComparisonRequiresIntegers(ScalarType),
    NonCanonicalConjunctionArity(usize),
    UnknownValue(ValueId),
    ConflictingValueType {
        id: ValueId,
        first: ScalarType,
        second: ScalarType,
    },
    ValueTypeMismatch {
        id: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
}

impl std::fmt::Display for PropositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PropositionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_literals_are_checked_against_their_terminal_type() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
        assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).is_ok());
        assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(128)).is_err());

        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
        assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(255)).is_ok());
        assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(256)).is_err());
    }

    #[test]
    fn ordered_comparisons_require_one_exact_integer_type() {
        let boolean = ScalarTerm::boolean(false);
        assert_eq!(
            Proposition::LessThan(boolean.clone(), boolean)
                .validate()
                .expect_err("booleans are unordered"),
            PropositionError::OrderedComparisonRequiresIntegers(ScalarType::Boolean)
        );
    }

    #[test]
    fn proposition_context_rejects_value_type_reinterpretation() {
        let id = ValueId::new(7).expect("value identity");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 type");
        let context = PropositionContext::from_value_types([(id, ScalarType::Integer(i32_type))])
            .expect("value context");
        let proposition = Proposition::Equal(
            ScalarTerm::value(id, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        assert!(matches!(
            context.validate(&proposition),
            Err(PropositionError::ValueTypeMismatch { .. })
        ));
    }

    #[test]
    fn wrapping_add_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
            Some(IntegerValue::Unsigned(44))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(-116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(0))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn saturating_add_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
            Some(IntegerValue::Unsigned(255))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.saturating_add(IntegerValue::Signed(-120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(-128))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
        assert_eq!(
            i128_type.saturating_add(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn wrapping_subtract_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
            Some(IntegerValue::Unsigned(251))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(116))
        );
        assert_eq!(
            i8_type.wrapping_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(-116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }

    #[test]
    fn saturating_subtract_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
            Some(IntegerValue::Unsigned(0))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(-128))
        );
        assert_eq!(
            i8_type.saturating_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(127))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(0))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
        assert_eq!(
            i128_type.saturating_sub(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }
}
