use super::control_flow::{cfg_fixture, successor};
use super::parameters::parameter_fixture;
use super::*;
use selected_instructions::SelectedBlockOrigin;

fn downstream_use_fixture(
    target: NativeTarget,
    parameter: bool,
    implementation: bool,
) -> ValidatedRuntimeSpill {
    let mut source = if parameter {
        parameter_fixture(target)
    } else {
        cfg_fixture(target)
    };
    let environment = baseline_target_register_environment(target).unwrap();
    let jump = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let anchor_index = if parameter { 2 } else { 1 };
    let next_block = function.blocks.len() as u32;
    let anchor = &mut function.blocks[anchor_index];
    let tail = SelectedBlock {
        id: SelectedBlockId(next_block),
        origin: if implementation {
            SelectedBlockOrigin::EdgeTransfer {
                edge: EdgeId::new(2).unwrap(),
                target: BlockId::new(3).unwrap(),
            }
        } else {
            SelectedBlockOrigin::Source(BlockId::new(4).unwrap())
        },
        instructions: vec![anchor.instructions.pop().unwrap()],
        terminator: anchor.terminator.clone(),
    };
    anchor.terminator = SelectedTerminator::Jump {
        instruction: admission::instruction(
            SelectedInstructionId(2000),
            SelectedInstructionKind::Jump,
            jump,
            &[],
        ),
        successor: successor(next_block),
    };
    for settlement in &mut function.boundary_settlements {
        if settlement.block == anchor.id
            && settlement.instruction_index > anchor.instructions.len() as u32
        {
            settlement.instruction_index = anchor.instructions.len() as u32;
        }
    }
    function.blocks.push(tail);
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

#[test]
fn dominated_uses_in_source_and_implementation_blocks_reload_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for parameter in [false, true] {
            for implementation in [false, true] {
                let mut source = downstream_use_fixture(target, parameter, implementation);
                // Uses may precede their dominating definition in storage order.
                Arc::make_mut(&mut source.transformed).functions[0]
                    .blocks
                    .reverse();
                let environment = baseline_target_register_environment(target).unwrap();
                let result = spill_selected_runtime_value(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                )
                .unwrap();
                let original = &source.transformed().functions[0];
                let transformed = &result.transformed().functions[0];
                for (before, after) in original.blocks.iter().zip(&transformed.blocks) {
                    assert_eq!(before.id, after.id);
                    assert_eq!(before.origin, after.origin);
                    assert_eq!(before.terminator, after.terminator);
                }
                let tail = &transformed.blocks[0];
                assert_eq!(tail.instructions.len(), 3);
                assert!(matches!(
                    tail.instructions[0].kind,
                    SelectedInstructionKind::FrameAddress { .. }
                ));
                assert!(matches!(
                    tail.instructions[1].kind,
                    SelectedInstructionKind::Load64 { .. }
                ));
                assert_eq!(
                    tail.instructions[2].id,
                    original.blocks[0].instructions[0].id
                );
                assert_ne!(
                    tail.instructions[2].operands[0].virtual_register,
                    VirtualRegisterId(1)
                );
            }
        }
    }
}

#[test]
fn bypassed_and_unreachable_uses_do_not_gain_initialized_storage() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for parameter in [false, true] {
        for bypass in [false, true] {
            let mut source = downstream_use_fixture(NativeTarget::linux_x64(), parameter, false);
            let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
            let tail_index = function.blocks.len() - 1;
            let anchor_index = if parameter { 2 } else { 1 };
            if bypass {
                let instruction = super::super::control(&function.blocks[0].terminator)
                    .0
                    .clone();
                function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero: successor(1),
                    when_zero: successor(function.blocks[tail_index].id.0),
                };
            } else {
                function.blocks[anchor_index].terminator =
                    function.blocks[tail_index].terminator.clone();
            }
            assert_eq!(
                spill_selected_runtime_value(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget()
                )
                .unwrap_err(),
                RuntimeSpillError::UnsupportedUse,
                "parameter={parameter} bypass={bypass}"
            );
        }
    }
}

#[test]
fn replay_requires_every_dominated_reload_and_exact_dominating_edges() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    for parameter in [false, true] {
        let source = downstream_use_fixture(NativeTarget::linux_x64(), parameter, false);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        for mutation in 0..3 {
            let mut proposed = result.transformed().clone();
            let function = &mut proposed.functions[0];
            let tail_index = function.blocks.len() - 1;
            match mutation {
                0 => {
                    function.blocks[tail_index].instructions.remove(1);
                }
                1 => {
                    function.blocks[tail_index].instructions[2].operands[0].virtual_register =
                        VirtualRegisterId(1)
                }
                2 => {
                    let instruction = super::super::control(&function.blocks[0].terminator)
                        .0
                        .clone();
                    function.blocks[0].terminator = SelectedTerminator::Jump {
                        instruction,
                        successor: successor(function.blocks[tail_index].id.0),
                    };
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_runtime_spill(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                    proposed
                )
                .unwrap_err(),
                RuntimeSpillError::ReplayMismatch
            );
        }
    }
}
