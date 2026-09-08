//! Parallel-copy preparation is independent of authored cycle admission.
use super::*;
use semantic_vocabulary::{BlockId, EdgeId, IntegerType, MachineId};

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
            projected_structural_call: None,
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
            ranked: None,
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
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
