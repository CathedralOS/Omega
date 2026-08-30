#![forbid(unsafe_code)]

//! Closed, target-neutral semantic tables for terminal Psi.
//!
//! This crate owns declarative operation-row identity and the local semantics
//! that can be interpreted without control-flow, call-composition, or proof
//! reduction policy. It deliberately does not own traversal, evidence
//! availability, sufficient-form reduction, or provider realization.

use std::collections::BTreeMap;

use psi_core::{
    IntegerType, IntegerValue, Proposition, PropositionError, ScalarTerm, ScalarType, ValueId,
};
use psi_terminal::{Operation, OperationKind};

mod call_composition;
mod proof_bearing_scalar;
mod structural_effect;

pub use call_composition::*;
pub use proof_bearing_scalar::*;
pub use structural_effect::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationSemanticCustody {
    LeafDenotation,
    CallComposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafResultShape {
    DeclaredInteger,
    Boolean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafOperandShape {
    IntegerLiteral,
    BooleanLiteral,
    UnaryBoolean,
    BinaryBoolean,
    UnaryInteger,
    BinaryInteger,
    WideningInteger,
    ExactCastInteger,
    IntegerShift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafDenotation {
    IntegerConstant,
    BooleanConstant,
    BooleanNot,
    BooleanEqual,
    IntegerEqual,
    IntegerLessThan,
    IntegerLessOrEqual,
    IntegerBitwiseNot,
    IntegerWiden,
    IntegerExactCast,
    IntegerBitwiseAnd,
    IntegerBitwiseOr,
    IntegerBitwiseXor,
    WrappingIntegerShiftLeft,
    WrappingIntegerShiftRight,
    ExactIntegerShiftLeft,
    ExactIntegerShiftRight,
    ExactIntegerAdd,
    ExactIntegerSubtract,
    ExactIntegerMultiply,
    ExactIntegerDivide,
    ExactIntegerRemainder,
    WrappingIntegerDivide,
    WrappingIntegerRemainder,
    SaturatingIntegerDivide,
    SaturatingIntegerRemainder,
    WrappingIntegerAdd,
    SaturatingIntegerAdd,
    WrappingIntegerSubtract,
    SaturatingIntegerSubtract,
    WrappingIntegerMultiply,
    SaturatingIntegerMultiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafGoalShape {
    None,
    ExactCastRepresentable,
    ExactShiftCount,
    ExactShiftLeftRepresentable,
    ExactArithmeticRepresentable,
    ExactDivisionDefined,
    NonzeroDivisor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafFactShape {
    ResultEquation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafCrashPolicy {
    Never,
    CrashesUnlessGoal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafFuelPolicy {
    ConsumeOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarLeafFrontierPolicy {
    PreserveLocal,
}

/// One total scalar leaf row whose direct result equation needs no proof
/// reduction. Every semantic axis remains explicit even where this first Rust
/// cohort has only one admitted policy value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoalFreeScalarLeafSchema {
    result: ScalarLeafResultShape,
    operands: ScalarLeafOperandShape,
    denotation: ScalarLeafDenotation,
    goal: ScalarLeafGoalShape,
    fact: ScalarLeafFactShape,
    crash: ScalarLeafCrashPolicy,
    fuel: ScalarLeafFuelPolicy,
    frontier: ScalarLeafFrontierPolicy,
}

impl GoalFreeScalarLeafSchema {
    pub const fn result(self) -> ScalarLeafResultShape {
        self.result
    }

    pub const fn operands(self) -> ScalarLeafOperandShape {
        self.operands
    }

    pub const fn denotation(self) -> ScalarLeafDenotation {
        self.denotation
    }

    pub const fn goal(self) -> ScalarLeafGoalShape {
        self.goal
    }

    pub const fn fact(self) -> ScalarLeafFactShape {
        self.fact
    }

    pub const fn crash(self) -> ScalarLeafCrashPolicy {
        self.crash
    }

    pub const fn fuel(self) -> ScalarLeafFuelPolicy {
        self.fuel
    }

    pub const fn frontier(self) -> ScalarLeafFrontierPolicy {
        self.frontier
    }
}

const fn goal_free_scalar_leaf(
    result: ScalarLeafResultShape,
    operands: ScalarLeafOperandShape,
    denotation: ScalarLeafDenotation,
) -> Option<GoalFreeScalarLeafSchema> {
    Some(GoalFreeScalarLeafSchema {
        result,
        operands,
        denotation,
        goal: ScalarLeafGoalShape::None,
        fact: ScalarLeafFactShape::ResultEquation,
        crash: ScalarLeafCrashPolicy::Never,
        fuel: ScalarLeafFuelPolicy::ConsumeOne,
        frontier: ScalarLeafFrontierPolicy::PreserveLocal,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationSemanticRow {
    tag: OperationSemanticTag,
    identity: &'static str,
    custody: OperationSemanticCustody,
    goal_free_scalar_leaf: Option<GoalFreeScalarLeafSchema>,
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
    WriteOnlyPrimitiveStore => ("schema:operation:write-only-primitive-store", LeafDenotation, None),
    EstablishPayloadlessCase => ("schema:operation:establish-payloadless-case", LeafDenotation, None),
    EstablishByteSequenceLiteral => ("schema:operation:establish-byte-sequence-literal", LeafDenotation, None),
    EstablishTrivialAffineLocal => ("schema:operation:establish-trivial-affine-local", LeafDenotation, None),
    Call => ("algebra:call:call", CallComposition, None),
    CallUnit => ("algebra:call:call-unit", CallComposition, None),
    CallStructuralScalar => ("algebra:call:call-structural-scalar", CallComposition, None),
    CallStructural => ("algebra:call:call-structural", CallComposition, None),
    BoundaryCall => ("algebra:call:boundary-call", CallComposition, None),
    PortWrite => ("schema:operation:port-write", LeafDenotation, None),
    IntegerConstant => ("schema:operation:integer-constant", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::DeclaredInteger, Operands::IntegerLiteral, Denotation::IntegerConstant)),
    BooleanConstant => ("schema:operation:boolean-constant", LeafDenotation,
        goal_free_scalar_leaf(ResultShape::Boolean, Operands::BooleanLiteral, Denotation::BooleanConstant)),
    BooleanStructuralField => ("schema:operation:boolean-structural-field", LeafDenotation, None),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarLeafInputs {
    IntegerLiteral(IntegerValue),
    BooleanLiteral(bool),
    Unary(ValueId),
    Binary(ValueId, ValueId),
}

fn scalar_leaf_inputs(operation: &OperationKind) -> Option<ScalarLeafInputs> {
    match operation {
        OperationKind::IntegerConstant { value } => Some(ScalarLeafInputs::IntegerLiteral(*value)),
        OperationKind::BooleanConstant { value } => Some(ScalarLeafInputs::BooleanLiteral(*value)),
        OperationKind::BooleanNot { operand }
        | OperationKind::IntegerBitwiseNot { operand }
        | OperationKind::IntegerWiden { operand } => Some(ScalarLeafInputs::Unary(*operand)),
        OperationKind::BooleanEqual { left, right }
        | OperationKind::IntegerEqual { left, right }
        | OperationKind::IntegerLessThan { left, right }
        | OperationKind::IntegerLessOrEqual { left, right }
        | OperationKind::IntegerBitwiseAnd { left, right }
        | OperationKind::IntegerBitwiseOr { left, right }
        | OperationKind::IntegerBitwiseXor { left, right }
        | OperationKind::WrappingIntegerShiftLeft {
            value: left,
            count: right,
        }
        | OperationKind::WrappingIntegerShiftRight {
            value: left,
            count: right,
        }
        | OperationKind::WrappingIntegerAdd { left, right }
        | OperationKind::SaturatingIntegerAdd { left, right }
        | OperationKind::WrappingIntegerSubtract { left, right }
        | OperationKind::SaturatingIntegerSubtract { left, right }
        | OperationKind::WrappingIntegerMultiply { left, right }
        | OperationKind::SaturatingIntegerMultiply { left, right } => {
            Some(ScalarLeafInputs::Binary(*left, *right))
        }
        _ => None,
    }
}

fn value_term(
    value: ValueId,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<ScalarTerm, OperationSemanticError> {
    value_types
        .get(&value)
        .copied()
        .map(|scalar_type| ScalarTerm::value(value, scalar_type))
        .ok_or(OperationSemanticError::UnknownValue(value))
}

fn integer_type(term: &ScalarTerm) -> Option<psi_core::IntegerType> {
    match term.scalar_type() {
        ScalarType::Integer(integer_type) => Some(integer_type),
        ScalarType::Boolean => None,
    }
}

fn validate_result_shape(
    tag: OperationSemanticTag,
    shape: ScalarLeafResultShape,
    actual: ScalarType,
) -> Result<(), OperationSemanticError> {
    let valid = match shape {
        ScalarLeafResultShape::DeclaredInteger => matches!(actual, ScalarType::Integer(_)),
        ScalarLeafResultShape::Boolean => actual == ScalarType::Boolean,
    };
    valid
        .then_some(())
        .ok_or(OperationSemanticError::ResultShapeMismatch(tag))
}

fn validate_operand_shape(
    tag: OperationSemanticTag,
    schema: GoalFreeScalarLeafSchema,
    inputs: ScalarLeafInputs,
    result_type: ScalarType,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<(), OperationSemanticError> {
    let type_of = |value: ValueId| {
        value_types
            .get(&value)
            .copied()
            .ok_or(OperationSemanticError::UnknownValue(value))
    };
    let valid = match (schema.operands, inputs) {
        (ScalarLeafOperandShape::IntegerLiteral, ScalarLeafInputs::IntegerLiteral(_))
        | (ScalarLeafOperandShape::BooleanLiteral, ScalarLeafInputs::BooleanLiteral(_)) => true,
        (ScalarLeafOperandShape::UnaryBoolean, ScalarLeafInputs::Unary(operand)) => {
            type_of(operand)? == ScalarType::Boolean
        }
        (ScalarLeafOperandShape::BinaryBoolean, ScalarLeafInputs::Binary(left, right)) => {
            type_of(left)? == ScalarType::Boolean && type_of(right)? == ScalarType::Boolean
        }
        (ScalarLeafOperandShape::UnaryInteger, ScalarLeafInputs::Unary(operand)) => {
            matches!(result_type, ScalarType::Integer(_)) && type_of(operand)? == result_type
        }
        (ScalarLeafOperandShape::BinaryInteger, ScalarLeafInputs::Binary(left, right)) => {
            let left_type = type_of(left)?;
            let right_type = type_of(right)?;
            matches!(left_type, ScalarType::Integer(_))
                && left_type == right_type
                && (result_type == ScalarType::Boolean || result_type == left_type)
        }
        (ScalarLeafOperandShape::WideningInteger, ScalarLeafInputs::Unary(operand)) => {
            let (ScalarType::Integer(source), ScalarType::Integer(target)) =
                (type_of(operand)?, result_type)
            else {
                return Err(OperationSemanticError::OperandShapeMismatch(tag));
            };
            source.can_widen_to(target)
        }
        (ScalarLeafOperandShape::IntegerShift, ScalarLeafInputs::Binary(value, count)) => {
            type_of(value)? == result_type
                && matches!(result_type, ScalarType::Integer(_))
                && matches!(type_of(count)?, ScalarType::Integer(_))
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(OperationSemanticError::OperandShapeMismatch(tag))
}

fn denotation_term(
    tag: OperationSemanticTag,
    schema: GoalFreeScalarLeafSchema,
    inputs: ScalarLeafInputs,
    result_type: ScalarType,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<ScalarTerm, OperationSemanticError> {
    let term = |value| value_term(value, value_types);
    let invalid = || OperationSemanticError::DenotationShapeMismatch(tag);
    let built = match (schema.denotation, inputs) {
        (ScalarLeafDenotation::IntegerConstant, ScalarLeafInputs::IntegerLiteral(value)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer(integer_type, value)
        }
        (ScalarLeafDenotation::BooleanConstant, ScalarLeafInputs::BooleanLiteral(value)) => {
            return Ok(ScalarTerm::boolean(value));
        }
        (ScalarLeafDenotation::BooleanNot, ScalarLeafInputs::Unary(operand)) => {
            ScalarTerm::boolean_not(term(operand)?)
        }
        (ScalarLeafDenotation::BooleanEqual, ScalarLeafInputs::Binary(left, right)) => {
            ScalarTerm::boolean_equal(term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerEqual, ScalarLeafInputs::Binary(left, right)) => {
            let left = term(left)?;
            let integer_type = integer_type(&left).ok_or_else(invalid)?;
            ScalarTerm::integer_equal(integer_type, left, term(right)?)
        }
        (ScalarLeafDenotation::IntegerLessThan, ScalarLeafInputs::Binary(left, right)) => {
            let left = term(left)?;
            let integer_type = integer_type(&left).ok_or_else(invalid)?;
            ScalarTerm::integer_less_than(integer_type, left, term(right)?)
        }
        (ScalarLeafDenotation::IntegerLessOrEqual, ScalarLeafInputs::Binary(left, right)) => {
            let left = term(left)?;
            let integer_type = integer_type(&left).ok_or_else(invalid)?;
            ScalarTerm::integer_less_or_equal(integer_type, left, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseNot, ScalarLeafInputs::Unary(operand)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_not(integer_type, term(operand)?)
        }
        (ScalarLeafDenotation::IntegerWiden, ScalarLeafInputs::Unary(operand)) => {
            let operand = term(operand)?;
            let source_type = integer_type(&operand).ok_or_else(invalid)?;
            let ScalarType::Integer(target_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_widen(source_type, target_type, operand)
        }
        (ScalarLeafDenotation::IntegerBitwiseAnd, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_and(integer_type, term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseOr, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_or(integer_type, term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseXor, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_xor(integer_type, term(left)?, term(right)?)
        }
        (
            ScalarLeafDenotation::WrappingIntegerShiftLeft,
            ScalarLeafInputs::Binary(value, count),
        ) => {
            let ScalarType::Integer(value_type) = result_type else {
                return Err(invalid());
            };
            let count = term(count)?;
            let count_type = integer_type(&count).ok_or_else(invalid)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, term(value)?, count)
        }
        (
            ScalarLeafDenotation::WrappingIntegerShiftRight,
            ScalarLeafInputs::Binary(value, count),
        ) => {
            let ScalarType::Integer(value_type) = result_type else {
                return Err(invalid());
            };
            let count = term(count)?;
            let count_type = integer_type(&count).ok_or_else(invalid)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, term(value)?, count)
        }
        (denotation, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            let left = term(left)?;
            let right = term(right)?;
            match denotation {
                ScalarLeafDenotation::WrappingIntegerAdd => {
                    ScalarTerm::wrapping_integer_add(integer_type, left, right)
                }
                ScalarLeafDenotation::SaturatingIntegerAdd => {
                    ScalarTerm::saturating_integer_add(integer_type, left, right)
                }
                ScalarLeafDenotation::WrappingIntegerSubtract => {
                    ScalarTerm::wrapping_integer_subtract(integer_type, left, right)
                }
                ScalarLeafDenotation::SaturatingIntegerSubtract => {
                    ScalarTerm::saturating_integer_subtract(integer_type, left, right)
                }
                ScalarLeafDenotation::WrappingIntegerMultiply => {
                    ScalarTerm::wrapping_integer_multiply(integer_type, left, right)
                }
                ScalarLeafDenotation::SaturatingIntegerMultiply => {
                    ScalarTerm::saturating_integer_multiply(integer_type, left, right)
                }
                _ => return Err(invalid()),
            }
        }
        _ => return Err(invalid()),
    };
    built.map_err(OperationSemanticError::InvalidProposition)
}

/// Interpret one goal-free scalar leaf through its exact declarative row.
/// `Ok(None)` means the operation belongs to a different semantic algebra.
pub fn goal_free_scalar_leaf_equation(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let row = operation_semantic_row(&operation.kind)?;
    let Some(schema) = row.goal_free_scalar_leaf else {
        return Ok(None);
    };
    let inputs = scalar_leaf_inputs(&operation.kind)
        .ok_or(OperationSemanticError::OperandShapeMismatch(row.tag))?;
    let result = operation
        .result
        .scalar_ref()
        .ok_or(OperationSemanticError::MissingScalarResult(row.tag))?;
    validate_result_shape(row.tag, schema.result, result.scalar_type)?;
    validate_operand_shape(row.tag, schema, inputs, result.scalar_type, value_types)?;
    let denotation = denotation_term(row.tag, schema, inputs, result.scalar_type, value_types)?;
    Ok(Some(Proposition::Equal(
        ScalarTerm::value(result.id, result.scalar_type),
        denotation,
    )))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use psi_core::{IntegerSign, IntegerType, OperationId, ScalarType, ValueId};
    use psi_terminal::{Operation, OperationKind, OperationResult, ValueDeclaration};

    use super::*;

    fn i8_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("i8"))
    }

    #[test]
    fn operation_inventory_is_exact_unique_and_closed() {
        assert_eq!(OperationSemanticTag::ALL.len(), 43);
        assert_eq!(OperationSemanticRow::ALL.len(), 43);
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .filter(|row| row.custody == OperationSemanticCustody::LeafDenotation)
                .count(),
            38,
        );
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .filter(|row| row.custody == OperationSemanticCustody::CallComposition)
                .count(),
            5,
        );
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .filter(|row| row.goal_free_scalar_leaf.is_some())
                .count(),
            20,
        );
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .map(|row| row.tag)
                .collect::<BTreeSet<_>>()
                .len(),
            43,
        );
        assert_eq!(
            OperationSemanticRow::ALL
                .iter()
                .map(|row| row.identity)
                .collect::<BTreeSet<_>>()
                .len(),
            43,
        );
        assert!(
            OperationSemanticRow::ALL
                .iter()
                .all(|row| !row.identity.is_empty()),
        );
    }

    #[test]
    fn exact_row_lookup_rejects_missing_and_duplicate_rows() {
        let tag = OperationSemanticTag::WrappingIntegerAdd;
        let canonical = *exact_operation_semantic_row_in(tag, &OperationSemanticRow::ALL).unwrap();
        let missing = OperationSemanticRow::ALL
            .iter()
            .copied()
            .filter(|row| row.tag != tag)
            .collect::<Vec<_>>();
        assert_eq!(
            exact_operation_semantic_row_in(tag, &missing),
            Err(OperationSemanticError::MissingRow(tag)),
        );
        let mut duplicate = OperationSemanticRow::ALL.to_vec();
        duplicate.push(canonical);
        assert_eq!(
            exact_operation_semantic_row_in(tag, &duplicate),
            Err(OperationSemanticError::DuplicateRow(tag)),
        );
    }

    #[test]
    fn goal_free_scalar_rows_emit_equations_through_one_interpreter() {
        let left = ValueId::new(1).unwrap();
        let right = ValueId::new(2).unwrap();
        let result = ValueId::new(3).unwrap();
        let operation = Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: i8_type(),
            }),
            kind: OperationKind::WrappingIntegerAdd { left, right },
        };
        let value_types = BTreeMap::from([(left, i8_type()), (right, i8_type())]);
        let actual = goal_free_scalar_leaf_equation(&operation, &value_types)
            .unwrap()
            .unwrap();
        let integer_type = match i8_type() {
            ScalarType::Integer(integer_type) => integer_type,
            ScalarType::Boolean => unreachable!(),
        };
        assert_eq!(
            actual,
            Proposition::Equal(
                ScalarTerm::value(result, i8_type()),
                ScalarTerm::wrapping_integer_add(
                    integer_type,
                    ScalarTerm::value(left, i8_type()),
                    ScalarTerm::value(right, i8_type()),
                )
                .unwrap(),
            ),
        );
    }

    #[test]
    fn goal_free_scalar_rows_fail_closed_on_type_drift() {
        let left = ValueId::new(1).unwrap();
        let right = ValueId::new(2).unwrap();
        let result = ValueId::new(3).unwrap();
        let operation = Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: i8_type(),
            }),
            kind: OperationKind::WrappingIntegerAdd { left, right },
        };
        let value_types = BTreeMap::from([(left, i8_type()), (right, ScalarType::Boolean)]);
        assert_eq!(
            goal_free_scalar_leaf_equation(&operation, &value_types),
            Err(OperationSemanticError::OperandShapeMismatch(
                OperationSemanticTag::WrappingIntegerAdd,
            )),
        );
    }
}
