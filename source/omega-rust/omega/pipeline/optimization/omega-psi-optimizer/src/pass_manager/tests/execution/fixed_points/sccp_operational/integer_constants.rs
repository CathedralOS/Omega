//! Operational matrix for SCCP integer-result rows 0--24.

use psi_core::{IntegerSign, IntegerType, IntegerValue};

use super::custody::{Case, assert_operational_custody};
use crate::PsiOptimizationRule;
use crate::rules::tests::{
    BinaryConstantFixtureKind as Binary, UnaryConstantFixtureKind as Unary, binary_constant_unit,
    unary_constant_unit,
};
use crate::{
    ExactIntegerAddConstantsRule, ExactIntegerCastConstantsRule, ExactIntegerDivideConstantsRule,
    ExactIntegerMultiplyConstantsRule, ExactIntegerRemainderConstantsRule,
    ExactIntegerShiftLeftConstantsRule, ExactIntegerShiftRightConstantsRule,
    ExactIntegerSubtractConstantsRule, IntegerBitwiseAndConstantsRule,
    IntegerBitwiseNotConstantsRule, IntegerBitwiseOrConstantsRule, IntegerBitwiseXorConstantsRule,
    IntegerWidenConstantsRule, SaturatingIntegerAddConstantsRule,
    SaturatingIntegerDivideConstantsRule, SaturatingIntegerMultiplyConstantsRule,
    SaturatingIntegerRemainderConstantsRule, SaturatingIntegerSubtractConstantsRule,
    WrappingIntegerAddConstantsRule, WrappingIntegerDivideConstantsRule,
    WrappingIntegerMultiplyConstantsRule, WrappingIntegerRemainderConstantsRule,
    WrappingIntegerShiftLeftConstantsRule, WrappingIntegerShiftRightConstantsRule,
    WrappingIntegerSubtractConstantsRule,
};

fn rule(rule: impl PsiOptimizationRule) -> omega_optimization_core::OptimizationRuleIdentity {
    rule.contract().identity()
}

#[test]
fn every_integer_result_rule_has_whole_engine_operational_custody() {
    let u8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u16 = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let i8 = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let i16 = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    let u = IntegerValue::Unsigned;
    let s = IntegerValue::Signed;

    assert_operational_custody(vec![
        Case::integer(
            0,
            binary_constant_unit(Binary::ExactAdd, u8, u8, u(200), u(55)),
            rule(ExactIntegerAddConstantsRule),
        ),
        Case::integer(
            1,
            binary_constant_unit(Binary::ExactSubtract, u8, u8, u(5), u(5)),
            rule(ExactIntegerSubtractConstantsRule),
        ),
        Case::integer(
            2,
            binary_constant_unit(Binary::ExactMultiply, u8, u8, u(51), u(5)),
            rule(ExactIntegerMultiplyConstantsRule),
        ),
        Case::integer(
            3,
            binary_constant_unit(Binary::WrappingAdd, u8, u8, u(200), u(100)),
            rule(WrappingIntegerAddConstantsRule),
        ),
        Case::integer(
            4,
            binary_constant_unit(Binary::WrappingSubtract, u8, u8, u(5), u(10)),
            rule(WrappingIntegerSubtractConstantsRule),
        ),
        Case::integer(
            5,
            binary_constant_unit(Binary::WrappingMultiply, u8, u8, u(20), u(13)),
            rule(WrappingIntegerMultiplyConstantsRule),
        ),
        Case::integer(
            6,
            binary_constant_unit(Binary::SaturatingAdd, u8, u8, u(200), u(100)),
            rule(SaturatingIntegerAddConstantsRule),
        ),
        Case::integer(
            7,
            binary_constant_unit(Binary::SaturatingSubtract, u8, u8, u(5), u(10)),
            rule(SaturatingIntegerSubtractConstantsRule),
        ),
        Case::integer(
            8,
            binary_constant_unit(Binary::SaturatingMultiply, u8, u8, u(20), u(13)),
            rule(SaturatingIntegerMultiplyConstantsRule),
        ),
        Case::integer(
            9,
            binary_constant_unit(Binary::ExactDivide, i8, i8, s(-127), s(-1)),
            rule(ExactIntegerDivideConstantsRule),
        ),
        Case::integer(
            10,
            binary_constant_unit(Binary::ExactRemainder, i8, i8, s(-127), s(5)),
            rule(ExactIntegerRemainderConstantsRule),
        ),
        Case::integer(
            11,
            binary_constant_unit(Binary::WrappingDivide, i8, i8, s(-128), s(-1)),
            rule(WrappingIntegerDivideConstantsRule),
        ),
        Case::integer(
            12,
            binary_constant_unit(Binary::WrappingRemainder, i8, i8, s(-128), s(-1)),
            rule(WrappingIntegerRemainderConstantsRule),
        ),
        Case::integer(
            13,
            binary_constant_unit(Binary::SaturatingDivide, i8, i8, s(-128), s(-1)),
            rule(SaturatingIntegerDivideConstantsRule),
        ),
        Case::integer(
            14,
            binary_constant_unit(Binary::SaturatingRemainder, i8, i8, s(-128), s(-1)),
            rule(SaturatingIntegerRemainderConstantsRule),
        ),
        Case::integer(
            15,
            binary_constant_unit(Binary::ExactShiftLeft, u8, u16, u(7), u(2)),
            rule(ExactIntegerShiftLeftConstantsRule),
        ),
        Case::integer(
            16,
            binary_constant_unit(Binary::ExactShiftRight, i8, u16, s(-128), u(2)),
            rule(ExactIntegerShiftRightConstantsRule),
        ),
        Case::integer(
            17,
            binary_constant_unit(Binary::WrappingShiftLeft, u8, u16, u(250), u(10)),
            rule(WrappingIntegerShiftLeftConstantsRule),
        ),
        Case::integer(
            18,
            binary_constant_unit(Binary::WrappingShiftRight, i8, u16, s(-8), u(10)),
            rule(WrappingIntegerShiftRightConstantsRule),
        ),
        Case::integer(
            19,
            unary_constant_unit(Unary::ExactCast, i16, i8, s(-128)),
            rule(ExactIntegerCastConstantsRule),
        ),
        Case::integer(
            20,
            unary_constant_unit(Unary::Widen, i8, i16, s(-128)),
            rule(IntegerWidenConstantsRule),
        ),
        Case::integer(
            21,
            unary_constant_unit(Unary::BitwiseNot, u8, u8, u(0)),
            rule(IntegerBitwiseNotConstantsRule),
        ),
        Case::integer(
            22,
            binary_constant_unit(Binary::BitwiseAnd, u8, u8, u(0b1010), u(0b1100)),
            rule(IntegerBitwiseAndConstantsRule),
        ),
        Case::integer(
            23,
            binary_constant_unit(Binary::BitwiseOr, u8, u8, u(0b1010), u(0b1100)),
            rule(IntegerBitwiseOrConstantsRule),
        ),
        Case::integer(
            24,
            binary_constant_unit(Binary::BitwiseXor, u8, u8, u(0b1010), u(0b1100)),
            rule(IntegerBitwiseXorConstantsRule),
        ),
    ]);
}
