//! Current CFG projection, including out-of-order source blocks and exact edge bindings.
use super::*;
use abstract_operations::ValueBinding;
use legalized_operations::{LegalizedScalarComparison as Comparison, LegalizedScalarSuccessor};
use optimization_unit::ValueDefinition;

fn graph(
    target: target::NativeTarget,
    predicate: Comparison,
    signed: bool,
) -> LegalizedScalarFunction {
    let mut source = fixture(target, 2);
    source.attachment = None;
    source.call_plan.result = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap()
    .result;
    let integer = IntegerType::new(
        if signed {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        },
        64,
    )
    .unwrap();
    if signed {
        source.blocks[0].instructions.truncate(2);
        for (index, row) in source.blocks[0].instructions.iter_mut().enumerate() {
            row.scalar_type = ScalarType::Integer(integer);
            row.kind =
                LegalizedScalarInstructionKind::Constant(IntegerValue::Signed(index as i128));
        }
    }
    let value = |raw| ValueId::new(raw).unwrap();
    let block = |raw| BlockId::new(raw).unwrap();
    let edge = |raw| EdgeId::new(raw).unwrap();
    let effect = EffectLink {
        input: 0,
        output: 1,
    };
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let successor = |raw, target, bindings| LegalizedScalarSuccessor {
        edge: edge(raw),
        target: block(target),
        bindings,
        fuel: Vec::new(),
    };
    let comparison_index = source.blocks[0].instructions.len();
    source.blocks[0]
        .instructions
        .push(LegalizedScalarInstruction {
            operation: OperationId::new(5).unwrap(),
            result: value(5),
            scalar_type: ScalarType::Boolean,
            definition_site: ValueDefinitionSite::Node {
                block: block(1),
                node: comparison_index as u32,
            },
            kind: LegalizedScalarInstructionKind::Compare {
                predicate,
                operand_type: integer,
                left: value(if signed { 2 } else { 4 }),
                right: value(1),
            },
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(OperationId::new(5).unwrap()),
                units: 1,
            }],
            effect,
            ownership: Vec::new(),
        });
    source.blocks[0].terminator = LegalizedScalarTerminator::Conditional {
        condition: value(5),
        when_true: successor(1, 2, Vec::new()),
        when_false: successor(2, 3, Vec::new()),
        effect,
        ownership: Vec::new(),
    };
    for raw in [2, 3] {
        let operation = OperationId::new(raw + 4).unwrap();
        source.blocks.push(LegalizedScalarBlock {
            id: block(raw),
            parameters: Vec::new(),
            instructions: vec![LegalizedScalarInstruction {
                operation,
                result: value(raw + 4),
                scalar_type: scalar,
                definition_site: ValueDefinitionSite::Node {
                    block: block(raw),
                    node: 0,
                },
                kind: LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(raw as u128)),
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Operation(operation),
                    units: 1,
                }],
                effect,
                ownership: Vec::new(),
            }],
            terminator: LegalizedScalarTerminator::Jump {
                successor: successor(
                    raw + 1,
                    4,
                    vec![ValueBinding {
                        parameter: value(8),
                        argument: value(raw + 4),
                        scalar_type: scalar,
                    }],
                ),
                effect,
                ownership: Vec::new(),
            },
        });
    }
    source.blocks.push(LegalizedScalarBlock {
        id: block(4),
        parameters: vec![ValueDefinition {
            value: value(8),
            scalar_type: scalar,
            site: ValueDefinitionSite::BlockParameter {
                block: block(4),
                position: 0,
            },
        }],
        instructions: Vec::new(),
        terminator: LegalizedScalarTerminator::Return(LegalizedScalarReturn {
            edge: edge(5),
            value: LegalizedScalarReturnValue::Value {
                value: value(8),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            },
            fuel: Vec::new(),
            effect,
            ownership: Vec::new(),
        }),
    });
    // Numeric IDs and source storage order are not execution order.
    source.blocks.swap(0, 3);
    source.provenance.operations = source
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter().map(|row| row.operation))
        .collect();
    source.provenance.edges = (1..=5).map(edge).collect();
    source
}

#[test]
fn scalar_control_keeps_blocks_branches_calls_and_parallel_bindings() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for signed in [false, true] {
            for predicate in [
                Comparison::Equal,
                Comparison::LessThan,
                Comparison::LessOrEqual,
            ] {
                let source = graph(target, predicate, signed);
                let selected = build(
                    0,
                    &source,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                let validate = |candidate: &SelectedFunction| {
                    crate::selection::validation::scalar_graph::validate(
                        0,
                        &source,
                        candidate,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                };
                validate(&selected).unwrap();
                assert_eq!(
                    selected
                        .blocks
                        .iter()
                        .map(|block| block.source_block.get())
                        .collect::<Vec<_>>(),
                    vec![1, 2, 3, 4]
                );
                let instruction_ids = selected
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        block
                            .instructions
                            .iter()
                            .map(|row| row.id.0)
                            .chain(std::iter::once(match &block.terminator {
                                SelectedTerminator::Return { instruction, .. }
                                | SelectedTerminator::Jump { instruction, .. }
                                | SelectedTerminator::ConditionalBranch { instruction, .. }
                                | SelectedTerminator::ConditionalBranchI64LessThan {
                                    instruction,
                                    ..
                                }
                                | SelectedTerminator::ConditionalBranchU64LessThan {
                                    instruction,
                                    ..
                                } => instruction.id.0,
                            }))
                    })
                    .collect::<Vec<_>>();
                assert!(
                    instruction_ids
                        .iter()
                        .copied()
                        .eq(0..instruction_ids.len() as u32)
                );
                for corruption in 0..8 {
                    let mut changed = selected.clone();
                    match corruption {
                        0 => changed.blocks[1].source_block = changed.blocks[2].source_block,
                        1 => changed.blocks[1].id = SelectedBlockId(99),
                        2 => {
                            let SelectedTerminator::Jump { successor, .. } =
                                &mut changed.blocks[1].terminator
                            else {
                                unreachable!();
                            };
                            successor.block = SelectedBlockId(2);
                        }
                        3 => {
                            let SelectedTerminator::Jump { successor, .. } =
                                &mut changed.blocks[1].terminator
                            else {
                                unreachable!();
                            };
                            successor.bindings[0].semantic.argument = ValueId::new(7).unwrap();
                        }
                        4 => {
                            let SelectedTerminator::Jump { successor, .. } =
                                &mut changed.blocks[1].terminator
                            else {
                                unreachable!();
                            };
                            successor.bindings.clear();
                        }
                        5 => {
                            changed.blocks[0].instructions.last_mut().unwrap().kind =
                                SelectedInstructionKind::CompareI64Zero
                        }
                        6 => changed.blocks[0].instructions.swap(0, 1),
                        _ => changed.blocks[0].terminator = changed.blocks[3].terminator.clone(),
                    }
                    assert!(
                        validate(&changed).is_err(),
                        "{predicate:?}, signed {signed}, corruption {corruption}"
                    );
                }
            }
        }
    }
}

#[test]
fn graph_zero_equality_retains_fuel_and_does_not_elide_shared_zero() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for shared in [false, true] {
            let mut source = graph(target, Comparison::Equal, false);
            let entry = source
                .blocks
                .iter_mut()
                .find(|block| block.id == source.entry_block)
                .unwrap();
            let mut comparison = entry.instructions.pop().unwrap();
            let zero_value = ValueId::new(90).unwrap();
            let zero_operation = OperationId::new(90).unwrap();
            entry.instructions.push(LegalizedScalarInstruction {
                operation: zero_operation,
                result: zero_value,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                definition_site: ValueDefinitionSite::Node {
                    block: entry.id,
                    node: entry.instructions.len() as u32,
                },
                kind: LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(0)),
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Operation(zero_operation),
                    units: 1,
                }],
                effect: comparison.effect,
                ownership: Vec::new(),
            });
            comparison.definition_site = ValueDefinitionSite::Node {
                block: entry.id,
                node: entry.instructions.len() as u32,
            };
            let LegalizedScalarInstructionKind::Compare { right, .. } = &mut comparison.kind else {
                unreachable!()
            };
            *right = zero_value;
            entry.instructions.push(comparison);
            if shared {
                let returned = source
                    .blocks
                    .iter_mut()
                    .find_map(|block| match &mut block.terminator {
                        LegalizedScalarTerminator::Return(returned) => Some(returned),
                        _ => None,
                    })
                    .unwrap();
                let LegalizedScalarReturnValue::Value { value, .. } = &mut returned.value else {
                    unreachable!()
                };
                *value = zero_value;
            }
            source.provenance.operations = source
                .blocks
                .iter()
                .flat_map(|block| block.instructions.iter().map(|row| row.operation))
                .collect();
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |candidate: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &source,
                    candidate,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            let comparison = selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::CompareI64
                            | SelectedInstructionKind::CompareI64Zero
                    )
                })
                .unwrap();
            assert_eq!(
                comparison.kind == SelectedInstructionKind::CompareI64Zero,
                !shared
            );
            if !shared {
                assert_eq!(comparison.provenance.fuel.len(), 2);
                let mut corrupt = selected.clone();
                let comparison = corrupt
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|instruction| instruction.kind == SelectedInstructionKind::CompareI64Zero)
                    .unwrap();
                comparison.provenance.fuel.pop();
                assert!(validate(&corrupt).is_err());
            }
        }
    }
}

#[test]
fn edge_transport_names_durable_call_result_not_abi_temporary() {
    use selected_instructions::SelectedValueTransport;
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let mut source = graph(target, Comparison::LessThan, false);
        for block in &mut source.blocks {
            if let LegalizedScalarTerminator::Jump { successor, .. } = &mut block.terminator {
                successor.bindings[0].argument = ValueId::new(4).unwrap();
            }
        }
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                candidate,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        let call = selected.blocks[0]
            .instructions
            .iter()
            .rposition(|row| matches!(row.kind, SelectedInstructionKind::CallI64 { .. }))
            .unwrap();
        let short = selected.blocks[0].instructions[call]
            .operands
            .last()
            .unwrap()
            .virtual_register;
        let durable = selected.blocks[0].instructions[call + 1].operands[1].virtual_register;
        let SelectedTerminator::Jump { successor, .. } = &selected.blocks[1].terminator else {
            unreachable!();
        };
        let SelectedValueTransport::Registers {
            argument,
            parameter,
        } = successor.bindings[0].transport
        else {
            panic!("materialized join");
        };
        assert_eq!(argument, durable);
        assert_ne!(argument, short);
        for transport in [
            SelectedValueTransport::Unused,
            SelectedValueTransport::Registers {
                argument: short,
                parameter,
            },
            SelectedValueTransport::Registers {
                argument,
                parameter: argument,
            },
        ] {
            let mut changed = selected.clone();
            let SelectedTerminator::Jump { successor, .. } = &mut changed.blocks[1].terminator
            else {
                unreachable!();
            };
            successor.bindings[0].transport = transport;
            assert!(validate(&changed).is_err());
        }
        // A dominating call result may also be read directly after the join.
        let joined = source
            .blocks
            .iter_mut()
            .find(|block| block.id == BlockId::new(4).unwrap())
            .unwrap();
        returned(joined).value = LegalizedScalarReturnValue::Value {
            value: ValueId::new(4).unwrap(),
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        };
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
        let SelectedTerminator::Jump { successor, .. } = &selected.blocks[1].terminator else {
            unreachable!();
        };
        assert_eq!(
            successor.bindings[0].transport,
            SelectedValueTransport::Unused
        );
    }
}
