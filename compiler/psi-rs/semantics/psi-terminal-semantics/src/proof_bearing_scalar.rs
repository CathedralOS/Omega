//! Exact proof-bearing scalar leaf schemas and their canonical local meaning.
//!
//! This table owns operation-local denotation and canonical goal identity. It
//! deliberately does not choose a sufficient preimage, inspect predecessor
//! definitions, or decide which proof-search strategy can discharge the goal.

use std::collections::BTreeMap;

use psi_core::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition, ScalarTerm,
    ScalarType, ValueId,
};
use psi_terminal::{Operation, OperationKind};

use super::{
    OperationSemanticError, OperationSemanticTag, ScalarLeafCrashPolicy, ScalarLeafDenotation,
    ScalarLeafFactShape, ScalarLeafFrontierPolicy, ScalarLeafFuelPolicy, ScalarLeafGoalShape,
    ScalarLeafOperandShape, ScalarLeafResultShape, integer_type, value_term,
};

/// One proof-bearing scalar leaf row. The canonical goal is operation-local;
/// any multi-operation affine, interval, shift, or divide summary belongs to
/// an untrusted certificate producer rather than this schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofBearingScalarLeafSchema {
    result: ScalarLeafResultShape,
    operands: ScalarLeafOperandShape,
    denotation: ScalarLeafDenotation,
    goal: ScalarLeafGoalShape,
    fact: ScalarLeafFactShape,
    crash: ScalarLeafCrashPolicy,
    fuel: ScalarLeafFuelPolicy,
    frontier: ScalarLeafFrontierPolicy,
}

impl ProofBearingScalarLeafSchema {
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

const fn proof_bearing_scalar_leaf(
    operands: ScalarLeafOperandShape,
    denotation: ScalarLeafDenotation,
    goal: ScalarLeafGoalShape,
) -> ProofBearingScalarLeafSchema {
    ProofBearingScalarLeafSchema {
        result: ScalarLeafResultShape::DeclaredInteger,
        operands,
        denotation,
        goal,
        fact: ScalarLeafFactShape::ResultEquation,
        crash: ScalarLeafCrashPolicy::CrashesUnlessGoal,
        fuel: ScalarLeafFuelPolicy::ConsumeOne,
        frontier: ScalarLeafFrontierPolicy::PreserveLocal,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofBearingScalarSemanticRow {
    tag: OperationSemanticTag,
    schema: ProofBearingScalarLeafSchema,
}

impl ProofBearingScalarSemanticRow {
    pub const ALL: [Self; 12] = [
        Self {
            tag: OperationSemanticTag::IntegerExactCast,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::ExactCastInteger,
                ScalarLeafDenotation::IntegerExactCast,
                ScalarLeafGoalShape::ExactCastRepresentable,
            ),
        },
        Self {
            tag: OperationSemanticTag::ExactIntegerShiftLeft,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::IntegerShift,
                ScalarLeafDenotation::ExactIntegerShiftLeft,
                ScalarLeafGoalShape::ExactShiftLeftRepresentable,
            ),
        },
        Self {
            tag: OperationSemanticTag::ExactIntegerShiftRight,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::IntegerShift,
                ScalarLeafDenotation::ExactIntegerShiftRight,
                ScalarLeafGoalShape::ExactShiftCount,
            ),
        },
        Self {
            tag: OperationSemanticTag::ExactIntegerAdd,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::ExactIntegerAdd,
                ScalarLeafGoalShape::ExactArithmeticRepresentable,
            ),
        },
        Self {
            tag: OperationSemanticTag::ExactIntegerSubtract,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::ExactIntegerSubtract,
                ScalarLeafGoalShape::ExactArithmeticRepresentable,
            ),
        },
        Self {
            tag: OperationSemanticTag::ExactIntegerMultiply,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::ExactIntegerMultiply,
                ScalarLeafGoalShape::ExactArithmeticRepresentable,
            ),
        },
        Self {
            tag: OperationSemanticTag::ExactIntegerDivide,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::ExactIntegerDivide,
                ScalarLeafGoalShape::ExactDivisionDefined,
            ),
        },
        Self {
            tag: OperationSemanticTag::ExactIntegerRemainder,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::ExactIntegerRemainder,
                ScalarLeafGoalShape::ExactDivisionDefined,
            ),
        },
        Self {
            tag: OperationSemanticTag::WrappingIntegerDivide,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::WrappingIntegerDivide,
                ScalarLeafGoalShape::NonzeroDivisor,
            ),
        },
        Self {
            tag: OperationSemanticTag::WrappingIntegerRemainder,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::WrappingIntegerRemainder,
                ScalarLeafGoalShape::NonzeroDivisor,
            ),
        },
        Self {
            tag: OperationSemanticTag::SaturatingIntegerDivide,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::SaturatingIntegerDivide,
                ScalarLeafGoalShape::NonzeroDivisor,
            ),
        },
        Self {
            tag: OperationSemanticTag::SaturatingIntegerRemainder,
            schema: proof_bearing_scalar_leaf(
                ScalarLeafOperandShape::BinaryInteger,
                ScalarLeafDenotation::SaturatingIntegerRemainder,
                ScalarLeafGoalShape::NonzeroDivisor,
            ),
        },
    ];

    pub const fn tag(self) -> OperationSemanticTag {
        self.tag
    }

    pub const fn schema(self) -> ProofBearingScalarLeafSchema {
        self.schema
    }
}

const PROOF_BEARING_SCALAR_TAGS: [OperationSemanticTag; 12] = [
    OperationSemanticTag::IntegerExactCast,
    OperationSemanticTag::ExactIntegerShiftLeft,
    OperationSemanticTag::ExactIntegerShiftRight,
    OperationSemanticTag::ExactIntegerAdd,
    OperationSemanticTag::ExactIntegerSubtract,
    OperationSemanticTag::ExactIntegerMultiply,
    OperationSemanticTag::ExactIntegerDivide,
    OperationSemanticTag::ExactIntegerRemainder,
    OperationSemanticTag::WrappingIntegerDivide,
    OperationSemanticTag::WrappingIntegerRemainder,
    OperationSemanticTag::SaturatingIntegerDivide,
    OperationSemanticTag::SaturatingIntegerRemainder,
];

const fn is_proof_bearing_scalar_tag(tag: OperationSemanticTag) -> bool {
    matches!(
        tag,
        OperationSemanticTag::IntegerExactCast
            | OperationSemanticTag::ExactIntegerShiftLeft
            | OperationSemanticTag::ExactIntegerShiftRight
            | OperationSemanticTag::ExactIntegerAdd
            | OperationSemanticTag::ExactIntegerSubtract
            | OperationSemanticTag::ExactIntegerMultiply
            | OperationSemanticTag::ExactIntegerDivide
            | OperationSemanticTag::ExactIntegerRemainder
            | OperationSemanticTag::WrappingIntegerDivide
            | OperationSemanticTag::WrappingIntegerRemainder
            | OperationSemanticTag::SaturatingIntegerDivide
            | OperationSemanticTag::SaturatingIntegerRemainder
    )
}

fn canonical_schema(tag: OperationSemanticTag) -> Option<ProofBearingScalarLeafSchema> {
    ProofBearingScalarSemanticRow::ALL
        .iter()
        .find(|row| row.tag == tag)
        .map(|row| row.schema)
}

pub fn exact_proof_bearing_scalar_semantic_row_in(
    tag: OperationSemanticTag,
    rows: &[ProofBearingScalarSemanticRow],
) -> Result<Option<&ProofBearingScalarSemanticRow>, OperationSemanticError> {
    if !is_proof_bearing_scalar_tag(tag) {
        return Ok(None);
    }
    let mut matches = rows.iter().filter(|row| row.tag == tag);
    let row = matches
        .next()
        .ok_or(OperationSemanticError::MissingProofBearingScalarRow(tag))?;
    if matches.next().is_some() {
        return Err(OperationSemanticError::DuplicateProofBearingScalarRow(tag));
    }
    Ok(Some(row))
}

pub fn validate_proof_bearing_scalar_semantic_rows(
    rows: &[ProofBearingScalarSemanticRow],
) -> Result<(), OperationSemanticError> {
    for row in rows {
        if !is_proof_bearing_scalar_tag(row.tag) {
            return Err(OperationSemanticError::UnexpectedProofBearingScalarRow(
                row.tag,
            ));
        }
    }
    for tag in PROOF_BEARING_SCALAR_TAGS {
        let row = exact_proof_bearing_scalar_semantic_row_in(tag, rows)?
            .expect("the requested tag belongs to the proof-bearing scalar cohort");
        if Some(row.schema) != canonical_schema(tag) {
            return Err(OperationSemanticError::ProofBearingScalarSchemaMismatch(
                tag,
            ));
        }
    }
    Ok(())
}

pub fn proof_bearing_scalar_semantic_row(
    operation: &OperationKind,
) -> Result<Option<&'static ProofBearingScalarSemanticRow>, OperationSemanticError> {
    validate_proof_bearing_scalar_semantic_rows(&ProofBearingScalarSemanticRow::ALL)?;
    exact_proof_bearing_scalar_semantic_row_in(
        OperationSemanticTag::for_operation(operation),
        &ProofBearingScalarSemanticRow::ALL,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalScalarGoal {
    ExactCastRepresentable {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    },
    ExactShiftCount {
        value_type: IntegerType,
        count_type: IntegerType,
        count: ScalarTerm,
    },
    ExactShiftLeftRepresentable {
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    },
    ExactArithmeticRepresentable {
        integer_type: IntegerType,
        expression: ScalarTerm,
    },
    ExactDivisionDefined {
        integer_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    },
    NonzeroDivisor {
        integer_type: IntegerType,
        divisor: ScalarTerm,
    },
}

impl CanonicalScalarGoal {
    pub const fn shape(&self) -> ScalarLeafGoalShape {
        match self {
            Self::ExactCastRepresentable { .. } => ScalarLeafGoalShape::ExactCastRepresentable,
            Self::ExactShiftCount { .. } => ScalarLeafGoalShape::ExactShiftCount,
            Self::ExactShiftLeftRepresentable { .. } => {
                ScalarLeafGoalShape::ExactShiftLeftRepresentable
            }
            Self::ExactArithmeticRepresentable { .. } => {
                ScalarLeafGoalShape::ExactArithmeticRepresentable
            }
            Self::ExactDivisionDefined { .. } => ScalarLeafGoalShape::ExactDivisionDefined,
            Self::NonzeroDivisor { .. } => ScalarLeafGoalShape::NonzeroDivisor,
        }
    }

    /// Project the one canonical goal whose proposition vocabulary is fully
    /// settled. Other goal shapes remain explicit typed carriers until their
    /// own exact proposition mappings land.
    pub fn kernel_proposition(&self) -> Result<Option<Proposition>, OperationSemanticError> {
        let Self::NonzeroDivisor {
            integer_type,
            divisor,
        } = self
        else {
            return Ok(None);
        };
        if integer_type.carrier() != IntegerCarrier::Fixed {
            return Err(OperationSemanticError::NonzeroDivisorRequiresFixedInteger(
                *integer_type,
            ));
        }
        if divisor.scalar_type() != ScalarType::Integer(*integer_type) {
            return Err(OperationSemanticError::NonzeroDivisorTypeMismatch {
                declared: *integer_type,
                actual: divisor.scalar_type(),
            });
        }

        let proposition = match integer_type.sign() {
            IntegerSign::Unsigned => Proposition::LessOrEqual(
                ScalarTerm::integer(*integer_type, IntegerValue::Unsigned(1))
                    .map_err(OperationSemanticError::InvalidProposition)?,
                divisor.clone(),
            ),
            IntegerSign::Signed => {
                let negative = Proposition::LessOrEqual(
                    divisor.clone(),
                    ScalarTerm::integer(*integer_type, IntegerValue::Signed(-1))
                        .map_err(OperationSemanticError::InvalidProposition)?,
                );
                if integer_type.bits() == 1 {
                    negative
                } else {
                    Proposition::Disjunction(vec![
                        negative,
                        Proposition::LessOrEqual(
                            ScalarTerm::integer(*integer_type, IntegerValue::Signed(1))
                                .map_err(OperationSemanticError::InvalidProposition)?,
                            divisor.clone(),
                        ),
                    ])
                }
            }
        };
        proposition
            .validate()
            .map_err(OperationSemanticError::InvalidProposition)?;
        Ok(Some(proposition))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofBearingScalarLeafSemantics {
    tag: OperationSemanticTag,
    schema: ProofBearingScalarLeafSchema,
    obligation: ObligationId,
    canonical_goal: CanonicalScalarGoal,
    denotation: ScalarTerm,
    result_equation: Proposition,
}

impl ProofBearingScalarLeafSemantics {
    pub const fn tag(&self) -> OperationSemanticTag {
        self.tag
    }

    pub const fn schema(&self) -> ProofBearingScalarLeafSchema {
        self.schema
    }

    pub const fn obligation(&self) -> ObligationId {
        self.obligation
    }

    pub const fn canonical_goal(&self) -> &CanonicalScalarGoal {
        &self.canonical_goal
    }

    pub const fn denotation(&self) -> &ScalarTerm {
        &self.denotation
    }

    pub const fn result_equation(&self) -> &Proposition {
        &self.result_equation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProofBearingInputs {
    Unary(ValueId),
    Binary(ValueId, ValueId),
}

fn proof_bearing_inputs(operation: &OperationKind) -> Option<(ProofBearingInputs, ObligationId)> {
    match operation {
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        } => Some((ProofBearingInputs::Unary(*operand), *obligation)),
        OperationKind::ExactIntegerShiftLeft {
            value,
            count,
            obligation,
        }
        | OperationKind::ExactIntegerShiftRight {
            value,
            count,
            obligation,
        } => Some((ProofBearingInputs::Binary(*value, *count), *obligation)),
        OperationKind::ExactIntegerAdd {
            left,
            right,
            obligation,
        }
        | OperationKind::ExactIntegerSubtract {
            left,
            right,
            obligation,
        }
        | OperationKind::ExactIntegerMultiply {
            left,
            right,
            obligation,
        }
        | OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        }
        | OperationKind::ExactIntegerRemainder {
            left,
            right,
            obligation,
        }
        | OperationKind::WrappingIntegerDivide {
            left,
            right,
            obligation,
        }
        | OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        }
        | OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        }
        | OperationKind::SaturatingIntegerRemainder {
            left,
            right,
            obligation,
        } => Some((ProofBearingInputs::Binary(*left, *right), *obligation)),
        _ => None,
    }
}

fn require_integer_type(
    tag: OperationSemanticTag,
    scalar_type: ScalarType,
) -> Result<IntegerType, OperationSemanticError> {
    match scalar_type {
        ScalarType::Integer(integer_type) => Ok(integer_type),
        ScalarType::Boolean => Err(OperationSemanticError::OperandShapeMismatch(tag)),
    }
}

fn build_denotation(
    tag: OperationSemanticTag,
    schema: ProofBearingScalarLeafSchema,
    inputs: ProofBearingInputs,
    result_type: IntegerType,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<(ScalarTerm, Option<IntegerType>, Vec<ScalarTerm>), OperationSemanticError> {
    let invalid = || OperationSemanticError::DenotationShapeMismatch(tag);
    let built = match (schema.operands, schema.denotation, inputs) {
        (
            ScalarLeafOperandShape::ExactCastInteger,
            ScalarLeafDenotation::IntegerExactCast,
            ProofBearingInputs::Unary(operand),
        ) => {
            let operand = value_term(operand, value_types)?;
            let source_type = integer_type(&operand).ok_or_else(invalid)?;
            let denotation =
                ScalarTerm::integer_exact_cast(source_type, result_type, operand.clone())
                    .map_err(OperationSemanticError::InvalidProposition)?;
            (denotation, Some(source_type), vec![operand])
        }
        (
            ScalarLeafOperandShape::IntegerShift,
            ScalarLeafDenotation::ExactIntegerShiftLeft,
            ProofBearingInputs::Binary(value, count),
        )
        | (
            ScalarLeafOperandShape::IntegerShift,
            ScalarLeafDenotation::ExactIntegerShiftRight,
            ProofBearingInputs::Binary(value, count),
        ) => {
            let value = value_term(value, value_types)?;
            let count = value_term(count, value_types)?;
            if integer_type(&value) != Some(result_type) {
                return Err(OperationSemanticError::OperandShapeMismatch(tag));
            }
            let count_type = integer_type(&count).ok_or_else(invalid)?;
            let denotation = match schema.denotation {
                ScalarLeafDenotation::ExactIntegerShiftLeft => {
                    ScalarTerm::exact_integer_shift_left(
                        result_type,
                        count_type,
                        value.clone(),
                        count.clone(),
                    )
                }
                ScalarLeafDenotation::ExactIntegerShiftRight => {
                    ScalarTerm::exact_integer_shift_right(
                        result_type,
                        count_type,
                        value.clone(),
                        count.clone(),
                    )
                }
                _ => unreachable!("matched exact-shift denotation"),
            }
            .map_err(OperationSemanticError::InvalidProposition)?;
            (denotation, Some(count_type), vec![value, count])
        }
        (
            ScalarLeafOperandShape::BinaryInteger,
            denotation,
            ProofBearingInputs::Binary(left, right),
        ) => {
            let left = value_term(left, value_types)?;
            let right = value_term(right, value_types)?;
            if integer_type(&left) != Some(result_type) || integer_type(&right) != Some(result_type)
            {
                return Err(OperationSemanticError::OperandShapeMismatch(tag));
            }
            let built = match denotation {
                ScalarLeafDenotation::ExactIntegerAdd => {
                    ScalarTerm::exact_integer_add(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::ExactIntegerSubtract => {
                    ScalarTerm::exact_integer_subtract(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::ExactIntegerMultiply => {
                    ScalarTerm::exact_integer_multiply(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::ExactIntegerDivide => {
                    ScalarTerm::exact_integer_divide(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::ExactIntegerRemainder => {
                    ScalarTerm::exact_integer_remainder(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::WrappingIntegerDivide => {
                    ScalarTerm::wrapping_integer_divide(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::WrappingIntegerRemainder => {
                    ScalarTerm::wrapping_integer_remainder(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::SaturatingIntegerDivide => {
                    ScalarTerm::saturating_integer_divide(result_type, left.clone(), right.clone())
                }
                ScalarLeafDenotation::SaturatingIntegerRemainder => {
                    ScalarTerm::saturating_integer_remainder(
                        result_type,
                        left.clone(),
                        right.clone(),
                    )
                }
                _ => return Err(invalid()),
            }
            .map_err(OperationSemanticError::InvalidProposition)?;
            (built, None, vec![left, right])
        }
        _ => return Err(OperationSemanticError::OperandShapeMismatch(tag)),
    };
    Ok(built)
}

fn canonical_goal(
    tag: OperationSemanticTag,
    schema: ProofBearingScalarLeafSchema,
    result_type: IntegerType,
    auxiliary_type: Option<IntegerType>,
    inputs: &[ScalarTerm],
    denotation: ScalarTerm,
) -> Result<CanonicalScalarGoal, OperationSemanticError> {
    let invalid = || OperationSemanticError::ProofBearingScalarSchemaMismatch(tag);
    match (schema.goal, inputs) {
        (ScalarLeafGoalShape::ExactCastRepresentable, [operand]) => {
            Ok(CanonicalScalarGoal::ExactCastRepresentable {
                source_type: auxiliary_type.ok_or_else(invalid)?,
                target_type: result_type,
                operand: operand.clone(),
            })
        }
        (ScalarLeafGoalShape::ExactShiftCount, [_, count]) => {
            Ok(CanonicalScalarGoal::ExactShiftCount {
                value_type: result_type,
                count_type: auxiliary_type.ok_or_else(invalid)?,
                count: count.clone(),
            })
        }
        (ScalarLeafGoalShape::ExactShiftLeftRepresentable, [value, count]) => {
            Ok(CanonicalScalarGoal::ExactShiftLeftRepresentable {
                value_type: result_type,
                count_type: auxiliary_type.ok_or_else(invalid)?,
                value: value.clone(),
                count: count.clone(),
            })
        }
        (ScalarLeafGoalShape::ExactArithmeticRepresentable, [_, _]) => {
            Ok(CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: result_type,
                expression: denotation,
            })
        }
        (ScalarLeafGoalShape::ExactDivisionDefined, [left, right]) => {
            Ok(CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: result_type,
                left: left.clone(),
                right: right.clone(),
            })
        }
        (ScalarLeafGoalShape::NonzeroDivisor, [_, divisor]) => {
            Ok(CanonicalScalarGoal::NonzeroDivisor {
                integer_type: result_type,
                divisor: divisor.clone(),
            })
        }
        _ => Err(invalid()),
    }
}

/// Interpret one proof-bearing scalar leaf through its exact declarative row.
/// The returned canonical goal is not a sufficient preimage and is not itself
/// a kernel certificate; it is the stable goal a future untrusted reducer must
/// prove. The result equation becomes available only after that goal is
/// discharged on the operation's normal successor.
pub fn proof_bearing_scalar_leaf_semantics(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Option<ProofBearingScalarLeafSemantics>, OperationSemanticError> {
    let Some(row) = proof_bearing_scalar_semantic_row(&operation.kind)? else {
        return Ok(None);
    };
    let result = operation
        .result
        .scalar_ref()
        .ok_or(OperationSemanticError::MissingScalarResult(row.tag))?;
    let result_type = require_integer_type(row.tag, result.scalar_type)?;
    let (inputs, obligation) = proof_bearing_inputs(&operation.kind)
        .ok_or(OperationSemanticError::OperandShapeMismatch(row.tag))?;
    let (denotation, auxiliary_type, inputs) =
        build_denotation(row.tag, row.schema, inputs, result_type, value_types)?;
    let canonical_goal = canonical_goal(
        row.tag,
        row.schema,
        result_type,
        auxiliary_type,
        &inputs,
        denotation.clone(),
    )?;
    if canonical_goal.shape() != row.schema.goal
        || row.schema.fact != ScalarLeafFactShape::ResultEquation
        || row.schema.crash != ScalarLeafCrashPolicy::CrashesUnlessGoal
        || row.schema.fuel != ScalarLeafFuelPolicy::ConsumeOne
        || row.schema.frontier != ScalarLeafFrontierPolicy::PreserveLocal
    {
        return Err(OperationSemanticError::ProofBearingScalarSchemaMismatch(
            row.tag,
        ));
    }
    Ok(Some(ProofBearingScalarLeafSemantics {
        tag: row.tag,
        schema: row.schema,
        obligation,
        canonical_goal,
        denotation: denotation.clone(),
        result_equation: Proposition::Equal(
            ScalarTerm::value(result.id, result.scalar_type),
            denotation,
        ),
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use psi_core::{IntegerSign, OperationId, ScalarType, ValueId};
    use psi_terminal::{Operation, OperationKind, OperationResult, ValueDeclaration};

    use super::*;

    fn integer_type(bits: u16) -> IntegerType {
        IntegerType::new(IntegerSign::Signed, bits).expect("fixed signed integer type")
    }

    fn obligation(value: u64) -> ObligationId {
        ObligationId::new(value).expect("nonzero obligation identity")
    }

    fn operation(tag: OperationSemanticTag) -> Operation {
        let unary = ValueId::new(1).unwrap();
        let left = ValueId::new(2).unwrap();
        let right = ValueId::new(3).unwrap();
        let count = ValueId::new(4).unwrap();
        let kind = match tag {
            OperationSemanticTag::IntegerExactCast => OperationKind::IntegerExactCast {
                operand: unary,
                obligation: obligation(1),
            },
            OperationSemanticTag::ExactIntegerShiftLeft => OperationKind::ExactIntegerShiftLeft {
                value: left,
                count,
                obligation: obligation(2),
            },
            OperationSemanticTag::ExactIntegerShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count,
                obligation: obligation(3),
            },
            OperationSemanticTag::ExactIntegerAdd => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: obligation(4),
            },
            OperationSemanticTag::ExactIntegerSubtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: obligation(5),
            },
            OperationSemanticTag::ExactIntegerMultiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: obligation(6),
            },
            OperationSemanticTag::ExactIntegerDivide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: obligation(7),
            },
            OperationSemanticTag::ExactIntegerRemainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: obligation(8),
            },
            OperationSemanticTag::WrappingIntegerDivide => OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation: obligation(9),
            },
            OperationSemanticTag::WrappingIntegerRemainder => {
                OperationKind::WrappingIntegerRemainder {
                    left,
                    right,
                    obligation: obligation(10),
                }
            }
            OperationSemanticTag::SaturatingIntegerDivide => {
                OperationKind::SaturatingIntegerDivide {
                    left,
                    right,
                    obligation: obligation(11),
                }
            }
            OperationSemanticTag::SaturatingIntegerRemainder => {
                OperationKind::SaturatingIntegerRemainder {
                    left,
                    right,
                    obligation: obligation(12),
                }
            }
            _ => panic!("tag is not proof-bearing scalar"),
        };
        Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(10).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(8)),
            }),
            kind,
        }
    }

    fn value_types() -> BTreeMap<ValueId, ScalarType> {
        BTreeMap::from([
            (
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type(16)),
            ),
            (
                ValueId::new(2).unwrap(),
                ScalarType::Integer(integer_type(8)),
            ),
            (
                ValueId::new(3).unwrap(),
                ScalarType::Integer(integer_type(8)),
            ),
            (
                ValueId::new(4).unwrap(),
                ScalarType::Integer(integer_type(16)),
            ),
        ])
    }

    #[test]
    fn proof_bearing_scalar_rows_are_exact_unique_and_cover_six_goal_shapes() {
        validate_proof_bearing_scalar_semantic_rows(&ProofBearingScalarSemanticRow::ALL).unwrap();
        assert_eq!(ProofBearingScalarSemanticRow::ALL.len(), 12);
        assert_eq!(
            ProofBearingScalarSemanticRow::ALL
                .iter()
                .map(|row| row.tag)
                .collect::<BTreeSet<_>>()
                .len(),
            12,
        );
        assert_eq!(
            ProofBearingScalarSemanticRow::ALL
                .iter()
                .map(|row| row.schema.goal)
                .collect::<BTreeSet<_>>()
                .len(),
            6,
        );
        assert!(ProofBearingScalarSemanticRow::ALL.iter().all(|row| {
            row.schema.result == ScalarLeafResultShape::DeclaredInteger
                && row.schema.fact == ScalarLeafFactShape::ResultEquation
                && row.schema.crash == ScalarLeafCrashPolicy::CrashesUnlessGoal
                && row.schema.fuel == ScalarLeafFuelPolicy::ConsumeOne
                && row.schema.frontier == ScalarLeafFrontierPolicy::PreserveLocal
        }));
    }

    #[test]
    fn every_proof_bearing_row_emits_one_typed_goal_and_normal_result_equation() {
        for row in ProofBearingScalarSemanticRow::ALL {
            let semantics =
                proof_bearing_scalar_leaf_semantics(&operation(row.tag), &value_types())
                    .unwrap()
                    .expect("proof-bearing row emits semantics");
            assert_eq!(semantics.tag(), row.tag);
            assert_eq!(semantics.schema(), row.schema);
            assert_eq!(semantics.canonical_goal().shape(), row.schema.goal);
            assert!(semantics.obligation().get() > 0);
            assert!(matches!(
                semantics.result_equation(),
                Proposition::Equal(ScalarTerm::Value { .. }, _)
            ));
            semantics.result_equation().validate().unwrap();
        }
        let goal_free = Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(10).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(8)),
            }),
            kind: OperationKind::WrappingIntegerAdd {
                left: ValueId::new(2).unwrap(),
                right: ValueId::new(3).unwrap(),
            },
        };
        assert_eq!(
            proof_bearing_scalar_leaf_semantics(&goal_free, &value_types()),
            Ok(None),
        );
    }

    #[test]
    fn proof_bearing_row_table_rejects_missing_duplicate_cross_kind_and_drift() {
        let tag = OperationSemanticTag::ExactIntegerAdd;
        let canonical =
            *exact_proof_bearing_scalar_semantic_row_in(tag, &ProofBearingScalarSemanticRow::ALL)
                .unwrap()
                .unwrap();
        let missing = ProofBearingScalarSemanticRow::ALL
            .iter()
            .copied()
            .filter(|row| row.tag != tag)
            .collect::<Vec<_>>();
        assert_eq!(
            validate_proof_bearing_scalar_semantic_rows(&missing),
            Err(OperationSemanticError::MissingProofBearingScalarRow(tag)),
        );
        let mut duplicate = ProofBearingScalarSemanticRow::ALL.to_vec();
        duplicate.push(canonical);
        assert_eq!(
            validate_proof_bearing_scalar_semantic_rows(&duplicate),
            Err(OperationSemanticError::DuplicateProofBearingScalarRow(tag)),
        );
        let mut cross_kind = ProofBearingScalarSemanticRow::ALL.to_vec();
        cross_kind[0].tag = OperationSemanticTag::WrappingIntegerAdd;
        assert_eq!(
            validate_proof_bearing_scalar_semantic_rows(&cross_kind),
            Err(OperationSemanticError::UnexpectedProofBearingScalarRow(
                OperationSemanticTag::WrappingIntegerAdd,
            )),
        );
        let mut drift = ProofBearingScalarSemanticRow::ALL.to_vec();
        let row = drift.iter_mut().find(|row| row.tag == tag).unwrap();
        row.schema.goal = ScalarLeafGoalShape::NonzeroDivisor;
        assert_eq!(
            validate_proof_bearing_scalar_semantic_rows(&drift),
            Err(OperationSemanticError::ProofBearingScalarSchemaMismatch(
                tag
            )),
        );
    }

    #[test]
    fn proof_bearing_semantics_fail_closed_on_operand_type_drift() {
        let mut types = value_types();
        types.insert(ValueId::new(3).unwrap(), ScalarType::Boolean);
        assert_eq!(
            proof_bearing_scalar_leaf_semantics(
                &operation(OperationSemanticTag::ExactIntegerAdd),
                &types,
            ),
            Err(OperationSemanticError::OperandShapeMismatch(
                OperationSemanticTag::ExactIntegerAdd,
            )),
        );
    }

    #[test]
    fn nonzero_divisor_projects_the_exact_fixed_carrier_proposition() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_divisor = ScalarTerm::value(
            ValueId::new(20).expect("signed divisor"),
            ScalarType::Integer(signed),
        );
        let negative_one = ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("-1i8");
        let positive_one = ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("1i8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: signed,
                divisor: signed_divisor.clone(),
            }
            .kernel_proposition(),
            Ok(Some(Proposition::Disjunction(vec![
                Proposition::LessOrEqual(signed_divisor.clone(), negative_one),
                Proposition::LessOrEqual(positive_one, signed_divisor),
            ]))),
        );

        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_divisor = ScalarTerm::value(
            ValueId::new(21).expect("unsigned divisor"),
            ScalarType::Integer(unsigned),
        );
        let one = ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("1u8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: unsigned,
                divisor: unsigned_divisor.clone(),
            }
            .kernel_proposition(),
            Ok(Some(Proposition::LessOrEqual(one, unsigned_divisor))),
        );
    }

    #[test]
    fn nonzero_divisor_handles_i1_and_rejects_invalid_carrier_identity() {
        let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let divisor = ScalarTerm::value(
            ValueId::new(22).expect("i1 divisor"),
            ScalarType::Integer(i1),
        );
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: i1,
                divisor: divisor.clone(),
            }
            .kernel_proposition(),
            Ok(Some(Proposition::LessOrEqual(
                divisor,
                ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("-1i1"),
            ))),
        );

        let address = IntegerType::address(64).expect("address64");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: address,
                divisor: ScalarTerm::value(
                    ValueId::new(23).expect("address divisor"),
                    ScalarType::Integer(address),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::NonzeroDivisorRequiresFixedInteger(
                address,
            )),
        );

        let i8 = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: i8,
                divisor: ScalarTerm::boolean(true),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::NonzeroDivisorTypeMismatch {
                declared: i8,
                actual: ScalarType::Boolean,
            }),
        );

        let u8 = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        assert_eq!(
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: i8,
                divisor: ScalarTerm::value(
                    ValueId::new(25).expect("wrong-carrier divisor"),
                    ScalarType::Integer(u8),
                ),
            }
            .kernel_proposition(),
            Err(OperationSemanticError::NonzeroDivisorTypeMismatch {
                declared: i8,
                actual: ScalarType::Integer(u8),
            }),
        );
    }

    #[test]
    fn unsettled_canonical_goal_shapes_do_not_project_to_kernel_propositions() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let value = ScalarTerm::value(
            ValueId::new(24).expect("value"),
            ScalarType::Integer(integer),
        );
        let goals = [
            CanonicalScalarGoal::ExactCastRepresentable {
                source_type: integer,
                target_type: integer,
                operand: value.clone(),
            },
            CanonicalScalarGoal::ExactShiftCount {
                value_type: integer,
                count_type: integer,
                count: value.clone(),
            },
            CanonicalScalarGoal::ExactShiftLeftRepresentable {
                value_type: integer,
                count_type: integer,
                value: value.clone(),
                count: value.clone(),
            },
            CanonicalScalarGoal::ExactArithmeticRepresentable {
                integer_type: integer,
                expression: value.clone(),
            },
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: integer,
                left: value.clone(),
                right: value,
            },
        ];
        assert!(
            goals
                .iter()
                .all(|goal| goal.kernel_proposition() == Ok(None))
        );
    }
}
