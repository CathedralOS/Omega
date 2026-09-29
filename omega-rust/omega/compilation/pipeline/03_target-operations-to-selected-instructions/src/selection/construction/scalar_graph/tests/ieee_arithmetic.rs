//! Selection and independent replay of inline nearest-even float arithmetic.
//!
//! Each case computes one operation on two IEEE constants and compares the
//! result with the exact nearest-even payload, whose truncated alternative
//! differs. The actual selected stream is executed (the arithmetic form by
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

/// The host's own nearest-even operation on raw payloads.
pub(super) fn host(form: IeeeFloatArithmeticForm, left: u64, right: u64) -> u64 {
    use IeeeFloatArithmeticOperation::{Add, Divide, Multiply, Subtract};
    match form.format {
        IeeeFloatFormat::Binary32 => {
            let (left, right) = (f32::from_bits(left as u32), f32::from_bits(right as u32));
            u64::from(
                match form.operation {
                    Add => left + right,
                    Subtract => left - right,
                    Multiply => left * right,
                    Divide => left / right,
                }
                .to_bits(),
            )
        }
        IeeeFloatFormat::Binary64 => {
            let (left, right) = (f64::from_bits(left), f64::from_bits(right));
            match form.operation {
                Add => left + right,
                Subtract => left - right,
                Multiply => left * right,
                Divide => left / right,
            }
            .to_bits()
        }
    }
}

/// `(left, right, nearest-even result)` per form; truncation differs in each.
fn case(form: IeeeFloatArithmeticForm) -> [u64; 3] {
    use IeeeFloatArithmeticOperation::{Add, Divide, Multiply, Subtract};
    match (form.format, form.operation) {
        // 1 + 3*2^-54 -> 1 + 2^-52
        (IeeeFloatFormat::Binary64, Add) => [
            0x3ff0_0000_0000_0000,
            0x3ca8_0000_0000_0000,
            0x3ff0_0000_0000_0001,
        ],
        // 1 - 2^-55 -> 1
        (IeeeFloatFormat::Binary64, Subtract) => [
            0x3ff0_0000_0000_0000,
            0x3c80_0000_0000_0000,
            0x3ff0_0000_0000_0000,
        ],
        // 0.1 * 3 -> 0.30000000000000004
        (IeeeFloatFormat::Binary64, Multiply) => [
            0x3fb9_9999_9999_999a,
            0x4008_0000_0000_0000,
            0x3fd3_3333_3333_3334,
        ],
        // 10 / 3 -> 3.3333333333333335
        (IeeeFloatFormat::Binary64, Divide) => [
            0x4024_0000_0000_0000,
            0x4008_0000_0000_0000,
            0x400a_aaaa_aaaa_aaab,
        ],
        (IeeeFloatFormat::Binary32, Add) => [0x3f80_0000, 0x33c0_0000, 0x3f80_0001],
        (IeeeFloatFormat::Binary32, Subtract) => [0x3f80_0000, 0x3280_0000, 0x3f80_0000],
        (IeeeFloatFormat::Binary32, Multiply) => [0x3dcc_cccd, 0x4040_0000, 0x3e99_999a],
        (IeeeFloatFormat::Binary32, Divide) => [0x3f80_0000, 0x4040_0000, 0x3eaa_aaab],
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
    let mut compare = block.instructions.pop().unwrap();
    let left_value = block.instructions[0].result.unwrap().value;
    let right_value = block.instructions[1].result.unwrap().value;
    let scalar = ScalarType::IeeeFloat(form.format);
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
            right: right_value,
        },
    );
    compare.result.as_mut().unwrap().definition_site = ValueDefinitionSite::Node {
        block: block_id,
        node: 4,
    };
    compare.kind = LegalizedScalarInstructionKind::IeeeFloatCompare {
        comparison: Relation::Equal,
        format: form.format,
        left: ValueId::new(16).unwrap(),
        right: ValueId::new(17).unwrap(),
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
        let key = constraints.keys.ieee_float_arithmetic.unwrap();
        for form in IeeeFloatArithmeticForm::ALL {
            let [left, right, expected] = case(form);
            assert_eq!(host(form, left, right), expected, "{form:?}");
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
            assert_eq!(instruction.operands.len(), 3);
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
