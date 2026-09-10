//! Materialized Boolean values retain exact conditions, result origins and fuel.
use super::*;
use legalized_operations::LegalizedScalarComparison;

#[test]
fn boolean_return_materialization_replays_condition_result_and_fuel() {
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
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for signed in [false, true] {
            for predicate in [
                LegalizedScalarComparison::Equal,
                LegalizedScalarComparison::LessThan,
                LegalizedScalarComparison::LessOrEqual,
            ] {
                let mut source = control::graph(target, predicate, signed);
                source.blocks.retain(|block| block.id == source.entry_block);
                let block = &mut source.blocks[0];
                let result = block.instructions.last().unwrap().result.unwrap();
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
                let index = selected.blocks[0]
                    .instructions
                    .iter()
                    .position(|row| {
                        matches!(
                            row.kind,
                            SelectedInstructionKind::MaterializeBooleanEqual
                                | SelectedInstructionKind::MaterializeBooleanU64LessThan
                                | SelectedInstructionKind::MaterializeBooleanI64LessThan
                                | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                                | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                        )
                    })
                    .unwrap();
                assert!(
                    selected.blocks[0].instructions[index]
                        .provenance
                        .operations
                        .is_empty()
                );
                assert!(
                    selected.blocks[0].instructions[index]
                        .provenance
                        .fuel
                        .is_empty()
                );
                for corruption in 0..8 {
                    let mut changed = selected.clone();
                    let rows = &mut changed.blocks[0].instructions;
                    match corruption {
                        0 => {
                            rows[index].kind = if rows[index].kind
                                == SelectedInstructionKind::MaterializeBooleanEqual
                            {
                                SelectedInstructionKind::MaterializeBooleanU64LessThan
                            } else {
                                SelectedInstructionKind::MaterializeBooleanEqual
                            }
                        }
                        1 => {
                            rows.remove(index);
                        }
                        2 => {
                            let duplicate = rows[index].clone();
                            rows.insert(index, duplicate);
                        }
                        3 => rows[index].provenance.values = vec![ValueId::new(999).unwrap()],
                        4 => rows[index].provenance.fuel = rows[index - 1].provenance.fuel.clone(),
                        5 => rows[index - 1].provenance.operations.clear(),
                        6 => {
                            rows.swap(index - 1, index);
                        }
                        _ => {
                            let register = changed
                                .virtual_registers
                                .iter_mut()
                                .find(|register| register.scalar_type == ScalarType::Boolean)
                                .unwrap();
                            register.scalar_type = ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                            );
                        }
                    }
                    assert!(validate(&changed).is_err(), "corruption {corruption}");
                }
            }
        }
    }
}
