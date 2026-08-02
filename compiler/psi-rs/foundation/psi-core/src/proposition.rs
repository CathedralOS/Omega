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

    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Value { scalar_type, .. } => *scalar_type,
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }

    pub const fn integer_value(&self) -> Option<(IntegerType, IntegerValue)> {
        match self {
            Self::Integer { scalar_type, value } => Some((*scalar_type, *value)),
            _ => None,
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
        let ScalarTerm::Value { id, scalar_type } = term else {
            return Ok(());
        };
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
        Ok(())
    }
}

fn require_same_type(left: &ScalarTerm, right: &ScalarTerm) -> Result<(), PropositionError> {
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
}
