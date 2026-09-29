//! Pins the canonical bytes of every admitted integer policy pair.
//!
//! The expected strings were captured from the per-pair variants that
//! `IntegerShift` and `IntegerArithmetic` replaced, so a changed tag,
//! obligation byte or field order fails here before it moves any unit
//! identity.

use super::{AbstractOperation, CanonicalBytes, encode_operation};
use abstract_operations::{
    IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection,
    IntegerShiftPolicy,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, OperationId, ValueId};

const PSI_OPERATION: u64 = 0x11;
const OBLIGATION: u64 = 0x22;
const RESULT: u64 = 0x33;
const LEFT: u64 = 0x44;
const RIGHT: u64 = 0x55;

fn encoded_hex(operation: &AbstractOperation) -> String {
    let mut bytes = CanonicalBytes::collect();
    encode_operation(&mut bytes, operation);
    bytes
        .finish()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn scalar_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32")
}

#[test]
fn every_shift_pair_encodes_its_former_variant_bytes() {
    let obligation = ObligationId::new(OBLIGATION).expect("nonzero obligation");
    let exact = IntegerShiftPolicy::Exact { obligation };
    let cases = [
        (
            IntegerShiftPolicy::Wrapping,
            IntegerShiftDirection::Left,
            "17110000000000000000330000000000000002200001080044000000000000005500000000000000",
        ),
        (
            IntegerShiftPolicy::Wrapping,
            IntegerShiftDirection::Right,
            "18110000000000000000330000000000000002200001080044000000000000005500000000000000",
        ),
        (
            exact,
            IntegerShiftDirection::Left,
            "191100000000000000012200000000000000330000000000000002200001080044000000000000005500000000000000",
        ),
        (
            exact,
            IntegerShiftDirection::Right,
            "1a1100000000000000012200000000000000330000000000000002200001080044000000000000005500000000000000",
        ),
    ];
    for (policy, direction, expected) in cases {
        let operation = AbstractOperation::IntegerShift {
            psi_operation: OperationId::new(PSI_OPERATION).expect("nonzero operation"),
            result: ValueId::new(RESULT).expect("nonzero result"),
            value_type: scalar_type(),
            count_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            policy,
            direction,
            value: ValueId::new(LEFT).expect("nonzero value"),
            count: ValueId::new(RIGHT).expect("nonzero count"),
        };
        assert_eq!(
            encoded_hex(&operation),
            expected,
            "{policy:?} {direction:?}"
        );
    }
}

#[test]
fn every_arithmetic_pair_encodes_its_former_variant_bytes() {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    let unobligated = "110000000000000000330000000000000002200044000000000000005500000000000000";
    let obligated =
        "1100000000000000012200000000000000330000000000000002200044000000000000005500000000000000";
    let cases = [
        (Policy::Wrapping, Operator::Add, "1b"),
        (Policy::Exact, Operator::Add, "1c"),
        (Policy::Saturating, Operator::Add, "1d"),
        (Policy::Wrapping, Operator::Subtract, "1e"),
        (Policy::Exact, Operator::Subtract, "1f"),
        (Policy::Saturating, Operator::Subtract, "20"),
        (Policy::Wrapping, Operator::Multiply, "21"),
        (Policy::Exact, Operator::Multiply, "22"),
        (Policy::Exact, Operator::Divide, "23"),
        (Policy::Exact, Operator::Remainder, "24"),
        (Policy::Wrapping, Operator::Divide, "25"),
        (Policy::Wrapping, Operator::Remainder, "26"),
        (Policy::Saturating, Operator::Divide, "27"),
        (Policy::Saturating, Operator::Remainder, "28"),
        (Policy::Saturating, Operator::Multiply, "29"),
    ];
    for (policy, operator, tag) in cases {
        let requires_obligation = IntegerArithmetic::requires_obligation(policy, operator);
        let obligation =
            requires_obligation.then(|| ObligationId::new(OBLIGATION).expect("nonzero obligation"));
        let operation = AbstractOperation::IntegerArithmetic {
            psi_operation: OperationId::new(PSI_OPERATION).expect("nonzero operation"),
            result: ValueId::new(RESULT).expect("nonzero result"),
            scalar_type: scalar_type(),
            arithmetic: IntegerArithmetic::new(policy, operator, obligation)
                .expect("admitted integer arithmetic pair"),
            left: ValueId::new(LEFT).expect("nonzero left"),
            right: ValueId::new(RIGHT).expect("nonzero right"),
        };
        let fields = if requires_obligation {
            obligated
        } else {
            unobligated
        };
        assert_eq!(
            encoded_hex(&operation),
            format!("{tag}{fields}"),
            "{policy:?} {operator:?}"
        );
    }
}
