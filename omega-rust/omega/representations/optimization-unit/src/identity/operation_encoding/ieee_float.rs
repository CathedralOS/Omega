//! IEEE operations retain raw format bits, relation, and ordered operand identity.
//! Tags and field order are unchanged when routing this family from the entrance.
use super::*;

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => {
            bytes.u8(42);
            bytes.id(*psi_operation);
            bytes.id(*result);
            match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                    bytes.u8(0);
                    bytes.u32(*bits);
                }
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                    bytes.u8(1);
                    bytes.u64(*bits);
                }
            }
        }
        O::IeeeFloatCompare {
            psi_operation,
            result,
            comparison,
            format,
            left,
            right,
        } => {
            // Tag 70 extends the operation roster without changing old encodings.
            bytes.u8(70);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.u8(match comparison {
                semantic_vocabulary::IeeeFloatComparisonOperation::Equal => 0,
                semantic_vocabulary::IeeeFloatComparisonOperation::NotEqual => 1,
                semantic_vocabulary::IeeeFloatComparisonOperation::Less => 2,
                semantic_vocabulary::IeeeFloatComparisonOperation::LessOrEqual => 3,
                semantic_vocabulary::IeeeFloatComparisonOperation::Greater => 4,
                semantic_vocabulary::IeeeFloatComparisonOperation::GreaterOrEqual => 5,
            });
            bytes.u8(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
            bytes.id(*left);
            bytes.id(*right);
        }
        O::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation,
            result,
            format,
            left,
            right,
            addend,
        } => {
            bytes.u8(43);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.u8(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
            bytes.id(*left);
            bytes.id(*right);
            bytes.id(*addend);
        }
        _ => unreachable!("operation family routing admitted a non-IEEE operation"),
    }
}
