//! IEEE operations retain raw format bits, relation, and ordered operand identity.
//! Tags and field order are unchanged when routing this family from the entrance.

use super::{AbstractOperation, CanonicalBytes};
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
            bytes.u8(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
            bytes.u8(match policy {
                terminal_psi::FloatResultPolicy::Trapping { .. } => 0,
                terminal_psi::FloatResultPolicy::Saturating {
                    rule: terminal_psi::FloatSaturationRule::FiniteOperandOverflow,
                    ..
                } => 1,
                terminal_psi::FloatResultPolicy::Saturating {
                    rule: terminal_psi::FloatSaturationRule::FiniteDivisionOverflow,
                    ..
                } => 2,
            });
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IeeeFloatFormat, OperationId, ValueId};
    use terminal_psi::{FloatResultPolicy, FloatSaturationRule};

    fn encoded(operation: &AbstractOperation) -> Vec<u8> {
        let mut bytes = CanonicalBytes::collect();
        super::super::encode_operation(&mut bytes, operation);
        bytes.finish()
    }

    #[test]
    fn float_result_policy_identity_binds_format_rule_and_ordered_operands() {
        let value = |raw| ValueId::new(raw).expect("nonzero value");
        let original = AbstractOperation::FloatResultPolicy {
            psi_operation: OperationId::new(1).expect("operation"),
            result: value(5),
            format: IeeeFloatFormat::Binary64,
            policy: FloatResultPolicy::Saturating {
                left: value(2),
                right: value(3),
                raw_result: value(4),
                rule: FloatSaturationRule::FiniteDivisionOverflow,
            },
        };
        let original_bytes = encoded(&original);
        assert_eq!(original_bytes[0], 97);
        for mutation in 0..7 {
            let mut candidate = original.clone();
            let AbstractOperation::FloatResultPolicy {
                psi_operation,
                result,
                format,
                policy,
            } = &mut candidate
            else {
                unreachable!()
            };
            match mutation {
                0 => *psi_operation = OperationId::new(9).expect("operation"),
                1 => *result = value(9),
                2 => *format = IeeeFloatFormat::Binary32,
                3 => {
                    *policy = FloatResultPolicy::Trapping {
                        raw_result: value(4),
                    }
                }
                4 => {
                    let FloatResultPolicy::Saturating { rule, .. } = policy else {
                        unreachable!()
                    };
                    *rule = FloatSaturationRule::FiniteOperandOverflow;
                }
                5 => {
                    let FloatResultPolicy::Saturating { left, right, .. } = policy else {
                        unreachable!()
                    };
                    std::mem::swap(left, right);
                }
                6 => {
                    let FloatResultPolicy::Saturating { raw_result, .. } = policy else {
                        unreachable!()
                    };
                    *raw_result = value(9);
                }
                _ => unreachable!(),
            }
            assert_ne!(original_bytes, encoded(&candidate), "mutation {mutation}");
        }
    }
}
