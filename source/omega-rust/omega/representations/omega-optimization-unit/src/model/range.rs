//! Revision-bound integer range carriers and canonical identities.

use super::*;

/// Exact authority consumed by one derived current-revision integer range.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueRangeSupport {
    ScalarConstant(ScalarConstantFactIdentity),
    AcceptedOperationProof {
        accepted: AcceptedObligationFactIdentity,
        question: ProofQuestionIdentity,
        operation: OperationId,
    },
}

/// A range is either valid wherever its SSA value is available, or only from
/// one verified operation entry through the operation's dominated region.
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
    /// Canonical current-CFG blocks dominated by the proof owner. Empty for
    /// an entire-value scalar fact.
    pub dominated_blocks: Vec<BlockId>,
}

/// One identity-bound interval derived from current scalar or proof custody.
/// This carrier is not stored in [`PsiOptimizationUnit`]; analyses recompute it
/// for each revision and independent validators reconstruct it on demand.
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
    canonical.extend_from_slice(&valid_in.machine.get().to_le_bytes());
    canonical.extend_from_slice(&valid_in.value.get().to_le_bytes());
    canonical.extend_from_slice(&value.get().to_le_bytes());
    encode_range_integer_type(&mut canonical, scalar_type);
    encode_range_integer_value(&mut canonical, minimum);
    encode_range_integer_value(&mut canonical, maximum);
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
            canonical.extend_from_slice(&operation.get().to_le_bytes());
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
            canonical.extend_from_slice(&block.get().to_le_bytes());
            canonical.extend_from_slice(&node.to_le_bytes());
            canonical.extend_from_slice(&operation.get().to_le_bytes());
        }
    }
    canonical.extend_from_slice(
        &u64::try_from(valid_in.dominated_blocks.len())
            .expect("canonical dominated-block count fits u64")
            .to_le_bytes(),
    );
    for block in &valid_in.dominated_blocks {
        canonical.extend_from_slice(&block.get().to_le_bytes());
    }
    Some(ValueRangeFactIdentity::from_canonical_bytes(&canonical))
}

fn integer_value_cmp(
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

fn encode_range_integer_type(canonical: &mut Vec<u8>, scalar_type: IntegerType) {
    canonical.push(match scalar_type.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    canonical.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    canonical.extend_from_slice(&scalar_type.bits().to_le_bytes());
}

fn encode_range_integer_value(canonical: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            canonical.push(1);
            canonical.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            canonical.push(2);
            canonical.extend_from_slice(&value.to_le_bytes());
        }
    }
}
