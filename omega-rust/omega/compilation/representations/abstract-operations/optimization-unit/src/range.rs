/*
We represent revision-bound integer value-range facts (`ValueRangeFact`), extract
closed conjunctive integer intervals from canonical propositions
(`extract_integer_intervals`), and map proof-bearing shift and division operations
to their `CanonicalScalarGoal` (`proof_range_goal`). Every range fact binds its
supporting constant or accepted obligation proof and its dominated CFG scope into
a canonical `ValueRangeFactIdentity`.
*/

use super::{
    AcceptedObligationFactIdentity, BlockId, IntegerCarrier, IntegerType, IntegerValue,
    OperationId, ProofQuestionIdentity, ScalarConstantFactIdentity, ValueId,
    ValueRangeFactIdentity,
};
use super::{MachineId, OptimizationUnitIdentity};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueRangeSupport {
    ScalarConstant(ScalarConstantFactIdentity),
    AcceptedOperationProof {
        accepted: AcceptedObligationFactIdentity,
        question: ProofQuestionIdentity,
        operation: OperationId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueRangeScope {
    EntireValue,
    DominatedOperationEntry {
        block: BlockId,
        node: u32,
        operation: OperationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeRegion {
    pub revision: OptimizationUnitIdentity,
    pub machine: MachineId,
    pub value: ValueId,
    pub scope: ValueRangeScope,
    pub dominated_blocks: Vec<BlockId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeFact {
    pub identity: ValueRangeFactIdentity,
    pub value: ValueId,
    pub scalar_type: IntegerType,
    pub minimum: IntegerValue,
    pub maximum: IntegerValue,
    pub support: ValueRangeSupport,
    pub valid_in: ValueRangeRegion,
}

pub fn value_range_fact_identity(
    value: ValueId,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    support: &ValueRangeSupport,
    valid_in: &ValueRangeRegion,
) -> Option<ValueRangeFactIdentity> {
    if valid_in.value != value
        || scalar_type.carrier() != IntegerCarrier::Fixed
        || !scalar_type.admits(minimum)
        || !scalar_type.admits(maximum)
        || integer_value_cmp(scalar_type, minimum, maximum).is_none_or(|order| order.is_gt())
        || match (support, valid_in.scope) {
            (ValueRangeSupport::ScalarConstant(_), ValueRangeScope::EntireValue) => {
                !valid_in.dominated_blocks.is_empty()
            }
            (
                ValueRangeSupport::AcceptedOperationProof { operation, .. },
                ValueRangeScope::DominatedOperationEntry {
                    block,
                    operation: scope_operation,
                    ..
                },
            ) => {
                *operation != scope_operation
                    || valid_in.dominated_blocks.is_empty()
                    || valid_in
                        .dominated_blocks
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || valid_in.dominated_blocks.binary_search(&block).is_err()
            }
            _ => true,
        }
    {
        return None;
    }

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-value-range-fact.v1\0");
    canonical.extend_from_slice(&valid_in.revision.bytes());
    crate::codec_bytes::push_encoded(&mut canonical, valid_in.machine.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, valid_in.value.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, value.get().to_le_bytes());
    encode_integer_type(&mut canonical, scalar_type);
    encode_integer_value(&mut canonical, minimum);
    encode_integer_value(&mut canonical, maximum);
    match support {
        ValueRangeSupport::ScalarConstant(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        ValueRangeSupport::AcceptedOperationProof {
            accepted,
            question,
            operation,
        } => {
            canonical.push(2);
            canonical.extend_from_slice(&accepted.bytes());
            canonical.extend_from_slice(&question.bytes());
            crate::codec_bytes::push_encoded(&mut canonical, operation.get().to_le_bytes());
        }
    }
    match valid_in.scope {
        ValueRangeScope::EntireValue => canonical.push(1),
        ValueRangeScope::DominatedOperationEntry {
            block,
            node,
            operation,
        } => {
            canonical.push(2);
            crate::codec_bytes::push_encoded(&mut canonical, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(&mut canonical, node.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut canonical, operation.get().to_le_bytes());
        }
    }
    canonical.extend_from_slice(
        &u64::try_from(valid_in.dominated_blocks.len())
            .expect("canonical dominated-block count fits u64")
            .to_le_bytes(),
    );
    for block in &valid_in.dominated_blocks {
        crate::codec_bytes::push_encoded(&mut canonical, block.get().to_le_bytes());
    }
    Some(ValueRangeFactIdentity::from_canonical_bytes(&canonical))
}

use abstract_operations::{
    AbstractOperation as O, IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{IntegerSign, Proposition, ScalarTerm, ScalarType};
use std::collections::BTreeMap;
use terminal_semantics::CanonicalScalarGoal;

#[derive(Debug, Clone, Copy, Default)]
pub struct PartialIntegerBounds {
    pub lower: Option<IntegerValue>,
    pub upper: Option<IntegerValue>,
}

pub enum IntervalExtraction {
    Unsupported,
    Contradiction,
    Bounds(BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>),
}

pub fn extract_integer_intervals(proposition: &Proposition) -> IntervalExtraction {
    if proposition.validate().is_err() {
        return IntervalExtraction::Unsupported;
    }
    let mut bounds = BTreeMap::new();
    if !extract_integer_interval_conjunct(proposition, &mut bounds) {
        return IntervalExtraction::Contradiction;
    }
    for ((_, scalar_type), interval) in &bounds {
        let minimum = interval
            .lower
            .unwrap_or_else(|| scalar_type.minimum_value());
        let maximum = interval
            .upper
            .unwrap_or_else(|| scalar_type.maximum_value());
        if integer_value_cmp(*scalar_type, minimum, maximum).is_none_or(|order| order.is_gt()) {
            return IntervalExtraction::Contradiction;
        }
    }
    if bounds.is_empty() {
        IntervalExtraction::Unsupported
    } else {
        IntervalExtraction::Bounds(bounds)
    }
}

fn extract_integer_interval_conjunct(
    proposition: &Proposition,
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
) -> bool {
    match proposition {
        Proposition::Truth => true,
        Proposition::Falsehood => false,
        Proposition::Conjunction(conjuncts) => conjuncts
            .iter()
            .all(|conjunct| extract_integer_interval_conjunct(conjunct, bounds)),
        Proposition::Equal(left, right) => {
            let Some((value, scalar_type, literal)) =
                value_and_literal(left, right).or_else(|| value_and_literal(right, left))
            else {
                return true;
            };
            merge_lower(bounds, value, scalar_type, literal)
                && merge_upper(bounds, value, scalar_type, literal)
        }
        Proposition::LessOrEqual(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                merge_upper(bounds, value, scalar_type, literal)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                merge_lower(bounds, value, scalar_type, literal)
            } else {
                true
            }
        }
        Proposition::LessThan(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                let Some(upper) = predecessor(scalar_type, literal) else {
                    return false;
                };
                merge_upper(bounds, value, scalar_type, upper)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                let Some(lower) = successor(scalar_type, literal) else {
                    return false;
                };
                merge_lower(bounds, value, scalar_type, lower)
            } else {
                true
            }
        }
        Proposition::Atom(_)
        | Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _)
        | Proposition::IeeeFloatComparison { .. }
        | Proposition::ScalarIeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralIdentity { .. }
        | Proposition::StructuralCaseMembership { .. }
        | Proposition::Disjunction(_)
        | Proposition::Implication { .. }
        | Proposition::ContentConservation(_) => true,
    }
}

fn value_and_literal(
    value: &ScalarTerm,
    literal: &ScalarTerm,
) -> Option<(ValueId, IntegerType, IntegerValue)> {
    let ScalarTerm::Value {
        id,
        scalar_type: ScalarType::Integer(value_type),
    } = value
    else {
        return None;
    };
    let ScalarTerm::Integer {
        scalar_type: literal_type,
        value,
    } = literal
    else {
        return None;
    };
    (*value_type == *literal_type
        && value_type.carrier() == IntegerCarrier::Fixed
        && value_type.admits(*value))
    .then_some((*id, *value_type, *value))
}

fn merge_lower(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    lower: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.lower = match interval.lower {
        None => Some(lower),
        Some(current) => match integer_value_cmp(scalar_type, current, lower) {
            Some(order) if order.is_lt() => Some(lower),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

fn merge_upper(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    upper: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.upper = match interval.upper {
        None => Some(upper),
        Some(current) => match integer_value_cmp(scalar_type, current, upper) {
            Some(order) if order.is_gt() => Some(upper),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

pub(crate) fn integer_value_cmp(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<std::cmp::Ordering> {
    if !scalar_type.admits(left) || !scalar_type.admits(right) {
        return None;
    }
    match (scalar_type.sign(), left, right) {
        (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            Some(left.cmp(&right))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            Some(left.cmp(&right))
        }
        _ => None,
    }
}

fn predecessor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = scalar_type.one();
    scalar_type.exact_sub(value, one)
}

fn successor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = scalar_type.one();
    scalar_type.exact_add(value, one)
}

pub fn proof_range_goal(
    operation: &O,
) -> Option<(
    OperationId,
    semantic_vocabulary::ObligationId,
    CanonicalScalarGoal,
)> {
    let value_term = |id, scalar_type| ScalarTerm::value(id, ScalarType::Integer(scalar_type));
    match operation {
        O::IntegerShift {
            psi_operation,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { obligation },
            direction: IntegerShiftDirection::Right,
            count,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactShiftCount {
                value_type: *value_type,
                count_type: *count_type,
                count: value_term(*count, *count_type),
            },
        )),
        O::IntegerArithmetic {
            psi_operation,
            scalar_type,
            arithmetic,
            left,
            right,
            ..
        } if arithmetic.operator().is_division() => {
            let obligation = arithmetic.obligation()?;
            let goal = match arithmetic.policy() {
                IntegerOverflowPolicy::Exact => CanonicalScalarGoal::ExactDivisionDefined {
                    integer_type: *scalar_type,
                    left: value_term(*left, *scalar_type),
                    right: value_term(*right, *scalar_type),
                },
                IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating => {
                    CanonicalScalarGoal::NonzeroDivisor {
                        integer_type: *scalar_type,
                        divisor: value_term(*right, *scalar_type),
                    }
                }
            };
            Some((*psi_operation, obligation, goal))
        }
        _ => None,
    }
}

pub(crate) fn encode_integer_type(canonical: &mut Vec<u8>, scalar_type: IntegerType) {
    canonical.push(match scalar_type.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    canonical.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    crate::codec_bytes::push_encoded(canonical, scalar_type.bits().to_le_bytes());
}

pub(crate) fn encode_integer_value(canonical: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            canonical.push(1);
            crate::codec_bytes::push_encoded(canonical, value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            canonical.push(2);
            crate::codec_bytes::push_encoded(canonical, value.to_le_bytes());
        }
    }
}
