//! Projection controls over raw graph data; no source-admission receipt is invented.
use super::*;
use calling_conventions::{CallSignature, ValueShape, evaluate_call_plan};
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarBlock, LegalizedScalarCall, LegalizedScalarInstruction,
    LegalizedScalarParameter, LegalizedScalarReturn,
};
use optimization_unit::EffectLink;
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, StructuralTypeId,
};

fn fixture(target: target::NativeTarget, count: usize) -> LegalizedScalarFunction {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let shape = ValueShape::integer(8, 8);
    let block = BlockId::new(1).unwrap();
    let effect = EffectLink {
        input: 0,
        output: 1,
    };
    let mut instructions = Vec::new();
    for raw in 1..=4 {
        let operation = OperationId::new(raw).unwrap();
        let kind = if raw <= 2 {
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(u128::from(raw)))
        } else {
            let arity = if raw == 3 { count } else { 1 };
            let call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![shape; arity],
                    result: Some(shape),
                },
            )
            .unwrap();
            let arguments = call_plan
                .parameters
                .iter()
                .enumerate()
                .map(|(index, placement)| LegalizedScalarArgument {
                    source: ValueId::new(if raw == 4 { 3 } else { 1 + (index % 2) as u64 })
                        .unwrap(),
                    placement: placement.clone(),
                })
                .collect();
            LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                callee: MachineId::new(raw + 7).unwrap(),
                arguments,
                result_placement: call_plan.result.clone().unwrap(),
                call_plan,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            })
        };
        instructions.push(LegalizedScalarInstruction {
            operation,
            result: ValueId::new(raw).unwrap(),
            scalar_type: integer,
            definition_site: ValueDefinitionSite::Node {
                block,
                node: raw as u32 - 1,
            },
            kind,
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(operation),
                units: 1,
            }],
            effect,
            ownership: Vec::new(),
        });
    }
    LegalizedScalarFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: Some(StructuralTypeId::new(1).unwrap()),
        provenance: target_operations::TerminalPsiProvenance {
            operations: (1..=4).map(|raw| OperationId::new(raw).unwrap()).collect(),
            edges: vec![EdgeId::new(1).unwrap()],
        },
        call_plan: evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .unwrap(),
        parameters: Vec::new(),
        entry_block: block,
        blocks: vec![LegalizedScalarBlock {
            id: block,
            instructions,
            terminator: LegalizedScalarReturn {
                edge: EdgeId::new(1).unwrap(),
                value: LegalizedScalarReturnValue::Unit,
                fuel: Vec::new(),
                effect,
                ownership: Vec::new(),
            },
        }],
    }
}

#[test]
fn register_call_arities_preserve_occurrences_and_reject_changed_projection() {
    for (target, maximum) in [
        (target::NativeTarget::linux_x64(), 6),
        (target::NativeTarget::linux_arm64(), 8),
        (target::NativeTarget::windows_x64(), 4),
        (target::NativeTarget::macos_arm64(), 8),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for count in 0..=maximum {
            let source = fixture(target, count);
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |selected: &SelectedFunction| {
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
            let call_index = 2 + count;
            assert_eq!(selected.blocks[0].instructions.len(), count + 7);
            assert_eq!(
                selected.blocks[0].instructions[call_index].operands.len(),
                count + 1
            );
            // The following one-argument call consumes the prior durable result,
            // whose position depends on the preceding call's actual arity.
            assert_eq!(
                selected.blocks[0].instructions[call_index + 2].operands[0].virtual_register,
                VirtualRegisterId((call_index + 1) as u32)
            );
            for corruption in 0..6 {
                let mut changed = selected.clone();
                let instructions = &mut changed.blocks[0].instructions;
                match corruption {
                    0 => {
                        instructions[0].operands[0].virtual_register = VirtualRegisterId(1);
                        instructions[1].operands[0].virtual_register = VirtualRegisterId(0);
                    }
                    1 => {
                        instructions[call_index].operands.pop();
                    }
                    2 => {
                        instructions[call_index + 1].operands[0].virtual_register =
                            VirtualRegisterId(0)
                    }
                    3 => instructions[call_index]
                        .clobbers
                        .push(register_model::RegisterUnitId(999)),
                    4 => instructions[call_index].provenance.fuel[0].units += 1,
                    _ => {
                        instructions.pop();
                    }
                }
                assert!(
                    validate(&changed).is_err(),
                    "arity {count}, corruption {corruption}"
                );
            }
            let mut wrong_keys = constraints.clone();
            wrong_keys.keys.call_i64[count] =
                constraints.keys.call_i64[(count + 1) % (maximum + 1)];
            assert!(
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &source,
                    &selected,
                    target,
                    &wrong_keys,
                    environment.physical(),
                    environment.constraints()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn scalar_returns_and_entry_parameters_keep_short_abi_transport() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        for return_parameter in [false, true] {
            let mut source = fixture(target, 1);
            source.attachment = None;
            source.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![ValueShape::integer(8, 8)],
                    result: Some(ValueShape::integer(8, 8)),
                },
            )
            .unwrap();
            let parameter = ValueId::new(9).unwrap();
            let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let placement = source.call_plan.parameters[0].clone();
            let [ValueLocation::Register { register, .. }] = placement.locations.as_slice() else {
                panic!("register ABI");
            };
            let fixed_view = environment.fixed_register_view(*register).unwrap();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                projected_structural_call: None,
                fixed_inputs: vec![SelectedFixedInputConstraint {
                    machine: source.machine,
                    source_value: parameter,
                    parameter_index: 0,
                    register: *register,
                    fixed_view,
                }],
            };
            source.parameters.push(LegalizedScalarParameter {
                value: parameter,
                scalar_type: integer,
                definition_site: ValueDefinitionSite::FunctionParameter(0),
                placement,
            });
            let LegalizedScalarInstructionKind::Call(call) =
                &mut source.blocks[0].instructions[2].kind
            else {
                unreachable!();
            };
            call.arguments[0].source = parameter;
            source.blocks[0].terminator.value = LegalizedScalarReturnValue::Value {
                value: if return_parameter {
                    parameter
                } else {
                    ValueId::new(4).unwrap()
                },
                scalar_type: integer,
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
                selected.virtual_registers[0].entry_fixed_view,
                Some(fixed_view)
            );
            assert_eq!(selected.virtual_registers[1].entry_fixed_view, None);
            let rows = &selected.blocks[0].instructions;
            assert_eq!(rows[0].kind, SelectedInstructionKind::CopyI64);
            assert_eq!(rows[0].operands[0].virtual_register, VirtualRegisterId(0));
            assert_eq!(rows[0].operands[1].virtual_register, VirtualRegisterId(1));
            assert_eq!(rows[3].operands[0].virtual_register, VirtualRegisterId(1));
            assert_eq!(rows.last().unwrap().kind, SelectedInstructionKind::CopyI64);
            for corruption in 0..8 {
                let mut changed = selected.clone();
                match corruption {
                    0 => {
                        changed.blocks[0].instructions[3].operands[0].virtual_register =
                            VirtualRegisterId(0)
                    }
                    1 => changed.virtual_registers[0].entry_fixed_view = None,
                    2 => changed.virtual_registers[1].entry_fixed_view = Some(fixed_view),
                    3 => {
                        changed.blocks[0].instructions[4].kind = SelectedInstructionKind::CallI64 {
                            callee: MachineId::new(99).unwrap(),
                        }
                    }
                    4 => changed.blocks[0].instructions[4].operands[0].fixed_view = None,
                    5 => {
                        changed.blocks[0].instructions.last_mut().unwrap().operands[0]
                            .virtual_register = VirtualRegisterId(0)
                    }
                    6 => {
                        let SelectedTerminator::Return { instruction, .. } =
                            &mut changed.blocks[0].terminator
                        else {
                            unreachable!();
                        };
                        instruction.operands[0].virtual_register = VirtualRegisterId(0);
                    }
                    _ => {
                        let SelectedTerminator::Return { instruction, .. } =
                            &mut changed.blocks[0].terminator
                        else {
                            unreachable!();
                        };
                        instruction.kind = SelectedInstructionKind::ReturnUnit;
                    }
                }
                assert!(
                    validate(&changed).is_err(),
                    "return parameter {return_parameter}, corruption {corruption}"
                );
            }
        }
    }
}
