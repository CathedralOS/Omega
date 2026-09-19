//! Parallel-copy preparation is independent of authored cycle admission.
use super::{
    IntegerSign, ScalarType, SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    SelectedSelectionConstraints, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueTransport, ValueDefinitionSite, ValueId, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin, project,
};
use crate::selection::edge_transfers::prepare;
use crate::selection::edge_transfers::successors_mut;
use semantic_vocabulary::{BlockId, EdgeId, IntegerType, MachineId};

#[test]
fn shared_conditional_fallthrough_bridges_project_to_exact_original_edges() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for predicate in 0..3 {
            let successor = |edge, destination| SelectedSuccessor {
                structural_case: None,
                structural_bindings: Vec::new(),
                role: SelectedSuccessorRole::Semantic,
                psi_edge: EdgeId::new(edge).unwrap(),
                block: SelectedBlockId(destination),
                source_target: BlockId::new(u64::from(destination) + 101).unwrap(),
                bindings: Vec::new(),
                fuel: Vec::new(),
            };
            let mut blocks = Vec::new();
            for position in 0..4 {
                let conditional = position < 2;
                let kind = if conditional {
                    match predicate {
                        0 => SelectedInstructionKind::ConditionalBranchNonZero,
                        1 => SelectedInstructionKind::ConditionalBranchU64LessThan,
                        _ => SelectedInstructionKind::ConditionalBranchI64LessThan,
                    }
                } else {
                    SelectedInstructionKind::Jump
                };
                let instruction = super::super::constraints::instruction(
                    SelectedInstructionId(position),
                    kind,
                    if conditional {
                        constraints.keys.conditional_branch
                    } else {
                        constraints.keys.jump
                    },
                    &[],
                    SelectedInstructionProvenance::default(),
                    environment.constraints(),
                )
                .unwrap();
                let terminator = if conditional {
                    let when_taken = successor(
                        u64::from(position) * 2 + 1,
                        if position == 0 { 1 } else { 3 },
                    );
                    let when_fallthrough = successor(u64::from(position) * 2 + 2, 2);
                    match predicate {
                        0 => SelectedTerminator::ConditionalBranch {
                            instruction,
                            when_nonzero: when_taken,
                            when_zero: when_fallthrough,
                        },
                        1 => SelectedTerminator::ConditionalBranchU64LessThan {
                            instruction,
                            when_less: when_taken,
                            when_not_less: when_fallthrough,
                        },
                        _ => SelectedTerminator::ConditionalBranchI64LessThan {
                            instruction,
                            when_less: when_taken,
                            when_not_less: when_fallthrough,
                        },
                    }
                } else {
                    SelectedTerminator::Jump {
                        instruction,
                        successor: successor(u64::from(position) + 3, 3),
                    }
                };
                blocks.push(SelectedBlock {
                    id: SelectedBlockId(position),
                    origin: SelectedBlockOrigin::Source(
                        BlockId::new(u64::from(position) + 101).unwrap(),
                    ),
                    instructions: Vec::new(),
                    terminator,
                });
            }
            let original = SelectedFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: target_operations::TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: (1..=6).map(|edge| EdgeId::new(edge).unwrap()).collect(),
                },
                structural: None,
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: Vec::new(),
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                entry_block: SelectedBlockId(0),
                virtual_registers: Vec::new(),
                blocks,
            };
            let mut prepared = original.clone();
            prepare(0, &mut prepared, &constraints, environment.constraints()).unwrap();
            assert_eq!(prepared.blocks.len(), 5);
            assert!(prepared.blocks[4].instructions.is_empty());
            assert_eq!(project(0, &prepared, &constraints).unwrap(), original);
            let first_false = successors_mut(&mut prepared.blocks[0].terminator)[1].block;
            let second_false = successors_mut(&mut prepared.blocks[1].terminator)[1].block;
            assert_eq!(
                (first_false, second_false),
                (SelectedBlockId(2), SelectedBlockId(4))
            );
            for corruption in 0..4 {
                let mut changed = prepared.clone();
                let SelectedTerminator::Jump {
                    instruction,
                    successor,
                } = &mut changed.blocks[4].terminator
                else {
                    panic!("bridge continuation");
                };
                match corruption {
                    0 => successor.psi_edge = EdgeId::new(99).unwrap(),
                    1 => successor.role = SelectedSuccessorRole::Semantic,
                    2 => instruction.provenance.edges.push(successor.psi_edge),
                    _ => successor.source_target = BlockId::new(999).unwrap(),
                }
                assert!(
                    project(0, &changed, &constraints).is_err(),
                    "corruption {corruption}"
                );
            }
            let mut redirected = prepared.clone();
            let SelectedTerminator::Jump { successor, .. } = &mut redirected.blocks[4].terminator
            else {
                panic!("bridge");
            };
            successor.block = SelectedBlockId(3);
            assert_ne!(
                project(0, &redirected, &constraints).unwrap(),
                original,
                "downstream semantic replay must see and reject a redirected original edge"
            );
            let mut wrong_polarity = prepared.clone();
            let mut successors = successors_mut(&mut wrong_polarity.blocks[1].terminator);
            let (taken, fallthrough) = successors.split_at_mut(1);
            std::mem::swap(taken[0], fallthrough[0]);
            assert!(
                project(0, &wrong_polarity, &constraints).is_err(),
                "empty bridges cannot move to a taken edge"
            );
        }
    }
}

#[test]
fn cyclic_swap_snapshots_both_inputs_before_destination_copies() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let class =
            super::super::constraints::row(environment.constraints(), constraints.keys.copy_i64)
                .unwrap()
                .operands[0]
                .class;
        let block = BlockId::new(1).unwrap();
        let edge = EdgeId::new(1).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let registers = (0..2)
            .map(|position| VirtualRegister {
                id: VirtualRegisterId(position),
                scalar_type,
                class,
                origin: VirtualRegisterOrigin::BlockParameter {
                    source_value: ValueId::new(u64::from(position) + 1).unwrap(),
                    block: SelectedBlockId(0),
                    parameter_index: position as usize,
                },
                definition_site: Some(ValueDefinitionSite::BlockParameter { block, position }),
                entry_fixed_view: None,
            })
            .collect();
        let bindings = (0..2)
            .map(|position| selected_instructions::SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(u64::from(position) + 1).unwrap(),
                    argument: ValueId::new(u64::from(1 - position) + 1).unwrap(),
                    scalar_type,
                },
                transport: SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(1 - position),
                    parameter: VirtualRegisterId(position),
                },
            })
            .collect();
        let jump = super::super::constraints::instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::Jump,
            constraints.keys.jump,
            &[],
            SelectedInstructionProvenance::default(),
            environment.constraints(),
        )
        .unwrap();
        let original = SelectedFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: target_operations::TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![edge],
            },
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(block),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Jump {
                    instruction: jump,
                    successor: SelectedSuccessor {
                        structural_case: None,
                        structural_bindings: Vec::new(),
                        role: SelectedSuccessorRole::Semantic,
                        psi_edge: edge,
                        block: SelectedBlockId(0),
                        source_target: block,
                        bindings,
                        fuel: Vec::new(),
                    },
                },
            }],
        };
        let mut prepared = original.clone();
        prepare(0, &mut prepared, &constraints, environment.constraints()).unwrap();
        assert_eq!(project(0, &prepared, &constraints).unwrap(), original);
        let copies = &prepared.blocks[1].instructions;
        assert_eq!(
            copies
                .iter()
                .map(|copy| (
                    copy.operands[0].virtual_register.0,
                    copy.operands[1].virtual_register.0
                ))
                .collect::<Vec<_>>(),
            vec![(1, 2), (0, 3), (2, 4), (3, 5)]
        );
        // Associate the transfer outputs with destination homes after the
        // snapshots; this remains a swap even when old and new homes alias.
        let mut homes = (0..prepared.virtual_registers.len()).collect::<Vec<_>>();
        let SelectedTerminator::Jump { successor, .. } = &prepared.blocks[1].terminator else {
            panic!("continuation");
        };
        for binding in &successor.bindings {
            let SelectedValueTransport::Registers {
                argument,
                parameter,
            } = binding.transport
            else {
                panic!("transfer");
            };
            homes[argument.0 as usize] = homes[parameter.0 as usize];
        }
        let mut physical = vec![0_u64; prepared.virtual_registers.len()];
        physical[0] = 11;
        physical[1] = 22;
        for expected in [[22, 11], [11, 22]] {
            for copy in copies {
                assert_eq!(copy.kind, SelectedInstructionKind::CopyI64);
                let input = homes[copy.operands[0].virtual_register.0 as usize];
                let output = homes[copy.operands[1].virtual_register.0 as usize];
                physical[output] = physical[input];
            }
            assert_eq!(&physical[..2], &expected);
        }
        for mutation in 0..8 {
            let mut changed = prepared.clone();
            let bridge = &mut changed.blocks[1];
            match mutation {
                0 => bridge.instructions.swap(1, 2),
                1 => bridge.instructions[2].operands[0].virtual_register = VirtualRegisterId(1),
                2 => bridge.instructions[0].provenance.edges.push(edge),
                3 => bridge.origin = SelectedBlockOrigin::Source(block),
                4 => {
                    bridge.instructions.pop();
                }
                _ => {
                    let SelectedTerminator::Jump { successor, .. } = &mut bridge.terminator else {
                        panic!("jump");
                    };
                    match mutation {
                        5 => successor.role = SelectedSuccessorRole::Semantic,
                        6 => successor.bindings.swap(0, 1),
                        _ => successor.block = SelectedBlockId(1),
                    }
                }
            }
            assert!(
                project(0, &changed, &constraints).is_err(),
                "mutation {mutation}: {native:?}"
            );
        }
    }
}
