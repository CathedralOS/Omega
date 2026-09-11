//! Execute the actual selected integer stream against IEEE edge-case expectations.
use super::*;
use semantic_vocabulary::{IeeeFloatComparisonOperation as Relation, IeeeFloatFormat};

fn comparison_source(
    target: target::NativeTarget,
    comparison: Relation,
    format: IeeeFloatFormat,
    bits: [u64; 2],
) -> LegalizedScalarFunction {
    let mut source = control::graph(
        target,
        legalized_operations::LegalizedScalarComparison::Equal,
        true,
    );
    source.blocks.retain(|block| block.id == source.entry_block);
    let block = &mut source.blocks[0];
    for (operation, bits) in block.instructions[..2].iter_mut().zip(bits) {
        operation.kind =
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(u128::from(bits)));
        operation.result.as_mut().unwrap().scalar_type = ScalarType::IeeeFloat(format);
    }
    let left = block.instructions[0].result.unwrap().value;
    let right = block.instructions[1].result.unwrap().value;
    let operation = block.instructions.last_mut().unwrap();
    operation.kind = LegalizedScalarInstructionKind::IeeeFloatCompare {
        comparison,
        format,
        left,
        right,
    };
    let result = operation.result.unwrap();
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
    source.provenance.operations = block
        .instructions
        .iter()
        .map(|operation| operation.operation)
        .collect();
    source.provenance.edges = vec![EdgeId::new(1).unwrap()];
    source
}

fn execute(selected: &SelectedFunction) -> u64 {
    let mut values = vec![0u64; selected.virtual_registers.len()];
    let mut condition = std::cmp::Ordering::Equal;
    let mut signed_condition = std::cmp::Ordering::Equal;
    for instruction in &selected.blocks[0].instructions {
        let operands = instruction
            .operands
            .iter()
            .map(|operand| operand.virtual_register.0 as usize)
            .collect::<Vec<_>>();
        match instruction.kind {
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(value),
            } => values[operands[0]] = u64::try_from(value).unwrap(),
            SelectedInstructionKind::CompareI64 => {
                condition = values[operands[0]].cmp(&values[operands[1]]);
                signed_condition = (values[operands[0]] as i64).cmp(&(values[operands[1]] as i64));
            }
            SelectedInstructionKind::CompareI64Zero => {
                condition = values[operands[0]].cmp(&0);
                signed_condition = (values[operands[0]] as i64).cmp(&0);
            }
            SelectedInstructionKind::MaterializeBooleanEqual => {
                values[operands[0]] = u64::from(condition.is_eq())
            }
            SelectedInstructionKind::MaterializeBooleanU64LessThan => {
                values[operands[0]] = u64::from(condition.is_lt())
            }
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
                values[operands[0]] = u64::from(!condition.is_gt())
            }
            SelectedInstructionKind::CopyI64 => values[operands[1]] = values[operands[0]],
            SelectedInstructionKind::MaterializeBooleanI64LessThan => {
                values[operands[0]] = u64::from(signed_condition.is_lt())
            }
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
                values[operands[0]] = u64::from(!signed_condition.is_gt())
            }
            SelectedInstructionKind::SignExtendI32 => {
                values[operands[1]] = values[operands[0]] as i32 as i64 as u64
            }
            SelectedInstructionKind::ZeroExtendU8 => {
                values[operands[1]] = values[operands[0]] & 0xff
            }
            SelectedInstructionKind::ReturnScalar => return values[operands[0]],
            other => panic!("unexpected comparison instruction {other:?}"),
        }
    }
    let SelectedTerminator::Return { instruction, .. } = &selected.blocks[0].terminator else {
        panic!("comparison did not return");
    };
    assert_eq!(instruction.kind, SelectedInstructionKind::ReturnScalar);
    values[instruction.operands[0].virtual_register.0 as usize]
}

#[test]
fn every_ieee_relation_uses_existing_integer_forms_and_replays_both_formats() {
    use Relation::*;
    let relations = [Equal, NotEqual, Less, LessOrEqual, Greater, GreaterOrEqual];
    let equal = [true, false, false, true, false, true];
    let less = [false, true, true, true, false, false];
    let greater = [false, true, false, false, true, true];
    let unordered = [false, true, false, false, false, false];
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
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let values = match format {
                IeeeFloatFormat::Binary32 => [
                    0,
                    0x8000_0000,
                    0x3f80_0000,
                    0x4000_0000,
                    1,
                    0x7f80_0000,
                    0xff80_0000,
                    0x7fc0_0042,
                    0xff80_0001,
                    0xbf80_0000,
                    0xc000_0000,
                    0x8000_0001,
                ],
                IeeeFloatFormat::Binary64 => [
                    0,
                    0x8000_0000_0000_0000,
                    0x3ff0_0000_0000_0000,
                    0x4000_0000_0000_0000,
                    1,
                    0x7ff0_0000_0000_0000,
                    0xfff0_0000_0000_0000,
                    0x7ff8_0000_0000_0042,
                    0xfff0_0000_0000_0001,
                    0xbff0_0000_0000_0000,
                    0xc000_0000_0000_0000,
                    0x8000_0000_0000_0001,
                ],
            };
            for (left, right, expected) in [
                (0, 1, equal),
                (1, 0, equal),
                (2, 3, less),
                (3, 2, greater),
                (4, 0, greater),
                (11, 0, less),
                (5, 5, equal),
                (6, 2, less),
                (2, 7, unordered),
                (7, 2, unordered),
                (7, 7, unordered),
                (8, 2, unordered),
                (2, 8, unordered),
                (8, 8, unordered),
                (9, 10, greater),
                (10, 9, less),
                (9, 2, less),
                (2, 9, greater),
            ] {
                for (relation, expected) in relations.into_iter().zip(expected) {
                    let source =
                        comparison_source(target, relation, format, [values[left], values[right]]);
                    let selected = build(
                        0,
                        &source,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                    .unwrap();
                    crate::selection::validation::scalar_graph::validate(
                        0,
                        &source,
                        &selected,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                    .unwrap();
                    assert_eq!(
                        execute(&selected),
                        u64::from(expected),
                        "{target:?} {format:?} {relation:?} {left} {right}"
                    );
                    let fuel = selected.blocks[0]
                        .instructions
                        .iter()
                        .flat_map(|instruction| &instruction.provenance.fuel)
                        .filter(|fuel| {
                            fuel.site == PsiProvenance::Operation(OperationId::new(5).unwrap())
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        fuel.len(),
                        1,
                        "one logical comparison despite private intermediates"
                    );
                    assert_eq!(fuel[0].units, 1);
                    let count = selected.blocks[0]
                        .instructions
                        .iter()
                        .filter(|instruction| {
                            instruction
                                .provenance
                                .operations
                                .contains(&OperationId::new(5).unwrap())
                        })
                        .count();
                    let expected_count = match relation {
                        Equal => 54,
                        NotEqual => 56,
                        Less | Greater => 78,
                        LessOrEqual | GreaterOrEqual => 82,
                    } + usize::from(format == IeeeFloatFormat::Binary32) * 2;
                    assert_eq!(
                        count, expected_count,
                        "the full comparison span retains its operation attribution"
                    );
                }
            }
        }
    }
}

#[test]
fn comparison_replay_rejects_relation_carrier_operands_constants_and_boolean_rules() {
    let target = target::NativeTarget::macos_arm64();
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    let constraints = SelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs: Vec::new(),
    };
    let source = comparison_source(
        target,
        Relation::Less,
        IeeeFloatFormat::Binary64,
        [0xbff0_0000_0000_0000, 0x3ff0_0000_0000_0000],
    );
    let selected = build(
        0,
        &source,
        target,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .unwrap();
    let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
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
    for mutation in ["relation", "operand", "format", "result"] {
        let mut changed = source.clone();
        let operation = changed.blocks[0].instructions.last_mut().unwrap();
        let LegalizedScalarInstructionKind::IeeeFloatCompare {
            comparison,
            left,
            right,
            format,
        } = &mut operation.kind
        else {
            panic!("comparison")
        };
        match mutation {
            "relation" => *comparison = Relation::Greater,
            "operand" => *right = *left,
            "format" => *format = IeeeFloatFormat::Binary32,
            "result" => {
                operation.result.as_mut().unwrap().scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
            }
            _ => unreachable!(),
        }
        assert!(validate(&changed, &selected).is_err(), "{mutation}");
    }
    for mutation in ["constant", "boolean", "operand", "fuel"] {
        let mut changed = selected.clone();
        let instructions = &mut changed.blocks[0].instructions;
        match mutation {
            "constant" => {
                let instruction = instructions
                    .iter_mut()
                    .find(|instruction| {
                        matches!(
                            instruction.kind,
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(0x8000_0000_0000_0000)
                            }
                        )
                    })
                    .unwrap();
                instruction.kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                };
            }
            "boolean" => {
                let instruction = instructions
                    .iter_mut()
                    .find(|instruction| {
                        instruction.kind
                            == SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                    })
                    .unwrap();
                instruction.kind = SelectedInstructionKind::MaterializeBooleanU64LessThan;
            }
            "operand" => {
                let instruction = instructions
                    .iter_mut()
                    .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64)
                    .unwrap();
                instruction.operands.swap(0, 1);
            }
            "fuel" => {
                let instruction = instructions
                    .iter_mut()
                    .find(|instruction| {
                        instruction.provenance.operations == vec![OperationId::new(5).unwrap()]
                    })
                    .unwrap();
                instruction.provenance.fuel.clear();
            }
            _ => unreachable!(),
        }
        assert!(validate(&source, &changed).is_err(), "{mutation}");
    }
}
