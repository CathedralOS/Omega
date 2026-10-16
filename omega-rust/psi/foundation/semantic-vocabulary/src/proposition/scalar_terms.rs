//! Scalar terms: scalar and float types, structural path segments, the
//! structural fields a term can name, and the term forms themselves.
//! Constructors and validation share the operand rules owned here; proposition
//! validation consumes these terms before applying proposition-specific rules.

use crate::proposition::{
    IntegerMathTerm, IntegerType, IntegerValue, Proposition, PropositionError,
};
use crate::{
    ByteSequencePredicate, PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
};

mod validation;
mod value_queries;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarType {
    Boolean,
    Integer(IntegerType),
    IeeeFloat(IeeeFloatFormat),
}

impl ScalarType {
    /// The integer descriptor, if this scalar is one.
    pub fn as_integer(&self) -> Option<IntegerType> {
        let ScalarType::Integer(integer) = self else {
            return None;
        };
        Some(*integer)
    }

    /// The IEEE format, if this scalar is a float.
    pub fn as_ieee_float(&self) -> Option<IeeeFloatFormat> {
        let ScalarType::IeeeFloat(format) = self else {
            return None;
        };
        Some(*format)
    }
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

impl IeeeFloatFormat {
    /// Interchange byte extent: 4 for binary32, 8 for binary64.
    pub const fn byte_size(self) -> u16 {
        match self {
            Self::Binary32 => 4,
            Self::Binary64 => 8,
        }
    }
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
    LessThan,
    LessOrEqual,
    /// Logical complement of ordered `<`, including unordered NaN operands.
    NotLessThan,
    /// Logical complement of ordered `<=`, including unordered NaN operands.
    NotLessOrEqual,
}

impl IeeeFloatComparisonKind {
    pub const fn negated(self) -> Self {
        match self {
            Self::Equal => Self::NotEqual,
            Self::NotEqual => Self::Equal,
            Self::LessThan => Self::NotLessThan,
            Self::LessOrEqual => Self::NotLessOrEqual,
            Self::NotLessThan => Self::LessThan,
            Self::NotLessOrEqual => Self::LessOrEqual,
        }
    }

    pub const fn is_symmetric(self) -> bool {
        matches!(self, Self::Equal | Self::NotEqual)
    }
}

/// One nonempty canonical path to a relevant IEEE floating-point field below
/// a Terminal structural root.
macro_rules! canonical_field_accessors {
    () => {
        /// The place this field path is rooted at.
        pub const fn root(&self) -> PlaceId {
            self.root
        }

        /// The canonical path below the root.
        pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
            &self.path
        }

        /// Re-roots this field path at `root`, prepending `prefix`.
        pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
            let mut path = Vec::with_capacity(prefix.len() + self.path.len());
            path.extend_from_slice(prefix);
            path.extend_from_slice(&self.path);
            Self { root, path }
        }
    };
}

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

    canonical_field_accessors!();
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

/// A complete typed value observed in the surrounding predicate's invocation
/// frame, not a storage address or an executable comparison operand. Transport
/// must establish correspondence to that frame; a surviving path alone never
/// carries an identity assumption across a mutation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralIdentitySubject {
    pub(crate) root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
    structural_type: StructuralTypeId,
}

impl StructuralIdentitySubject {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        structural_type: StructuralTypeId,
    ) -> Self {
        Self {
            root,
            path,
            structural_type,
        }
    }

    pub fn root(&self) -> PlaceId {
        self.root
    }
    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        self.structural_type
    }

    /// Rebase an authenticated actual, preserving the exact endpoint type.
    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self {
            root,
            path,
            structural_type: self.structural_type,
        }
    }
}

impl StructuralCaseSubject {
    pub fn new(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Self {
        Self { root, path }
    }

    canonical_field_accessors!();
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

    canonical_field_accessors!();
}

/// An observation path keeps declaration geometry separate from evaluated
/// selectors. Index operands are scalar terms, never executable syntax or a
/// claim that the selected storage is readable or in bounds.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarReadProjection {
    Static(CanonicalStructuralPathSegment),
    Index(Box<ScalarTerm>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarTerm {
    /// A current scalar observation at an exact root and ordered projection.
    /// Only supplied premises establish its relation to an SSA value; this
    /// term grants no access, bounds, case membership or value guarantee.
    StructuralRead {
        root: PlaceId,
        path: Vec<ScalarReadProjection>,
        scalar_type: ScalarType,
    },
    /// Exact IEEE interchange bits, distinct from mathematical equality and
    /// from an integer encoding of those bits.
    IeeeFloatLiteral(IeeeFloatValue),
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
    /// Exact IEEE interchange bits at a nonempty canonical structural path.
    /// The verifier independently checks the relevant leaf and its format.
    /// This observation is not FloatMeaning or an IEEE comparison: payload
    /// identity preserves NaN payloads and distinguishes signed zeros.
    IeeeFloatField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        format: IeeeFloatFormat,
    },
    /// The live extent (element count for element views, byte count for
    /// byte views) of one borrowed view or bounded byte field below a
    /// terminal structural root. `path` is empty for a whole view and selects
    /// the authored byte field when the source nests it — the same rebasing
    /// shape a field leaf takes, so callsite substitution can plant the
    /// extent at the caller's actual prefix. The extent is an opaque
    /// observation discharged by supplied facts, never recomputed.
    ViewExtent {
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
    /// `scalar_type` witnesses the left carrier; the right child retains its
    /// own carrier. No common runtime carrier or implicit conversion is implied.
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
    /// Boolean observation of the exact byte sequence named by this root and
    /// canonical projection. An empty path observes the whole descriptor.
    /// This term grants no predicate truth: operation/arrival evidence must
    /// independently authenticate the current bytes and their custody.
    ByteSequencePredicate {
        predicate: ByteSequencePredicate,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
}

/// Operand-destructuring patterns for the one-operand term forms, shared by
/// the traversals that would otherwise restate this variant list (codec depth
/// walks, substitution, admission budgets). `ScalarTerm` resolves at the call
/// site, so every consumer keeps its own import.
#[macro_export]
macro_rules! scalar_term_unary_operand {
    ($operand:ident) => {
        $crate::ScalarTerm::BooleanNot { operand: $operand }
            | $crate::ScalarTerm::IntegerBitwiseNot {
                operand: $operand,
                ..
            }
            | $crate::ScalarTerm::IntegerWiden {
                operand: $operand,
                ..
            }
            | $crate::ScalarTerm::IntegerExactCast {
                operand: $operand,
                ..
            }
    };
}

/// Two-operand patterns for every binary term form; shift terms bind their
/// `value`/`count` operands to `$left`/`$right` so a traversal sees one
/// uniform pair.
#[macro_export]
macro_rules! scalar_term_binary_operands {
    ($left:ident, $right:ident) => {
        $crate::ScalarTerm::BooleanEqual {
            left: $left,
            right: $right,
        } | $crate::ScalarTerm::IntegerEqual {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerLessThan {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerLessOrEqual {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerBitwiseAnd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerBitwiseOr {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerBitwiseXor {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerShiftLeft {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerShiftRight {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerShiftLeft {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerShiftRight {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerAdd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerSubtract {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerMultiply {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerDivide {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerRemainder {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerDivide {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerRemainder {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerDivide {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerRemainder {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerAdd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerAdd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerSubtract {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerSubtract {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerMultiply {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerMultiply {
            left: $left,
            right: $right,
            ..
        }
    };
}

macro_rules! integer_binary_term {
    ($name:ident => $variant:ident) => {
        integer_binary_term!(@valid $name => $variant, validate_integer_operands);
    };
    (@valid $name:ident => $variant:ident, $validate:ident) => {
        pub fn $name(
            scalar_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Result<Self, PropositionError> {
            $validate(scalar_type, &left, &right)?;
            Ok(Self::$variant {
                scalar_type,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
    };
}

macro_rules! integer_shift_term {
    ($name:ident => $variant:ident) => {
        pub fn $name(
            value_type: IntegerType,
            count_type: IntegerType,
            value: ScalarTerm,
            count: ScalarTerm,
        ) -> Result<Self, PropositionError> {
            validate_integer_shift_operands(value_type, count_type, &value, &count)?;
            Ok(Self::$variant {
                value_type,
                count_type,
                value: Box::new(value),
                count: Box::new(count),
            })
        }
    };
}

macro_rules! integer_binary_term_mismatch {
    ($name:ident => $variant:ident, $error:ident) => {
        pub fn $name(
            scalar_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Result<Self, PropositionError> {
            let expected = ScalarType::Integer(scalar_type);
            if left.scalar_type() != expected || right.scalar_type() != expected {
                return Err(PropositionError::$error {
                    expected,
                    left: left.scalar_type(),
                    right: right.scalar_type(),
                });
            }
            Ok(Self::$variant {
                scalar_type,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
    };
}

macro_rules! scalar_term_operand_table {
    ($self:expr, $borrow:ident) => {
        match $self {
            Self::BooleanNot { operand }
            | Self::IntegerBitwiseNot { operand, .. }
            | Self::IntegerWiden { operand, .. }
            | Self::IntegerExactCast { operand, .. } => vec![operand.$borrow()],
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
                vec![left.$borrow(), right.$borrow()]
            }
            _ => Vec::new(),
        }
    };
}

impl ScalarTerm {
    pub fn structural_read(
        root: PlaceId,
        path: Vec<ScalarReadProjection>,
        scalar_type: ScalarType,
    ) -> Result<Self, PropositionError> {
        let term = Self::StructuralRead {
            root,
            path,
            scalar_type,
        };
        term.validate()?;
        Ok(term)
    }

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

    pub fn ieee_float_field_path(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        format: IeeeFloatFormat,
    ) -> Self {
        Self::IeeeFloatField { root, path, format }
    }

    pub fn view_extent(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    ) -> Self {
        Self::ViewExtent {
            root,
            path,
            scalar_type,
        }
    }

    pub fn byte_sequence_predicate(
        predicate: ByteSequencePredicate,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Self {
        Self::ByteSequencePredicate {
            predicate,
            root,
            path,
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
        if !integer_comparison_operands_match(scalar_type, &left, &right) {
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

    integer_binary_term!(@valid integer_less_than => IntegerLessThan, validate_integer_comparison_operands);

    integer_binary_term!(@valid integer_less_or_equal => IntegerLessOrEqual, validate_integer_comparison_operands);

    /// Interpret a mixed comparison over exact values/literals in the
    /// proof-only integer vocabulary. Each `MathValue` keeps its original
    /// carrier and identity; no runtime cast or representability premise is
    /// introduced. Open compound endpoints retain their Boolean term until
    /// an exact value equality exposes a supported endpoint.
    pub fn mathematical_comparison(&self, positive: bool) -> Option<Proposition> {
        let (kind, left_type, left, right) = match self {
            Self::IntegerEqual {
                scalar_type,
                left,
                right,
            } => (0, *scalar_type, left, right),
            Self::IntegerLessThan {
                scalar_type,
                left,
                right,
            } => (1, *scalar_type, left, right),
            Self::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => (2, *scalar_type, left, right),
            _ => return None,
        };
        if left.scalar_type() == right.scalar_type()
            || !integer_comparison_operands_match(left_type, left, right)
        {
            return None;
        }
        let mathematical = |term: &Self| {
            if let Self::Value {
                id,
                scalar_type: ScalarType::Integer(source_type),
            } = term
            {
                return Some(IntegerMathTerm::MathValue {
                    source_type: *source_type,
                    value: *id,
                });
            }
            term.integer_value()
                .map(|(_, value)| IntegerMathTerm::literal(value))
        };
        let mut left = mathematical(left)?;
        let mut right = mathematical(right)?;
        Some(match (kind, positive) {
            (0, true) => {
                if left > right {
                    std::mem::swap(&mut left, &mut right);
                }
                Proposition::IntegerMathEqual(left, right)
            }
            (0, false) => Proposition::Disjunction(vec![
                Proposition::IntegerMathLessThan(left.clone(), right.clone()),
                Proposition::IntegerMathLessThan(right, left),
            ]),
            (1, true) => Proposition::IntegerMathLessThan(left, right),
            (1, false) => Proposition::IntegerMathLessOrEqual(right, left),
            (_, true) => Proposition::IntegerMathLessOrEqual(left, right),
            (_, false) => Proposition::IntegerMathLessThan(right, left),
        })
    }

    integer_binary_term!(integer_bitwise_and => IntegerBitwiseAnd);

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

    integer_binary_term!(integer_bitwise_or => IntegerBitwiseOr);

    integer_binary_term!(integer_bitwise_xor => IntegerBitwiseXor);

    integer_shift_term!(wrapping_integer_shift_left => WrappingIntegerShiftLeft);

    integer_shift_term!(wrapping_integer_shift_right => WrappingIntegerShiftRight);

    integer_shift_term!(exact_integer_shift_right => ExactIntegerShiftRight);

    integer_shift_term!(exact_integer_shift_left => ExactIntegerShiftLeft);

    integer_binary_term!(exact_integer_add => ExactIntegerAdd);

    pub fn exact_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_exact_subtract_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    integer_binary_term!(exact_integer_multiply => ExactIntegerMultiply);

    integer_binary_term!(exact_integer_divide => ExactIntegerDivide);

    integer_binary_term!(exact_integer_remainder => ExactIntegerRemainder);

    integer_binary_term!(wrapping_integer_divide => WrappingIntegerDivide);

    integer_binary_term!(wrapping_integer_remainder => WrappingIntegerRemainder);

    integer_binary_term!(saturating_integer_divide => SaturatingIntegerDivide);

    integer_binary_term!(saturating_integer_remainder => SaturatingIntegerRemainder);

    pub fn integer(
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Result<Self, PropositionError> {
        if !scalar_type.admits(value) {
            return Err(PropositionError::IntegerLiteralOutsideType { scalar_type, value });
        }
        Ok(Self::Integer { scalar_type, value })
    }

    integer_binary_term_mismatch!(wrapping_integer_add => WrappingIntegerAdd, WrappingIntegerAddTypeMismatch);

    integer_binary_term_mismatch!(saturating_integer_add => SaturatingIntegerAdd, SaturatingIntegerAddTypeMismatch);

    integer_binary_term_mismatch!(wrapping_integer_subtract => WrappingIntegerSubtract, WrappingIntegerSubtractTypeMismatch);

    integer_binary_term_mismatch!(saturating_integer_subtract => SaturatingIntegerSubtract, SaturatingIntegerSubtractTypeMismatch);

    integer_binary_term_mismatch!(wrapping_integer_multiply => WrappingIntegerMultiply, WrappingIntegerMultiplyTypeMismatch);

    integer_binary_term_mismatch!(saturating_integer_multiply => SaturatingIntegerMultiply, SaturatingIntegerMultiplyTypeMismatch);

    /// The direct scalar operands this term owns, in declaration order:
    /// unary operators yield their operand, binary and shift operators yield
    /// `left`/`right` or `value`/`count`, and leaf variants yield none.
    pub fn operands(&self) -> Vec<&ScalarTerm> {
        if let Self::StructuralRead { path, .. } = self {
            return path
                .iter()
                .filter_map(|segment| match segment {
                    ScalarReadProjection::Index(index) => Some(index.as_ref()),
                    ScalarReadProjection::Static(_) => None,
                })
                .collect();
        }
        scalar_term_operand_table!(self, as_ref)
    }

    /// Mutable view of [`Self::operands`], in the same declaration order.
    pub fn operands_mut(&mut self) -> Vec<&mut ScalarTerm> {
        if let Self::StructuralRead { path, .. } = self {
            return path
                .iter_mut()
                .filter_map(|segment| match segment {
                    ScalarReadProjection::Index(index) => Some(index.as_mut()),
                    ScalarReadProjection::Static(_) => None,
                })
                .collect();
        }
        scalar_term_operand_table!(self, as_mut)
    }

    /// Visits every `ValueId` carried by `Value` leaves. Returning `false`
    /// from `visit` short-circuits the traversal and propagates `false`.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId) -> bool) -> bool {
        let mut pending = vec![self];
        while let Some(term) = pending.pop() {
            match term {
                Self::Value { id, .. } => {
                    if !visit(*id) {
                        return false;
                    }
                }
                _ => pending.extend(term.operands()),
            }
        }
        true
    }

    pub fn any_value_id(&self, mut predicate: impl FnMut(ValueId) -> bool) -> bool {
        !self.visit_value_ids(|id| !predicate(id))
    }
}

fn integer_comparison_operands_match(
    left_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> bool {
    let Some(right_type) = right.scalar_type().as_integer() else {
        return false;
    };
    left.scalar_type() == ScalarType::Integer(left_type)
        && (left_type == right_type || (!left_type.is_address() && !right_type.is_address()))
}

fn validate_integer_comparison_operands(
    left_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    if !integer_comparison_operands_match(left_type, left, right) {
        return Err(PropositionError::IntegerOperandTypeMismatch {
            expected: ScalarType::Integer(left_type),
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn validate_exact_subtract_operands(
    result: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    if !matches!((left.scalar_type(), right.scalar_type()),
        (ScalarType::Integer(left), ScalarType::Integer(right))
            if result.admits_exact_subtract_operands(left, right))
    {
        return Err(PropositionError::IntegerOperandTypeMismatch {
            expected: ScalarType::Integer(result),
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn validate_integer_operands(
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

fn validate_integer_shift_operands(
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

/// Worklist step shared by the iterative scalar-term walks: `Visit` expands a
/// node's operands, `Finalize` applies the node's own rule afterward.
enum Step<'a> {
    Visit(&'a ScalarTerm),
    Finalize(&'a ScalarTerm),
}

#[macro_export]
macro_rules! scalar_term_predicate_quint_pattern {
    ($($fields:tt)*) => {
        $crate::ScalarTerm::BooleanField { $($fields)* }
            | $crate::ScalarTerm::ByteSequencePredicate { $($fields)* }
            | $crate::ScalarTerm::IeeeFloatField { $($fields)* }
            | $crate::ScalarTerm::IntegerField { $($fields)* }
            | $crate::ScalarTerm::ViewExtent { $($fields)* }
    };
}
