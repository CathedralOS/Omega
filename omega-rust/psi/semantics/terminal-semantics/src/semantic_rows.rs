//! The operation semantic row: which custody each terminal operation kind
//! falls under, the exact row lookup, and the error when a row is missing
//! or contradicts its operation.

use crate::scalar_leaf_schema::goal_free_scalar_leaf;
use crate::{
    GoalFreeScalarLeafSchema, ScalarLeafCrashPolicy, ScalarLeafDenotation,
    ScalarLeafFrontierPolicy, ScalarLeafGoalShape, ScalarLeafOperandShape, ScalarLeafResultShape,
};
use semantic_vocabulary::{IntegerType, PropositionError, ScalarType, ValueId};
use terminal_psi::OperationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationSemanticCustody {
    LeafDenotation,
    CallComposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationSemanticRow {
    pub(crate) tag: OperationSemanticTag,
    pub(crate) identity: &'static str,
    pub(crate) custody: OperationSemanticCustody,
    pub(crate) goal_free_scalar_leaf: Option<GoalFreeScalarLeafSchema>,
}

impl OperationSemanticRow {
    pub const fn tag(self) -> OperationSemanticTag {
        self.tag
    }

    pub const fn name(self) -> &'static str {
        self.tag.name()
    }

    pub const fn identity(self) -> &'static str {
        self.identity
    }

    pub const fn custody(self) -> OperationSemanticCustody {
        self.custody
    }

    pub const fn goal_free_scalar_leaf(self) -> Option<GoalFreeScalarLeafSchema> {
        self.goal_free_scalar_leaf
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationSemanticError {
    MissingRow(OperationSemanticTag),
    DuplicateRow(OperationSemanticTag),
    MissingScalarResult(OperationSemanticTag),
    UnknownValue(ValueId),
    ResultShapeMismatch(OperationSemanticTag),
    OperandShapeMismatch(OperationSemanticTag),
    DenotationShapeMismatch(OperationSemanticTag),
    MissingProofBearingScalarRow(OperationSemanticTag),
    DuplicateProofBearingScalarRow(OperationSemanticTag),
    UnexpectedProofBearingScalarRow(OperationSemanticTag),
    ProofBearingScalarSchemaMismatch(OperationSemanticTag),
    MissingStructuralEffectRow(OperationSemanticTag),
    DuplicateStructuralEffectRow(OperationSemanticTag),
    UnexpectedStructuralEffectRow(OperationSemanticTag),
    StructuralEffectSchemaMismatch(OperationSemanticTag),
    StructuralEffectResultShapeMismatch(OperationSemanticTag),
    StructuralEffectActionShapeMismatch(OperationSemanticTag),
    MissingCallCompositionRow(OperationSemanticTag),
    DuplicateCallCompositionRow(OperationSemanticTag),
    UnexpectedCallCompositionRow(OperationSemanticTag),
    CallCompositionSchemaMismatch(OperationSemanticTag),
    NonzeroDivisorRequiresFixedInteger(IntegerType),
    NonzeroDivisorTypeMismatch {
        declared: IntegerType,
        actual: ScalarType,
    },
    ExactDivisionRequiresFixedInteger(IntegerType),
    ExactDivisionOperandTypeMismatch {
        declared: IntegerType,
        left: ScalarType,
        right: ScalarType,
    },
    ExactShiftCountRequiresFixedValueInteger(IntegerType),
    ExactShiftCountRequiresFixedCountInteger(IntegerType),
    ExactShiftCountTypeMismatch {
        declared: IntegerType,
        actual: ScalarType,
    },
    ExactShiftLeftValueTypeMismatch {
        declared: IntegerType,
        actual: ScalarType,
    },
    ExactShiftLeftRequiresValueOrLiteralOperand,
    ExactShiftLeftRequiresValueOrLiteralCount,
    ExactArithmeticRequiresFixedInteger(IntegerType),
    ExactArithmeticExpressionTypeMismatch {
        declared: IntegerType,
        actual: ScalarType,
    },
    ExactArithmeticExpressionShapeMismatch,
    ExactArithmeticOperandTypeMismatch {
        declared: IntegerType,
        left: ScalarType,
        right: ScalarType,
    },
    ExactArithmeticRequiresValueOrLiteralOperand,
    ExactCastRequiresFixedSourceInteger(IntegerType),
    ExactCastRequiresFixedTargetInteger(IntegerType),
    ExactCastOperandTypeMismatch {
        declared: IntegerType,
        actual: ScalarType,
    },
    ExactCastRequiresValueOrLiteralOperand,
    InvalidProposition(PropositionError),
}

macro_rules! operation_semantic_rows {
    ($( $variant:ident => ($identity:literal, $custody:ident, $schema:expr) ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum OperationSemanticTag {
            $( $variant ),+
        }

        impl OperationSemanticTag {
            pub const ALL: [Self; operation_semantic_rows!(@count $( $variant )+)] = [
                $( Self::$variant ),+
            ];

            pub const fn name(self) -> &'static str {
                match self {
                    $( Self::$variant => stringify!($variant) ),+
                }
            }

            pub const fn for_operation(operation: &OperationKind) -> Self {
                match operation {
                    $( OperationKind::$variant { .. } => Self::$variant ),+
                }
            }
        }

        impl OperationSemanticRow {
            pub const ALL: [Self; operation_semantic_rows!(@count $( $variant )+)] = [
                $( Self {
                    tag: OperationSemanticTag::$variant,
                    identity: $identity,
                    custody: OperationSemanticCustody::$custody,
                    goal_free_scalar_leaf: $schema,
                } ),+
            ];
        }
    };
    (@count $( $variant:ident )+) => {
        <[()]>::len(&[$(operation_semantic_rows!(@unit $variant)),+])
    };
    (@unit $variant:ident) => { () };
}

use ScalarLeafDenotation as Denotation;
use ScalarLeafOperandShape as Operands;
use ScalarLeafResultShape as ResultShape;

operation_semantic_rows! {
    EstablishReference => ("schema:operation:establish-reference", LeafDenotation, None),
    ReleaseReference => ("schema:operation:release-reference", LeafDenotation, None),
    EstablishPrimitiveLocal => ("schema:operation:establish-primitive-local", LeafDenotation, None),
    PrimitiveScalarRead => ("schema:operation:primitive-scalar-read", LeafDenotation, None),
    StructuralCaseMembership => ("schema:operation:structural-case-membership", LeafDenotation, None),
    WriteOnlyPrimitiveStore => ("schema:operation:write-only-primitive-store", LeafDenotation, None),
    WriteOnlyIndexedPrimitiveStore => ("schema:operation:write-only-indexed-primitive-store", LeafDenotation, None),
    StructuralScalarFieldStore => ("schema:operation:structural-scalar-field-store", LeafDenotation, None),
    MoveStructuralField => ("schema:operation:move-structural-field", LeafDenotation, None),
    StoreStructuralField => ("schema:operation:store-structural-field", LeafDenotation, None),
    StructuralByteSequenceFieldStore => ("schema:operation:structural-byte-sequence-field-store", LeafDenotation, None),
    StructuralByteSequenceFieldLength => ("schema:operation:structural-byte-sequence-field-length", LeafDenotation, None),
    StructuralByteSequenceFieldByteStore => ("schema:operation:structural-byte-sequence-field-byte-store", LeafDenotation, None),
    EstablishScalarCase => ("schema:operation:establish-scalar-case", LeafDenotation, None),
    EstablishScalarArray => ("schema:operation:establish-scalar-array", LeafDenotation, None),
    EstablishByteSequenceLiteral => ("schema:operation:establish-byte-sequence-literal", LeafDenotation, None),
    ByteSequenceLength => ("schema:operation:byte-sequence-length", LeafDenotation, None),
    ByteSequenceWrite => ("schema:operation:byte-sequence-write", LeafDenotation, None),
    ByteSequenceRead => ("schema:operation:byte-sequence-read", LeafDenotation, None),
    ByteSequenceSubslice => ("schema:operation:byte-sequence-subslice", LeafDenotation, None),
    EstablishTrivialAffineLocal => ("schema:operation:establish-trivial-affine-local", LeafDenotation, None),
    EstablishRecord => ("schema:operation:establish-record", LeafDenotation, None),
    StoreDynamicDescriptor => ("schema:operation:store-dynamic-descriptor", LeafDenotation, None),
    Call => ("algebra:call:call", CallComposition, None),
    CallUnit => ("algebra:call:call-unit", CallComposition, None),
    CallStructuralScalar => ("algebra:call:call-structural-scalar", CallComposition, None),
    CallDynamicScalar => ("algebra:call:call-dynamic-scalar", CallComposition, None),
    CallDynamicParameterScalar => ("algebra:call:call-dynamic-parameter-scalar", CallComposition, None),
    CallDynamicUnit => ("algebra:call:call-dynamic-unit", CallComposition, None),
    CallDynamicParameterUnit => ("algebra:call:call-dynamic-parameter-unit", CallComposition, None),
    CallStructural => ("algebra:call:call-structural", CallComposition, None),
    CallStructuralWithScalarArguments => ("algebra:call:call-structural-with-scalar-arguments", CallComposition, None),
    BoundaryCall => ("algebra:call:boundary-call", CallComposition, None),
    PortWrite => ("schema:operation:port-write", LeafDenotation, None),
    IntegerConstant => ("schema:operation:integer-constant", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::IntegerLiteral, Denotation::IntegerConstant)),
    BooleanConstant => ("schema:operation:boolean-constant", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::Boolean, Operands::BooleanLiteral, Denotation::BooleanConstant)),
    IeeeFloatConstant => ("schema:operation:ieee-float-constant", LeafDenotation, None),
    IeeeFloatCompare => ("schema:operation:ieee-float-compare", LeafDenotation, None),
    NearestIeeeFloatFusedMultiplyAdd => ("schema:operation:nearest-ieee-float-fused-multiply-add", LeafDenotation, None),
    BooleanStructuralField => ("schema:operation:boolean-structural-field", LeafDenotation, None),
    IntegerStructuralField => ("schema:operation:integer-structural-field", LeafDenotation, None),
    BooleanNot => ("schema:operation:boolean-not", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::Boolean, Operands::UnaryBoolean, Denotation::BooleanNot)),
    BooleanEqual => ("schema:operation:boolean-equal", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryBoolean, Denotation::BooleanEqual)),
    IntegerEqual => ("schema:operation:integer-equal", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryInteger, Denotation::IntegerEqual)),
    IntegerLessThan => ("schema:operation:integer-less-than", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryInteger, Denotation::IntegerLessThan)),
    IntegerLessOrEqual => ("schema:operation:integer-less-or-equal", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::Boolean, Operands::BinaryInteger, Denotation::IntegerLessOrEqual)),
    IntegerBitwiseNot => ("schema:operation:integer-bitwise-not", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::UnaryInteger, Denotation::IntegerBitwiseNot)),
    IntegerWiden => ("schema:operation:integer-widen", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::WideningInteger, Denotation::IntegerWiden)),
    IntegerExactCast => ("schema:operation:integer-exact-cast", LeafDenotation, None),
    IntegerBitwiseAnd => ("schema:operation:integer-bitwise-and", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::IntegerBitwiseAnd)),
    IntegerBitwiseOr => ("schema:operation:integer-bitwise-or", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::IntegerBitwiseOr)),
    IntegerBitwiseXor => ("schema:operation:integer-bitwise-xor", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::IntegerBitwiseXor)),
    WrappingIntegerShiftLeft => ("schema:operation:wrapping-integer-shift-left", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::IntegerShift, Denotation::WrappingIntegerShiftLeft)),
    WrappingIntegerShiftRight => ("schema:operation:wrapping-integer-shift-right", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::IntegerShift, Denotation::WrappingIntegerShiftRight)),
    ExactIntegerShiftLeft => ("schema:operation:exact-integer-shift-left", LeafDenotation, None),
    ExactIntegerShiftRight => ("schema:operation:exact-integer-shift-right", LeafDenotation, None),
    ExactIntegerAdd => ("schema:operation:exact-integer-add", LeafDenotation, None),
    ExactIntegerSubtract => ("schema:operation:exact-integer-subtract", LeafDenotation, None),
    ExactIntegerMultiply => ("schema:operation:exact-integer-multiply", LeafDenotation, None),
    ExactIntegerDivide => ("schema:operation:exact-integer-divide", LeafDenotation, None),
    ExactIntegerRemainder => ("schema:operation:exact-integer-remainder", LeafDenotation, None),
    WrappingIntegerDivide => ("schema:operation:wrapping-integer-divide", LeafDenotation, None),
    WrappingIntegerRemainder => ("schema:operation:wrapping-integer-remainder", LeafDenotation, None),
    SaturatingIntegerDivide => ("schema:operation:saturating-integer-divide", LeafDenotation, None),
    SaturatingIntegerRemainder => ("schema:operation:saturating-integer-remainder", LeafDenotation, None),
    WrappingIntegerAdd => ("schema:operation:wrapping-integer-add", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::WrappingIntegerAdd)),
    SaturatingIntegerAdd => ("schema:operation:saturating-integer-add", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::SaturatingIntegerAdd)),
    WrappingIntegerSubtract => ("schema:operation:wrapping-integer-subtract", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::WrappingIntegerSubtract)),
    SaturatingIntegerSubtract => ("schema:operation:saturating-integer-subtract", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::SaturatingIntegerSubtract)),
    WrappingIntegerMultiply => ("schema:operation:wrapping-integer-multiply", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::WrappingIntegerMultiply)),
    SaturatingIntegerMultiply => ("schema:operation:saturating-integer-multiply", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::BinaryInteger, Denotation::SaturatingIntegerMultiply)),
}

pub fn exact_operation_semantic_row_in(
    tag: OperationSemanticTag,
    rows: &[OperationSemanticRow],
) -> Result<&OperationSemanticRow, OperationSemanticError> {
    let mut matches = rows.iter().filter(|row| row.tag == tag);
    let row = matches
        .next()
        .ok_or(OperationSemanticError::MissingRow(tag))?;
    if matches.next().is_some() {
        return Err(OperationSemanticError::DuplicateRow(tag));
    }
    Ok(row)
}

pub fn operation_semantic_row(
    operation: &OperationKind,
) -> Result<&'static OperationSemanticRow, OperationSemanticError> {
    exact_operation_semantic_row_in(
        OperationSemanticTag::for_operation(operation),
        &OperationSemanticRow::ALL,
    )
}

/// These scalar computations cannot trap, perform an external event, or change
/// ownership. Their result and any proof use must still be dead before removal.
pub fn is_unconditionally_total_scalar(operation: &OperationKind) -> bool {
    matches!(operation, OperationKind::IeeeFloatConstant { .. })
        || operation_semantic_row(operation).is_ok_and(|row| {
            row.goal_free_scalar_leaf().is_some_and(|schema| {
                schema.goal() == ScalarLeafGoalShape::None
                    && schema.crash() == ScalarLeafCrashPolicy::Never
                    && schema.frontier() == ScalarLeafFrontierPolicy::PreserveLocal
            })
        })
}
