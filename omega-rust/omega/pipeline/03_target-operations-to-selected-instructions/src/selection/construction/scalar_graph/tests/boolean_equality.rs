//! Boolean equality shares compare instructions, never integer semantic types.
use super::{
    IntegerSign, IntegerType, IntegerValue, LegalizedScalarFunction,
    LegalizedScalarInstructionKind, ScalarType, SelectedSelectionConstraints, ValueId, build,
};
use crate::selection::construction::scalar_graph::tests::control;
use legalized_operations::LegalizedScalarComparison as Comparison;

#[test]
fn boolean_comparison_replay_rejects_operand_carrier_and_ordering_substitution() {
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
        let mut source = control::graph(target, Comparison::Equal, true);
        let entry = source
            .blocks
            .iter_mut()
            .find(|block| block.id == source.entry_block)
            .unwrap();
        for (ordinal, row) in entry.instructions[..2].iter_mut().enumerate() {
            row.result.as_mut().unwrap().scalar_type = ScalarType::Boolean;
            row.kind =
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(ordinal as u128));
        }
        let LegalizedScalarInstructionKind::Compare {
            operand_type,
            right_operand_type,
            ..
        } = &mut entry.instructions[2].kind
        else {
            unreachable!();
        };
        *operand_type = ScalarType::Boolean;
        *right_operand_type = ScalarType::Boolean;
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |candidate: &LegalizedScalarFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                candidate,
                &selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source).unwrap();
        for mutation in 0..6 {
            let mut changed = source.clone();
            let entry = changed
                .blocks
                .iter_mut()
                .find(|block| block.id == changed.entry_block)
                .unwrap();
            let LegalizedScalarInstructionKind::Compare {
                predicate,
                operand_type,
                left,
                right,
                ..
            } = &mut entry.instructions[2].kind
            else {
                unreachable!();
            };
            match mutation {
                0 => *left = *right,
                1 => {
                    *operand_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
                }
                2 => {
                    *operand_type =
                        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64)
                }
                3 => *right = ValueId::new(999).unwrap(),
                4 | 5 => {
                    *left = *right;
                    *predicate = if mutation == 4 {
                        Comparison::LessThan
                    } else {
                        Comparison::LessOrEqual
                    };
                }
                _ => unreachable!(),
            }
            assert!(validate(&changed).is_err(), "mutation {mutation}");
            if mutation != 0 {
                assert!(
                    build(
                        0,
                        &changed,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints()
                    )
                    .is_err(),
                    "mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn mixed_integer_comparisons_preserve_values_and_replay_both_carriers() {
    use super::{
        CallSignature, CallingPolicy, EdgeId, EffectLink, LegalizedScalarInstruction,
        LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarTerminator,
        LegalizedValueDefinition, OperationId, ValueDefinitionSite, ValueShape, evaluate_call_plan,
    };
    use selected_instructions::SelectedInstructionKind;
    let integer = |sign, bits| ScalarType::Integer(IntegerType::new(sign, bits).unwrap());
    let signed = |bits, value| {
        (
            integer(IntegerSign::Signed, bits),
            IntegerValue::Signed(value),
        )
    };
    let unsigned = |bits, value| {
        (
            integer(IntegerSign::Unsigned, bits),
            IntegerValue::Unsigned(value),
        )
    };
    let cases = [
        (
            signed(64, i128::from(i64::MIN)),
            unsigned(64, u128::from(u64::MAX)),
        ),
        (signed(64, -1), unsigned(64, u128::from(u64::MAX))),
        (
            signed(64, i128::from(i64::MAX)),
            unsigned(64, u128::from(u64::MAX)),
        ),
        (signed(32, -1), unsigned(64, 10)),
        (signed(8, 0), unsigned(64, 0)),
        (signed(16, 10), unsigned(8, 10)),
        (signed(8, -128), signed(64, -129)),
        (unsigned(8, 255), unsigned(64, 256)),
    ];
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
        for (first, second) in cases {
            for (left, right) in [(first, second), (second, first)] {
                let mathematical = |value| match value {
                    IntegerValue::Signed(value) => value,
                    IntegerValue::Unsigned(value) => i128::try_from(value).unwrap(),
                };
                let ordering = mathematical(left.1).cmp(&mathematical(right.1));
                for (predicate, swapped, negated, expected) in [
                    (Comparison::Equal, false, false, ordering.is_eq()),
                    (Comparison::Equal, false, true, !ordering.is_eq()),
                    (Comparison::LessThan, false, false, ordering.is_lt()),
                    (Comparison::LessOrEqual, false, false, !ordering.is_gt()),
                    (Comparison::LessThan, true, false, ordering.is_gt()),
                    (Comparison::LessOrEqual, true, false, !ordering.is_lt()),
                ] {
                    let mut source = control::graph(target, predicate, true);
                    source.blocks.retain(|block| block.id == source.entry_block);
                    let block = &mut source.blocks[0];
                    for (row, (scalar_type, value)) in
                        block.instructions[..2].iter_mut().zip([left, right])
                    {
                        row.result.as_mut().unwrap().scalar_type = scalar_type;
                        row.kind = LegalizedScalarInstructionKind::Constant(value);
                    }
                    let operands = [
                        block.instructions[0].result.unwrap().value,
                        block.instructions[1].result.unwrap().value,
                    ];
                    let (left_position, right_position) = if swapped { (1, 0) } else { (0, 1) };
                    let carriers = [left.0, right.0];
                    block.instructions[2].kind = LegalizedScalarInstructionKind::Compare {
                        predicate,
                        operand_type: carriers[left_position],
                        right_operand_type: carriers[right_position],
                        left: operands[left_position],
                        right: operands[right_position],
                    };
                    let mut result = block.instructions[2].result.unwrap();
                    if negated {
                        let operand = result.value;
                        result = LegalizedValueDefinition {
                            value: ValueId::new(6).unwrap(),
                            scalar_type: ScalarType::Boolean,
                            definition_site: ValueDefinitionSite::Node {
                                block: block.id,
                                node: 3,
                            },
                        };
                        block.instructions.push(LegalizedScalarInstruction {
                            operation: OperationId::new(6).unwrap(),
                            result: Some(result),
                            kind: LegalizedScalarInstructionKind::BooleanNot { operand },
                            fuel: Vec::new(),
                            effect: EffectLink {
                                input: 0,
                                output: 1,
                            },
                            ownership: Vec::new(),
                        });
                    }
                    block.terminator = LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                        edge: EdgeId::new(1).unwrap(),
                        value: LegalizedScalarReturnValue::Value {
                            value: result.value,
                            scalar_type: ScalarType::Boolean,
                        },
                        fuel: Vec::new(),
                        effect: EffectLink {
                            input: 0,
                            output: 1,
                        },
                        ownership: Vec::new(),
                    });
                    source.call_plan.result = evaluate_call_plan(
                        CallingPolicy::native_for_target(target),
                        &CallSignature {
                            parameters: Vec::new(),
                            result: Some(ValueShape::integer(1, 1)),
                        },
                    )
                    .unwrap()
                    .result;
                    source.provenance.operations =
                        block.instructions.iter().map(|row| row.operation).collect();
                    source.provenance.edges = vec![EdgeId::new(1).unwrap()];
                    let selected = build(
                        0,
                        &source,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                    .unwrap();
                    let replay =
                        |source: &LegalizedScalarFunction, selected: &super::SelectedFunction| {
                            crate::selection::validation::scalar_graph::validate(
                                0,
                                source,
                                selected,
                                target,
                                &constraints,
                                environment.physical(),
                                environment.constraints(),
                            )
                        };
                    replay(&source, &selected).unwrap();
                    assert_eq!(
                        execute_integer_comparison(&selected),
                        u64::from(expected),
                        "{target:?} {left:?} {right:?} {predicate:?} swapped={swapped} negated={negated}"
                    );
                    let charged = selected.blocks[0]
                        .instructions
                        .iter()
                        .flat_map(|row| &row.provenance.fuel)
                        .filter(|fuel| {
                            fuel.site
                                == super::PsiProvenance::Operation(OperationId::new(5).unwrap())
                        })
                        .map(|fuel| fuel.units)
                        .sum::<u64>();
                    assert_eq!(charged, 1);
                    let mut changed = source.clone();
                    let LegalizedScalarInstructionKind::Compare {
                        right_operand_type, ..
                    } = &mut changed.blocks[0].instructions[2].kind
                    else {
                        unreachable!()
                    };
                    *right_operand_type = ScalarType::Boolean;
                    assert!(
                        build(
                            0,
                            &changed,
                            target,
                            &constraints,
                            environment.physical(),
                            environment.constraints()
                        )
                        .is_err()
                    );
                    assert!(replay(&changed, &selected).is_err());
                    let mut changed = selected.clone();
                    let materialize = changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|row| {
                            matches!(
                                row.kind,
                                SelectedInstructionKind::MaterializeBooleanEqual
                                    | SelectedInstructionKind::MaterializeBooleanI64LessThan
                                    | SelectedInstructionKind::MaterializeBooleanU64LessThan
                                    | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                                    | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                            )
                        })
                        .unwrap();
                    materialize.kind =
                        if materialize.kind == SelectedInstructionKind::MaterializeBooleanEqual {
                            SelectedInstructionKind::MaterializeBooleanU64LessThan
                        } else {
                            SelectedInstructionKind::MaterializeBooleanEqual
                        };
                    assert!(replay(&source, &changed).is_err());
                }
            }
        }
    }
}

/// Execute the selected register operations, independently of the comparison's
/// source carriers and of the producer/replay expansion's Boolean formula.
fn execute_integer_comparison(selected: &super::SelectedFunction) -> u64 {
    use selected_instructions::{SelectedInstructionKind as Instruction, SelectedTerminator};
    let mut values = vec![0u64; selected.virtual_registers.len()];
    let mut unsigned = std::cmp::Ordering::Equal;
    let mut signed = std::cmp::Ordering::Equal;
    for row in &selected.blocks[0].instructions {
        let operands = row
            .operands
            .iter()
            .map(|operand| operand.virtual_register.0 as usize)
            .collect::<Vec<_>>();
        match row.kind {
            Instruction::MaterializeI64 { value } => {
                values[operands[0]] = match value {
                    IntegerValue::Signed(value) => i64::try_from(value).unwrap() as u64,
                    IntegerValue::Unsigned(value) => u64::try_from(value).unwrap(),
                }
            }
            Instruction::CompareI64 => {
                unsigned = values[operands[0]].cmp(&values[operands[1]]);
                signed = (values[operands[0]] as i64).cmp(&(values[operands[1]] as i64));
            }
            Instruction::CompareI64Zero => {
                unsigned = values[operands[0]].cmp(&0);
                signed = (values[operands[0]] as i64).cmp(&0);
            }
            Instruction::MaterializeBooleanEqual => {
                values[operands[0]] = u64::from(unsigned.is_eq())
            }
            Instruction::MaterializeBooleanU64LessThan => {
                values[operands[0]] = u64::from(unsigned.is_lt())
            }
            Instruction::MaterializeBooleanU64LessOrEqual => {
                values[operands[0]] = u64::from(!unsigned.is_gt())
            }
            Instruction::MaterializeBooleanI64LessThan => {
                values[operands[0]] = u64::from(signed.is_lt())
            }
            Instruction::MaterializeBooleanI64LessOrEqual => {
                values[operands[0]] = u64::from(!signed.is_gt())
            }
            Instruction::CopyI64 => values[operands[1]] = values[operands[0]],
            Instruction::SignExtendI8 => {
                values[operands[1]] = values[operands[0]] as i8 as i64 as u64
            }
            Instruction::SignExtendI16 => {
                values[operands[1]] = values[operands[0]] as i16 as i64 as u64
            }
            Instruction::SignExtendI32 => {
                values[operands[1]] = values[operands[0]] as i32 as i64 as u64
            }
            Instruction::ZeroExtendU8 => values[operands[1]] = values[operands[0]] & 0xff,
            Instruction::ZeroExtendU16 => values[operands[1]] = values[operands[0]] & 0xffff,
            Instruction::ZeroExtendU32 => values[operands[1]] = values[operands[0]] & 0xffff_ffff,
            Instruction::ReturnScalar => return values[operands[0]],
            other => panic!("unexpected selected comparison instruction {other:?}"),
        }
    }
    let SelectedTerminator::Return { instruction, .. } = &selected.blocks[0].terminator else {
        panic!("return")
    };
    values[instruction.operands[0].virtual_register.0 as usize]
}
