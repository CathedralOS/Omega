//! IEEE operations retain raw format bits, relation, and ordered operand identity.
//! Tags and field order are unchanged when routing this family from the entrance.

use super::{AbstractOperation, CanonicalBytes};
use semantic_vocabulary::IeeeFloatFormat;

/// The raw format tag every IEEE operation carries: 0 for binary32, 1 for
/// binary64. Proposition encoding numbers the same formats 1 and 2.
fn encode_operation_format(bytes: &mut CanonicalBytes, format: &IeeeFloatFormat) {
    bytes.u8(match format {
        IeeeFloatFormat::Binary32 => 0,
        IeeeFloatFormat::Binary64 => 1,
    });
}

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::FloatResultPolicy {
            psi_operation,
            result,
            format,
            policy,
        } => {
            bytes.u8(97);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_operation_format(bytes, format);
            match policy {
                terminal_psi::FloatResultPolicy::Trapping { .. } => bytes.u8(0),
                terminal_psi::FloatResultPolicy::Saturating { .. } => bytes.u8(1),
            };
            for operand in policy.operands() {
                bytes.id(operand);
            }
        }
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
            encode_operation_format(bytes, format);
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
            encode_operation_format(bytes, format);
            bytes.id(*left);
            bytes.id(*right);
            bytes.id(*addend);
        }
        _ => unreachable!("operation family routing admitted a non-IEEE operation"),
    }
}
