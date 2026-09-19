//! Scalar terms: scalar and float types, structural path segments, the
//! structural fields a term can name, and the term forms themselves.

use crate::proposition::propositions::{
    validate_integer_operands, validate_integer_shift_operands,
};
use crate::proposition::{IntegerType, IntegerValue, PropositionError};
use crate::{PlaceId, StructuralCaseId, StructuralFieldId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarType {
    Boolean,
    Integer(IntegerType),
    IeeeFloat(IeeeFloatFormat),
}

/// One canonical step below a terminal structural root in a scalar
/// proposition. Field steps use verifier-owned structural-field identities;
/// fixed indices retain the exact literal array element; case steps enter one
/// exact sum payload namespace before a following field step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalStructuralPathSegment {
    Field(StructuralFieldId),
    FixedIndex(u64),
    Case(StructuralCaseId),
}

/// Exact IEEE interchange format retained by target-neutral scalar execution
/// and structural predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatFormat {
    Binary32,
    Binary64,
}

/// One exact runtime IEEE interchange value. Raw bits are semantic payload:
/// signed zero and NaN representation are not reconstructed through a host
/// floating-point conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatValue {
    Binary32(u32),
    Binary64(u64),
}

impl IeeeFloatValue {
    pub const fn format(self) -> IeeeFloatFormat {
        match self {
            Self::Binary32(_) => IeeeFloatFormat::Binary32,
            Self::Binary64(_) => IeeeFloatFormat::Binary64,
        }
    }
}

/// Source IEEE comparison retained without mathematical-equality laws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatComparisonKind {
    Equal,
    NotEqual,
}

/// One nonempty canonical path to a relevant IEEE floating-point field below
/// a Terminal structural root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IeeeFloatStructuralField {
    pub(crate) root: PlaceId,
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
}

impl IeeeFloatStructuralField {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Result<Self, PropositionError> {
        if path.is_empty() {
            return Err(PropositionError::EmptyIeeeFloatStructuralFieldPath);
        }
        Ok(Self { root, path })
    }

    pub const fn root(&self) -> PlaceId {
        self.root
    }

    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }

    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self { root, path }
    }
}

/// One nonempty canonical path to a byte-sequence field below a Terminal
/// structural root. Equality observes only the live length and byte prefix;
/// native descriptor identity and unused bounded capacity are not semantic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteSequenceStructuralField {
    pub(crate) root: PlaceId,
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
}

/// One structural subject whose active sum case can be observed.
/// The path may be empty when the subject is the structural root itself.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralCaseSubject {
    pub(crate) root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
}

impl StructuralCaseSubject {
    pub fn new(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Self {
        Self { root, path }
    }

    pub const fn root(&self) -> PlaceId {
        self.root
    }

    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }

    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self { root, path }
    }
}

impl ByteSequenceStructuralField {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Result<Self, PropositionError> {
        if path.is_empty() {
            return Err(PropositionError::EmptyByteSequenceStructuralFieldPath);
        }
        Ok(Self { root, path })
    }

    pub const fn root(&self) -> PlaceId {
        self.root
    }

    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }

    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self { root, path }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarTerm {
    Value {
        id: ValueId,
        scalar_type: ScalarType,
    },
    /// One nonempty canonical structural path to a relevant Boolean field
    /// below a terminal structural root. The terminal verifier traverses every
    /// field and fixed index against the exact declared structural types and
    /// independently confirms that the final field is Boolean.
    BooleanField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
    /// One nonempty canonical structural path to a relevant fixed-integer
    /// field. The repeated integer type is independently checked against the
    /// terminal structural declaration at the leaf.
    IntegerField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    },
    Boolean(bool),
    BooleanNot {
        operand: Box<ScalarTerm>,
    },
    BooleanEqual {
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerEqual {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerLessThan {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerLessOrEqual {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseAnd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseNot {
        scalar_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerWiden {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerExactCast {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerBitwiseOr {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseXor {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerShiftLeft {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    WrappingIntegerShiftRight {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerShiftLeft {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerShiftRight {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
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
    WrappingIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
}

impl ScalarTerm {
    pub fn value(id: ValueId, scalar_type: ScalarType) -> Self {
        Self::Value { id, scalar_type }
    }

    pub fn boolean_field(root: PlaceId, field: StructuralFieldId) -> Self {
        Self::boolean_field_path(root, vec![CanonicalStructuralPathSegment::Field(field)])
    }

    pub fn boolean_field_path(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Self {
        Self::BooleanField { root, path }
    }

    pub fn integer_field_path(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    ) -> Self {
        Self::IntegerField {
            root,
            path,
            scalar_type,
        }
    }

    pub const fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    pub fn boolean_not(operand: ScalarTerm) -> Result<Self, PropositionError> {
        if operand.scalar_type() != ScalarType::Boolean {
            return Err(PropositionError::BooleanNotTypeMismatch(
                operand.scalar_type(),
            ));
        }
        Ok(Self::BooleanNot {
            operand: Box::new(operand),
        })
    }

    pub fn boolean_equal(left: ScalarTerm, right: ScalarTerm) -> Result<Self, PropositionError> {
        if left.scalar_type() != ScalarType::Boolean || right.scalar_type() != ScalarType::Boolean {
            return Err(PropositionError::BooleanEqualTypeMismatch {
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::BooleanEqual {
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_equal(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::IntegerEqualTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::IntegerEqual {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_less_than(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerLessThan {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_less_or_equal(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerLessOrEqual {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_bitwise_and(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerBitwiseAnd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_bitwise_not(
        scalar_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if operand.scalar_type() != expected {
            return Err(PropositionError::IntegerBitwiseNotTypeMismatch {
                expected,
                operand: operand.scalar_type(),
            });
        }
        Ok(Self::IntegerBitwiseNot {
            scalar_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_widen(
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let actual = operand.scalar_type();
        let expected = ScalarType::Integer(source_type);
        if actual != expected || !source_type.can_widen_to(target_type) {
            return Err(PropositionError::IntegerWidenTypeMismatch {
                source: expected,
                target: ScalarType::Integer(target_type),
                operand: actual,
            });
        }
        Ok(Self::IntegerWiden {
            source_type,
            target_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_exact_cast(
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let actual = operand.scalar_type();
        let expected = ScalarType::Integer(source_type);
        if actual != expected || !source_type.can_exact_cast_to(target_type) {
            return Err(PropositionError::IntegerExactCastTypeMismatch {
                source: expected,
                target: ScalarType::Integer(target_type),
                operand: actual,
            });
        }
        Ok(Self::IntegerExactCast {
            source_type,
            target_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_bitwise_or(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerBitwiseOr {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_bitwise_xor(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerBitwiseXor {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_shift_left(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn wrapping_integer_shift_right(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn exact_integer_shift_right(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::ExactIntegerShiftRight {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn exact_integer_shift_left(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn exact_integer_add(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_multiply(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerMultiply {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_divide(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerDivide {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_remainder(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerRemainder {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_divide(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::WrappingIntegerDivide {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_remainder(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::WrappingIntegerRemainder {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_divide(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::SaturatingIntegerDivide {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_remainder(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::SaturatingIntegerRemainder {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
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

    pub fn wrapping_integer_multiply(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::WrappingIntegerMultiplyTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::WrappingIntegerMultiply {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_multiply(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::SaturatingIntegerMultiplyTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::SaturatingIntegerMultiply {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Value { scalar_type, .. } => *scalar_type,
            Self::IntegerField { scalar_type, .. } => ScalarType::Integer(*scalar_type),
            Self::Boolean(_)
            | Self::BooleanField { .. }
            | Self::BooleanNot { .. }
            | Self::BooleanEqual { .. }
            | Self::IntegerEqual { .. }
            | Self::IntegerLessThan { .. }
            | Self::IntegerLessOrEqual { .. } => ScalarType::Boolean,
            Self::Integer { scalar_type, .. }
            | Self::IntegerBitwiseNot { scalar_type, .. }
            | Self::IntegerBitwiseAnd { scalar_type, .. }
            | Self::IntegerBitwiseOr { scalar_type, .. }
            | Self::IntegerBitwiseXor { scalar_type, .. }
            | Self::ExactIntegerAdd { scalar_type, .. }
            | Self::ExactIntegerSubtract { scalar_type, .. }
            | Self::ExactIntegerMultiply { scalar_type, .. }
            | Self::ExactIntegerDivide { scalar_type, .. }
            | Self::ExactIntegerRemainder { scalar_type, .. }
            | Self::WrappingIntegerDivide { scalar_type, .. }
            | Self::WrappingIntegerRemainder { scalar_type, .. }
            | Self::SaturatingIntegerDivide { scalar_type, .. }
            | Self::SaturatingIntegerRemainder { scalar_type, .. }
            | Self::WrappingIntegerAdd { scalar_type, .. }
            | Self::SaturatingIntegerAdd { scalar_type, .. }
            | Self::WrappingIntegerSubtract { scalar_type, .. }
            | Self::SaturatingIntegerSubtract { scalar_type, .. }
            | Self::WrappingIntegerMultiply { scalar_type, .. }
            | Self::SaturatingIntegerMultiply { scalar_type, .. } => {
                ScalarType::Integer(*scalar_type)
            }
            Self::IntegerWiden { target_type, .. } | Self::IntegerExactCast { target_type, .. } => {
                ScalarType::Integer(*target_type)
            }
            Self::WrappingIntegerShiftLeft { value_type, .. }
            | Self::WrappingIntegerShiftRight { value_type, .. }
            | Self::ExactIntegerShiftLeft { value_type, .. }
            | Self::ExactIntegerShiftRight { value_type, .. } => ScalarType::Integer(*value_type),
        }
    }

    pub fn integer_value(&self) -> Option<(IntegerType, IntegerValue)> {
        match self {
            Self::Integer { scalar_type, value } => Some((*scalar_type, *value)),
            Self::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => {
                let (operand_type, operand) = operand.integer_value()?;
                if operand_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.bitwise_not(operand)?))
            }
            Self::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => {
                let (operand_type, operand) = operand.integer_value()?;
                if operand_type != *source_type || !source_type.can_widen_to(*target_type) {
                    return None;
                }
                Some((
                    *target_type,
                    source_type.widen_value_to(*target_type, operand)?,
                ))
            }
            Self::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => {
                let (operand_type, operand) = operand.integer_value()?;
                if operand_type != *source_type || !source_type.can_exact_cast_to(*target_type) {
                    return None;
                }
                Some((
                    *target_type,
                    source_type.exact_cast_value_to(*target_type, operand)?,
                ))
            }
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
            Self::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_mul(left, right)?))
            }
            Self::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_mul(left, right)?))
            }
            Self::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                let value = match self {
                    Self::IntegerBitwiseAnd { .. } => scalar_type.bitwise_and(left, right)?,
                    Self::IntegerBitwiseOr { .. } => scalar_type.bitwise_or(left, right)?,
                    Self::IntegerBitwiseXor { .. } => scalar_type.bitwise_xor(left, right)?,
                    _ => unreachable!(),
                };
                Some((*scalar_type, value))
            }
            Self::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => {
                let (actual_value_type, value) = value.integer_value()?;
                let (actual_count_type, count) = count.integer_value()?;
                if actual_value_type != *value_type || actual_count_type != *count_type {
                    return None;
                }
                let result = match self {
                    Self::WrappingIntegerShiftLeft { .. } => {
                        value_type.wrapping_shift_left(value, *count_type, count)?
                    }
                    Self::WrappingIntegerShiftRight { .. } => {
                        value_type.wrapping_shift_right(value, *count_type, count)?
                    }
                    Self::ExactIntegerShiftLeft { .. } => {
                        value_type.exact_shift_left(value, *count_type, count)?
                    }
                    Self::ExactIntegerShiftRight { .. } => {
                        value_type.exact_shift_right(value, *count_type, count)?
                    }
                    _ => unreachable!(),
                };
                Some((*value_type, result))
            }
            Self::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_add(left, right)?))
            }
            Self::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_sub(left, right)?))
            }
            Self::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_mul(left, right)?))
            }
            Self::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_div(left, right)?))
            }
            Self::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_rem(left, right)?))
            }
            Self::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_div(left, right)?))
            }
            Self::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_rem(left, right)?))
            }
            Self::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_div(left, right)?))
            }
            Self::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_rem(left, right)?))
            }
            _ => None,
        }
    }

    pub fn boolean_value(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            Self::BooleanNot { operand } => Some(!operand.boolean_value()?),
            Self::BooleanEqual { left, right } => {
                Some(left.boolean_value()? == right.boolean_value()?)
            }
            Self::IntegerEqual {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                (left_type == *scalar_type && right_type == *scalar_type).then_some(left == right)
            }
            Self::IntegerLessThan {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some(scalar_type.compare(left, right)?.is_lt())
            }
            Self::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some(!scalar_type.compare(left, right)?.is_gt())
            }
            _ => None,
        }
    }

    /// Visits every `ValueId` carried by `Value` leaves. Returning `false`
    /// from `visit` short-circuits the traversal and propagates `false`.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId) -> bool) -> bool {
        let mut operands = vec![self];
        while let Some(term) = operands.pop() {
            match term {
                Self::Value { id, .. } => {
                    if !visit(*id) {
                        return false;
                    }
                }
                Self::BooleanNot { operand }
                | Self::IntegerBitwiseNot { operand, .. }
                | Self::IntegerWiden { operand, .. }
                | Self::IntegerExactCast { operand, .. } => operands.push(operand),
                Self::BooleanEqual { left, right, .. }
                | Self::IntegerEqual { left, right, .. }
                | Self::IntegerLessThan { left, right, .. }
                | Self::IntegerLessOrEqual { left, right, .. }
                | Self::IntegerBitwiseAnd { left, right, .. }
                | Self::IntegerBitwiseOr { left, right, .. }
                | Self::IntegerBitwiseXor { left, right, .. }
                | Self::WrappingIntegerShiftLeft {
                    value: left,
                    count: right,
                    ..
                }
                | Self::WrappingIntegerShiftRight {
                    value: left,
                    count: right,
                    ..
                }
                | Self::ExactIntegerShiftLeft {
                    value: left,
                    count: right,
                    ..
                }
                | Self::ExactIntegerShiftRight {
                    value: left,
                    count: right,
                    ..
                }
                | Self::ExactIntegerAdd { left, right, .. }
                | Self::ExactIntegerSubtract { left, right, .. }
                | Self::ExactIntegerMultiply { left, right, .. }
                | Self::ExactIntegerDivide { left, right, .. }
                | Self::ExactIntegerRemainder { left, right, .. }
                | Self::WrappingIntegerDivide { left, right, .. }
                | Self::WrappingIntegerRemainder { left, right, .. }
                | Self::SaturatingIntegerDivide { left, right, .. }
                | Self::SaturatingIntegerRemainder { left, right, .. }
                | Self::WrappingIntegerAdd { left, right, .. }
                | Self::SaturatingIntegerAdd { left, right, .. }
                | Self::WrappingIntegerSubtract { left, right, .. }
                | Self::SaturatingIntegerSubtract { left, right, .. }
                | Self::WrappingIntegerMultiply { left, right, .. }
                | Self::SaturatingIntegerMultiply { left, right, .. } => {
                    operands.push(left);
                    operands.push(right);
                }
                Self::BooleanField { .. }
                | Self::IntegerField { .. }
                | Self::Boolean(_)
                | Self::Integer { .. } => {}
            }
        }
        true
    }

    pub fn any_value_id(&self, mut predicate: impl FnMut(ValueId) -> bool) -> bool {
        !self.visit_value_ids(|id| !predicate(id))
    }

    pub fn validate(&self) -> Result<(), PropositionError> {
        match self {
            Self::Value { .. }
            | Self::BooleanField { .. }
            | Self::IntegerField { .. }
            | Self::Boolean(_) => Ok(()),
            Self::BooleanNot { operand } => {
                operand.validate()?;
                if operand.scalar_type() != ScalarType::Boolean {
                    return Err(PropositionError::BooleanNotTypeMismatch(
                        operand.scalar_type(),
                    ));
                }
                Ok(())
            }
            Self::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => {
                operand.validate()?;
                let expected = ScalarType::Integer(*source_type);
                if operand.scalar_type() != expected || !source_type.can_exact_cast_to(*target_type)
                {
                    return Err(PropositionError::IntegerExactCastTypeMismatch {
                        source: expected,
                        target: ScalarType::Integer(*target_type),
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::BooleanEqual { left, right } => {
                left.validate()?;
                right.validate()?;
                if left.scalar_type() != ScalarType::Boolean
                    || right.scalar_type() != ScalarType::Boolean
                {
                    return Err(PropositionError::BooleanEqualTypeMismatch {
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerEqual {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::IntegerEqualTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerLessThan {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => {
                operand.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if operand.scalar_type() != expected {
                    return Err(PropositionError::IntegerBitwiseNotTypeMismatch {
                        expected,
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => {
                operand.validate()?;
                let expected = ScalarType::Integer(*source_type);
                if operand.scalar_type() != expected || !source_type.can_widen_to(*target_type) {
                    return Err(PropositionError::IntegerWidenTypeMismatch {
                        source: expected,
                        target: ScalarType::Integer(*target_type),
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => {
                value.validate()?;
                count.validate()?;
                validate_integer_shift_operands(*value_type, *count_type, value, count)
            }
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
            Self::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
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
            Self::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::WrappingIntegerMultiplyTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::SaturatingIntegerMultiplyTypeMismatch {
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
