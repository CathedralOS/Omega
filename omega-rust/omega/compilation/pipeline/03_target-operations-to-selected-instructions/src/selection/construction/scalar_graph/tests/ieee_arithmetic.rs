//! Selection and independent replay of inline nearest-even float operations.
//!
//! Each case computes one operation on one or two constants (IEEE payloads
//! or integers) and compares the result with the exact nearest-even payload,
//! whose truncated alternative differs for every rounding operation. The actual selected stream is executed (the arithmetic form by
//! the host's own round-to-nearest-even operation), and replay rejects a
//! substituted operation, format or operand order.
use super::{
    IntegerValue, LegalizedScalarInstructionKind, OperationId, ScalarType, SelectedInstructionKind,
    SelectedSelectionConstraints, build,
};
use legalized_operations::LegalizedValueDefinition;
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use selected_instructions::{IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation};
use semantic_vocabulary::{
    BoundaryMachineId, IeeeFloatComparisonOperation as Relation, IeeeFloatFormat, ValueId,
};

/// The host's own nearest-even operation on raw payloads: `operands` holds
/// the operation's operand payloads (integers zero-extended to 64 bits).
/// Minimum and maximum return the second operand unless the first is
/// strictly smaller or greater, as `FloatSemantics::minimum`/`maximum` do.
pub(super) fn host(form: IeeeFloatArithmeticForm, operands: &[u64]) -> u64 {
    use IeeeFloatArithmeticOperation::{
        Add, ConvertFormat, Divide, FromInteger, Maximum, Minimum, Multiply, Negate, SquareRoot,
        Subtract, TruncateToInteger,
    };
    use selected_instructions::IeeeFloatIntegerCarrier::{I32, I64, U32};
    let left = operands[0];
    let right = operands.get(1).copied().unwrap_or_default();
    if let TruncateToInteger { target, .. } = form.operation {
        // Rust's float-to-integer `as` saturates and maps NaN to zero, the
        // Saturating contract; `i32` results keep a sign-extended carrier.
        let value = match form.format {
            IeeeFloatFormat::Binary32 => f64::from(f32::from_bits(left as u32)),
            IeeeFloatFormat::Binary64 => f64::from_bits(left),
        };
        return match target {
            I32 => value as i32 as i64 as u64,
            I64 => value as i64 as u64,
            U32 => u64::from(value as u32),
        };
    }
    match form.format {
        IeeeFloatFormat::Binary32 => {
            let (a, b) = (f32::from_bits(left as u32), f32::from_bits(right as u32));
            u64::from(
                match form.operation {
                    Add => a + b,
                    Subtract => a - b,
                    Multiply => a * b,
                    Divide => a / b,
                    Minimum => {
                        if a < b {
                            a
                        } else {
                            b
                        }
                    }
                    Maximum => {
                        if a > b {
                            a
                        } else {
                            b
                        }
                    }
                    Negate => -a,
                    SquareRoot => a.sqrt(),
                    ConvertFormat => f64::from_bits(left) as f32,
                    FromInteger(I32) => left as u32 as i32 as f32,
                    FromInteger(I64) => left as i64 as f32,
                    FromInteger(U32) => left as u32 as f32,
                    TruncateToInteger { .. } => unreachable!("integer results return above"),
                }
                .to_bits(),
            )
        }
        IeeeFloatFormat::Binary64 => {
            let (a, b) = (f64::from_bits(left), f64::from_bits(right));
            match form.operation {
                Add => a + b,
                Subtract => a - b,
                Multiply => a * b,
                Divide => a / b,
                Minimum => {
                    if a < b {
                        a
                    } else {
                        b
                    }
                }
                Maximum => {
                    if a > b {
                        a
                    } else {
                        b
                    }
                }
                Negate => -a,
                SquareRoot => a.sqrt(),
                ConvertFormat => f64::from(f32::from_bits(left as u32)),
                FromInteger(I32) => f64::from(left as u32 as i32),
                FromInteger(I64) => left as i64 as f64,
                FromInteger(U32) => f64::from(left as u32),
                TruncateToInteger { .. } => unreachable!("integer results return above"),
            }
            .to_bits()
        }
    }
}

/// `(left, right, nearest-even result)` per form; `right` is unused by the
/// one-operand forms. Truncation differs from each rounding result, and each
/// minimum/maximum case has a NaN first operand, which yields the second.
fn case(form: IeeeFloatArithmeticForm) -> [u64; 3] {
    use IeeeFloatArithmeticOperation::{
        Add, ConvertFormat, Divide, FromInteger, Maximum, Minimum, Multiply, Negate, SquareRoot,
        Subtract, TruncateToInteger,
    };
    use IeeeFloatFormat::{Binary32, Binary64};
    use selected_instructions::IeeeFloatIntegerCarrier::{I32, I64, U32};
    use selected_instructions::IeeeFloatToIntegerDomain::{Exact, Saturating};
    if let TruncateToInteger { target, domain } = form.operation {
        // Exact cases truncate an in-range operand toward zero; Saturating
        // cases clamp an out-of-range operand or map NaN to zero.
        return match (form.format, target, domain) {
            (Binary64, I32, Exact) => [0x4093_4b99_9999_999a, 0, 1234],
            (Binary64, I64, Exact) => [0x4340_0000_0000_0000, 0, 9_007_199_254_740_992],
            (Binary64, U32, Exact) => [0x41e6_5a0b_c000_0000, 0, 3_000_000_000],
            (Binary64, I32, Saturating) => [0x41e6_5a0b_c000_0000, 0, 0x7fff_ffff],
            (Binary64, I64, Saturating) => [0x7ff8_0000_0000_0000, 0, 0],
            (Binary64, U32, Saturating) => [0x41f2_a05f_2000_0000, 0, 0xffff_ffff],
            (Binary32, I32, Exact) => [0x4b80_0000, 0, 16_777_216],
            (Binary32, I64, Exact) => [0x5380_0000, 0, 1_099_511_627_776],
            (Binary32, U32, Exact) => [0x4f6e_6b28, 0, 4_000_000_000],
            (Binary32, I32, Saturating) => [0x5015_02f9, 0, 0x7fff_ffff],
            (Binary32, I64, Saturating) => [0x5f50_2ab5, 0, 0x7fff_ffff_ffff_ffff],
            (Binary32, U32, Saturating) => [0xc0a0_0000, 0, 0],
        };
    }
    match (form.format, form.operation) {
        // 1 + 3*2^-54 -> 1 + 2^-52
        (Binary64, Add) => [
            0x3ff0_0000_0000_0000,
            0x3ca8_0000_0000_0000,
            0x3ff0_0000_0000_0001,
        ],
        // 1 - 2^-55 -> 1
        (Binary64, Subtract) => [
            0x3ff0_0000_0000_0000,
            0x3c80_0000_0000_0000,
            0x3ff0_0000_0000_0000,
        ],
        // 0.1 * 3 -> 0.30000000000000004
        (Binary64, Multiply) => [
            0x3fb9_9999_9999_999a,
            0x4008_0000_0000_0000,
            0x3fd3_3333_3333_3334,
        ],
        // 10 / 3 -> 3.3333333333333335
        (Binary64, Divide) => [
            0x4024_0000_0000_0000,
            0x4008_0000_0000_0000,
            0x400a_aaaa_aaaa_aaab,
        ],
        // minimum(NaN, 1) -> 1; maximum(NaN, 2) -> 2
        (Binary64, Minimum) => [
            0x7ff8_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x3ff0_0000_0000_0000,
        ],
        (Binary64, Maximum) => [
            0x7ff8_0000_0000_0000,
            0x4000_0000_0000_0000,
            0x4000_0000_0000_0000,
        ],
        (Binary64, Negate) => [0x3ff0_0000_0000_0000, 0, 0xbff0_0000_0000_0000],
        // sqrt(2) -> 1.4142135623730951
        (Binary64, SquareRoot) => [0x4000_0000_0000_0000, 0, 0x3ff6_a09e_667f_3bcd],
        // 0.1f32 widens exactly.
        (Binary64, ConvertFormat) => [0x3dcc_cccd, 0, 0x3fb9_9999_a000_0000],
        // 2^24 + 1 is exact in binary64.
        (Binary64, FromInteger(I32)) => [16_777_217, 0, 0x4170_0000_1000_0000],
        // 2^53 + 1 ties to the even 2^53.
        (Binary64, FromInteger(I64)) => [9_007_199_254_740_993, 0, 0x4340_0000_0000_0000],
        // u32::MAX is exact in binary64.
        (Binary64, FromInteger(U32)) => [0xffff_ffff, 0, 0x41ef_ffff_ffe0_0000],
        (Binary32, Add) => [0x3f80_0000, 0x33c0_0000, 0x3f80_0001],
        (Binary32, Subtract) => [0x3f80_0000, 0x3280_0000, 0x3f80_0000],
        (Binary32, Multiply) => [0x3dcc_cccd, 0x4040_0000, 0x3e99_999a],
        (Binary32, Divide) => [0x3f80_0000, 0x4040_0000, 0x3eaa_aaab],
        (Binary32, Minimum) => [0x7fc0_0000, 0x3f80_0000, 0x3f80_0000],
        (Binary32, Maximum) => [0x7fc0_0000, 0x4000_0000, 0x4000_0000],
        (Binary32, Negate) => [0x3f80_0000, 0, 0xbf80_0000],
        (Binary32, SquareRoot) => [0x4000_0000, 0, 0x3fb5_04f3],
        // 0.1 narrows to the nearest binary32, 0.1f32.
        (Binary32, ConvertFormat) => [0x3fb9_9999_9999_999a, 0, 0x3dcc_cccd],
        // 2^24 + 1 ties to the even 2^24.
        (Binary32, FromInteger(I32)) => [16_777_217, 0, 0x4b80_0000],
        (Binary32, FromInteger(I64)) => [9_007_199_254_740_993, 0, 0x5a00_0000],
        // u32::MAX rounds up to 2^32.
        (Binary32, FromInteger(U32)) => [0xffff_ffff, 0, 0x4f80_0000],
        (_, TruncateToInteger { .. }) => unreachable!("integer results return above"),
    }
}

fn arithmetic_source(
    target: target::NativeTarget,
    form: IeeeFloatArithmeticForm,
) -> legalized_operations::LegalizedScalarFunction {
    let [left, right, expected] = case(form);
    let mut source = super::ieee_comparisons::comparison_source(
        target,
        Relation::Equal,
        form.format,
        [left, right],
    );
    let block_id = source.entry_block;
    let block = &mut source.blocks[0];
    // A one-operand form reads only the first constant, of its operand type.
    block.instructions[0].result.as_mut().unwrap().scalar_type =
        form.operation.operand_type(form.format);
    let mut compare = block.instructions.pop().unwrap();
    let left_value = block.instructions[0].result.unwrap().value;
    let right_value = block.instructions[1].result.unwrap().value;
    let scalar = form.operation.result_type(form.format);
    let row = |template: &legalized_operations::LegalizedScalarInstruction,
               raw: u64,
               node: u32,
               scalar_type: ScalarType,
               kind: LegalizedScalarInstructionKind| {
        let mut row = template.clone();
        row.operation = OperationId::new(raw).unwrap();
        row.result = Some(LegalizedValueDefinition {
            value: ValueId::new(raw).unwrap(),
            scalar_type,
            definition_site: ValueDefinitionSite::Node {
                block: block_id,
                node,
            },
        });
        row.kind = kind;
        row.fuel = vec![FuelSettlement {
            site: PsiProvenance::Operation(row.operation),
            units: 1,
        }];
        row
    };
    let expected_row = row(
        &block.instructions[1],
        17,
        2,
        scalar,
        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(u128::from(expected))),
    );
    let arithmetic = row(
        &compare,
        16,
        3,
        scalar,
        LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
            boundary: BoundaryMachineId::new(1).unwrap(),
            operation: form.operation,
            format: form.format,
            left: left_value,
            right: (form.operand_count() == 2).then_some(right_value),
        },
    );
    compare.result.as_mut().unwrap().definition_site = ValueDefinitionSite::Node {
        block: block_id,
        node: 4,
    };
    compare.kind = if matches!(scalar, ScalarType::IeeeFloat(_)) {
        LegalizedScalarInstructionKind::IeeeFloatCompare {
            comparison: Relation::Equal,
            format: form.format,
            left: ValueId::new(16).unwrap(),
            right: ValueId::new(17).unwrap(),
        }
    } else {
        LegalizedScalarInstructionKind::Compare {
            predicate: legalized_operations::LegalizedScalarComparison::Equal,
            operand_type: scalar,
            right_operand_type: scalar,
            left: ValueId::new(16).unwrap(),
            right: ValueId::new(17).unwrap(),
        }
    };
    block
        .instructions
        .extend([expected_row, arithmetic, compare]);
    source.provenance.operations = block
        .instructions
        .iter()
        .map(|operation| operation.operation)
        .collect();
    source
}

#[test]
fn every_form_selects_one_constrained_instruction_and_replays_on_every_target() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for form in IeeeFloatArithmeticForm::ALL {
            let [left, right, expected] = case(form);
            let operands = &[left, right][..form.operand_count()];
            assert_eq!(host(form, operands), expected, "{form:?}");
            let key = if form.operand_count() == 2 {
                constraints.keys.ieee_float_arithmetic.unwrap()
            } else {
                constraints.keys.ieee_float_unary.unwrap()
            };
            let source = arithmetic_source(target, form);
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |selected: &selected_instructions::SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &source,
                    selected,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            assert_eq!(
                super::ieee_comparisons::execute(&selected),
                1,
                "{target:?} {form:?}"
            );
            let arithmetic = selected.blocks[0]
                .instructions
                .iter()
                .enumerate()
                .filter(|(_, instruction)| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::NearestIeeeFloatArithmetic { .. }
                    )
                })
                .collect::<Vec<_>>();
            let [(position, instruction)] = arithmetic.as_slice() else {
                panic!("{target:?} {form:?} selects exactly one arithmetic form");
            };
            assert_eq!(
                instruction.kind,
                SelectedInstructionKind::NearestIeeeFloatArithmetic { form }
            );
            assert_eq!(instruction.constraint, key);
            assert_eq!(instruction.operands.len(), form.operand_count() + 1);
            assert!(
                !instruction.clobbers.is_empty(),
                "scratch registers are declared"
            );
            assert_eq!(
                instruction.provenance.operations,
                [OperationId::new(16).unwrap()]
            );

            // Replay rebuilds the form from the legal instruction: another
            // operation or format, or swapped operands, cannot stand in.
            for other in IeeeFloatArithmeticForm::ALL {
                if other != form {
                    let mut substituted = selected.clone();
                    substituted.blocks[0].instructions[*position].kind =
                        SelectedInstructionKind::NearestIeeeFloatArithmetic { form: other };
                    assert!(validate(&substituted).is_err(), "{form:?} -> {other:?}");
                }
            }
            let mut swapped = selected.clone();
            swapped.blocks[0].instructions[*position]
                .operands
                .swap(0, 1);
            assert!(validate(&swapped).is_err());
        }
    }
}
