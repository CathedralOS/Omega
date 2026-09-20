//! Propositions over scalar terms, the context that types them, and the
//! operand checks and errors that context enforces.

use crate::proposition::{
    ByteSequenceStructuralField, IeeeFloatComparisonKind, IeeeFloatFormat,
    IeeeFloatStructuralField, IntegerMathTerm, IntegerType, IntegerValue, ScalarTerm, ScalarType,
    StructuralCaseSubject,
};
use crate::{
    ContentConservation, ContentPlaceVersion, ContentTerm, PlaceId, PropositionId,
    StructuralCaseId, StructuralPlaceKind, ValueId,
};
use std::collections::BTreeMap;

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
    IntegerMathEqual(IntegerMathTerm, IntegerMathTerm),
    IntegerMathLessThan(IntegerMathTerm, IntegerMathTerm),
    IntegerMathLessOrEqual(IntegerMathTerm, IntegerMathTerm),
    /// IEEE `==` or `!=` over two exact structural leaves. This remains atomic
    /// rather than using mathematical equality: NaNs are non-reflexive while
    /// signed zeroes compare equal under `==` and unequal under `!=`.
    IeeeFloatComparison {
        kind: IeeeFloatComparisonKind,
        format: IeeeFloatFormat,
        left: IeeeFloatStructuralField,
        right: IeeeFloatStructuralField,
    },
    /// IEEE `==` or `!=` over two scalar terms of the same IEEE format. Like
    /// the structural-field form this stays atomic: NaN is non-reflexive and
    /// signed zeroes compare equal under `==` and unequal under `!=`, so the
    /// comparison is not mathematical equality.
    ScalarIeeeFloatComparison {
        kind: IeeeFloatComparisonKind,
        format: IeeeFloatFormat,
        left: ScalarTerm,
        right: ScalarTerm,
    },
    /// Content equality over two exact byte-sequence structural leaves.
    ByteSequenceEqual {
        left: ByteSequenceStructuralField,
        right: ByteSequenceStructuralField,
    },
    /// Exact membership in one declared case of a structural sum.
    StructuralCaseMembership {
        subject: StructuralCaseSubject,
        case: StructuralCaseId,
    },
    Conjunction(Vec<Proposition>),
    Disjunction(Vec<Proposition>),
    Implication {
        premise: Box<Proposition>,
        conclusion: Box<Proposition>,
    },
    /// Exact equality in one compiler-owned content algebra. Structural-place
    /// roots and projection identities are terminal semantic identities, not
    /// source handles.
    ContentConservation(ContentConservation),
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
            Self::IntegerMathEqual(left, right) => {
                left.validate()?;
                right.validate()?;
                if left > right {
                    return Err(PropositionError::NonCanonicalIntegerMathEqualityOperands);
                }
                Ok(())
            }
            Self::IntegerMathLessThan(left, right) | Self::IntegerMathLessOrEqual(left, right) => {
                left.validate()?;
                right.validate()
            }
            Self::IeeeFloatComparison { left, right, .. } => {
                if left.path.is_empty() || right.path.is_empty() {
                    return Err(PropositionError::EmptyIeeeFloatStructuralFieldPath);
                }
                if left > right {
                    return Err(PropositionError::NonCanonicalIeeeFloatComparisonOperands);
                }
                Ok(())
            }
            Self::ScalarIeeeFloatComparison {
                format,
                left,
                right,
                ..
            } => {
                require_ieee_float_operands(*format, left, right)?;
                if left > right {
                    return Err(PropositionError::NonCanonicalScalarIeeeFloatComparisonOperands);
                }
                Ok(())
            }
            Self::ByteSequenceEqual { left, right } => {
                if left.path.is_empty() || right.path.is_empty() {
                    return Err(PropositionError::EmptyByteSequenceStructuralFieldPath);
                }
                if left > right {
                    return Err(PropositionError::NonCanonicalByteSequenceEqualOperands);
                }
                Ok(())
            }
            Self::StructuralCaseMembership { .. } => Ok(()),
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
            Self::Disjunction(disjuncts) => {
                if disjuncts.len() < 2 {
                    return Err(PropositionError::NonCanonicalDisjunctionArity(
                        disjuncts.len(),
                    ));
                }
                for disjunct in disjuncts {
                    disjunct.validate()?;
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
            Self::ContentConservation(conservation) => conservation.validate(),
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
    structural_places: BTreeMap<PlaceId, StructuralPlaceKind>,
}

impl PropositionContext {
    pub fn from_value_types(
        value_types: impl IntoIterator<Item = (ValueId, ScalarType)>,
    ) -> Result<Self, PropositionError> {
        Self::from_value_types_and_places(value_types, [])
    }

    pub fn from_value_types_and_places(
        value_types: impl IntoIterator<Item = (ValueId, ScalarType)>,
        structural_places: impl IntoIterator<Item = (PlaceId, StructuralPlaceKind)>,
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
        for (id, kind) in structural_places {
            if let Some(previous) = context.structural_places.insert(id, kind)
                && previous != kind
            {
                return Err(PropositionError::ConflictingStructuralPlaceKind {
                    id,
                    first: previous,
                    second: kind,
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
            Proposition::IntegerMathEqual(left, right)
            | Proposition::IntegerMathLessThan(left, right)
            | Proposition::IntegerMathLessOrEqual(left, right) => {
                self.validate_integer_math_term(left)?;
                self.validate_integer_math_term(right)
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                for field in [left, right] {
                    if !self.structural_places.contains_key(&field.root) {
                        return Err(PropositionError::UnknownStructuralPlace(field.root));
                    }
                }
                Ok(())
            }
            Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                self.validate_term(left)?;
                self.validate_term(right)
            }
            Proposition::ByteSequenceEqual { left, right } => {
                for field in [left, right] {
                    if !self.structural_places.contains_key(&field.root) {
                        return Err(PropositionError::UnknownStructuralPlace(field.root));
                    }
                }
                Ok(())
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                if !self.structural_places.contains_key(&subject.root) {
                    return Err(PropositionError::UnknownStructuralPlace(subject.root));
                }
                Ok(())
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    self.validate_value_terms(proposition)?;
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
            Proposition::ContentConservation(conservation) => {
                self.validate_content_term(conservation.left())?;
                self.validate_content_term(conservation.right())
            }
        }
    }

    fn validate_integer_math_term(&self, term: &IntegerMathTerm) -> Result<(), PropositionError> {
        match term {
            IntegerMathTerm::MathValue { source_type, value } => {
                let Some(expected) = self.value_types.get(value) else {
                    return Err(PropositionError::UnknownValue(*value));
                };
                let actual = ScalarType::Integer(*source_type);
                if expected != &actual {
                    return Err(PropositionError::ValueTypeMismatch {
                        id: *value,
                        expected: *expected,
                        actual,
                    });
                }
            }
            IntegerMathTerm::IntegerLiteral(_) => {}
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right) => {
                self.validate_integer_math_term(left)?;
                self.validate_integer_math_term(right)?;
            }
            IntegerMathTerm::ShiftLeft { value, count } => {
                self.validate_integer_math_term(value)?;
                self.validate_integer_math_term(count)?;
            }
        }
        Ok(())
    }

    fn validate_content_term(&self, term: &ContentTerm) -> Result<(), PropositionError> {
        match term {
            ContentTerm::Projection { subject, .. } => {
                let Some(kind) = self.structural_places.get(&subject.root) else {
                    return Err(PropositionError::UnknownStructuralPlace(subject.root));
                };
                if subject.version == ContentPlaceVersion::Entry
                    && matches!(
                        kind,
                        StructuralPlaceKind::Result | StructuralPlaceKind::OperationResult { .. }
                    )
                {
                    return Err(PropositionError::EntryResultStructuralPlace(subject.root));
                }
                if matches!(
                    kind,
                    StructuralPlaceKind::TrivialAffineLocal { .. }
                        | StructuralPlaceKind::BlockParameter { .. }
                ) {
                    return Err(PropositionError::UnsupportedContentLocalStructuralPlace(
                        subject.root,
                    ));
                }
                Ok(())
            }
            ContentTerm::Separate(terms) => {
                for term in terms {
                    self.validate_content_term(term)?;
                }
                Ok(())
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
            ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
                if !self.structural_places.contains_key(root) {
                    return Err(PropositionError::UnknownStructuralPlace(*root));
                }
            }
            ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
            | ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
                self.validate_term(left)?;
                self.validate_term(right)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                self.validate_term(value)?;
                self.validate_term(count)?;
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => self.validate_term(operand)?,
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

pub(crate) fn validate_integer_operands(
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    let expected = ScalarType::Integer(integer_type);
    if left.scalar_type() != expected || right.scalar_type() != expected {
        return Err(PropositionError::IntegerOperandTypeMismatch {
            expected,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

pub(crate) fn validate_integer_shift_operands(
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
) -> Result<(), PropositionError> {
    let expected_value = ScalarType::Integer(value_type);
    let expected_count = ScalarType::Integer(count_type);
    if value.scalar_type() != expected_value || count.scalar_type() != expected_count {
        return Err(PropositionError::IntegerShiftOperandTypeMismatch {
            expected_value,
            actual_value: value.scalar_type(),
            expected_count,
            actual_count: count.scalar_type(),
        });
    }
    Ok(())
}

fn require_ieee_float_operands(
    format: IeeeFloatFormat,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    left.validate()?;
    right.validate()?;
    let expected = ScalarType::IeeeFloat(format);
    if left.scalar_type() != expected || right.scalar_type() != expected {
        return Err(PropositionError::ScalarIeeeFloatOperandTypeMismatch {
            expected,
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
    InvalidIntegerRange {
        integer_type: IntegerType,
        minimum: IntegerValue,
        maximum: IntegerValue,
    },
    NegativeZeroIntegerMathLiteral,
    AddressIntegerMathValue(IntegerType),
    NonCanonicalIntegerMathEqualityOperands,
    IntegerLiteralOutsideType {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    MismatchedScalarTypes {
        left: ScalarType,
        right: ScalarType,
    },
    BooleanNotTypeMismatch(ScalarType),
    BooleanEqualTypeMismatch {
        left: ScalarType,
        right: ScalarType,
    },
    IntegerEqualTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerBitwiseNotTypeMismatch {
        expected: ScalarType,
        operand: ScalarType,
    },
    IntegerWidenTypeMismatch {
        source: ScalarType,
        target: ScalarType,
        operand: ScalarType,
    },
    IntegerExactCastTypeMismatch {
        source: ScalarType,
        target: ScalarType,
        operand: ScalarType,
    },
    IntegerOperandTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerShiftOperandTypeMismatch {
        expected_value: ScalarType,
        actual_value: ScalarType,
        expected_count: ScalarType,
        actual_count: ScalarType,
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
    WrappingIntegerMultiplyTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    SaturatingIntegerMultiplyTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    OrderedComparisonRequiresIntegers(ScalarType),
    ScalarIeeeFloatOperandTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    NonCanonicalScalarIeeeFloatComparisonOperands,
    NonCanonicalConjunctionArity(usize),
    NonCanonicalDisjunctionArity(usize),
    EmptyIeeeFloatStructuralFieldPath,
    NonCanonicalIeeeFloatComparisonOperands,
    EmptyByteSequenceStructuralFieldPath,
    NonCanonicalByteSequenceEqualOperands,
    EmptyContentAlgebraParameter,
    EmptyContentCaseName,
    EmptyContentFieldName,
    ZeroContentProjectionFingerprint,
    NonCanonicalContentEquationOrder,
    NonCanonicalContentSeparationArity(usize),
    NonCanonicalContentSeparationOrder,
    NestedContentSeparation,
    ContentTermNestingTooDeep,
    UnknownValue(ValueId),
    UnknownStructuralPlace(PlaceId),
    EntryResultStructuralPlace(PlaceId),
    UnsupportedContentLocalStructuralPlace(PlaceId),
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
    ConflictingStructuralPlaceKind {
        id: PlaceId,
        first: StructuralPlaceKind,
        second: StructuralPlaceKind,
    },
}

impl std::fmt::Display for PropositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PropositionError {}
