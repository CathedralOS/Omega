//! Reentry executes the establishment store in its original loop block.
use super::*;
use legalized_operations::LegalizedScalarSuccessor;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedBlockOrigin};

#[test]
fn primitive_local_loop_reentry_rejects_missing_or_hoisted_initialization() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = local_fixture(target, false);
        let entry = source.entry_block;
        let header = BlockId::new(2).unwrap();
        let jump = |raw, destination| LegalizedScalarTerminator::Jump {
            successor: LegalizedScalarSuccessor {
                edge: EdgeId::new(raw).unwrap(),
                target: destination,
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(raw).unwrap()),
                    units: 1,
                }],
            },
            effect: EffectLink {
                input: 1,
                output: 1,
            },
            ownership: Vec::new(),
        };
        let mut instructions = source.blocks[0].instructions.split_off(1);
        for (position, instruction) in instructions.iter_mut().enumerate() {
            if let Some(result) = &mut instruction.result {
                result.definition_site = ValueDefinitionSite::Node {
                    block: header,
                    node: position as u32,
                };
            }
        }
        source.blocks[0].terminator = jump(1, header);
        source.blocks.push(LegalizedScalarBlock {
            id: header,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            instructions,
            terminator: jump(2, header),
        });
        source.provenance.edges.push(EdgeId::new(2).unwrap());
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
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
        let preheader = selected
            .blocks
            .iter()
            .position(|block| block.origin == SelectedBlockOrigin::Source(entry))
            .unwrap();
        let loop_block = selected
            .blocks
            .iter()
            .position(|block| block.origin == SelectedBlockOrigin::Source(header))
            .unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(2).unwrap(),
            place: PlaceId::new(1).unwrap(),
        };
        assert_eq!(selected.local_storage_slots.len(), 1);
        assert!(
            selected.blocks[preheader]
                .instructions
                .iter()
                .all(|row| !matches!(
                    row.kind,
                    SelectedInstructionKind::FrameAddress { .. }
                        | SelectedInstructionKind::Store { .. }
                ))
        );
        assert!(
            selected.blocks[loop_block]
                .instructions
                .iter()
                .any(|row| row.kind
                    == SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    })
        );
        let store = selected.blocks[loop_block]
            .instructions
            .iter()
            .position(|row| {
                matches!(
                    row.kind,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8
                    }
                ) && row.provenance.operations == [OperationId::new(2).unwrap()]
            })
            .unwrap();
        assert_eq!(
            selected.blocks[loop_block].instructions[store]
                .provenance
                .fuel,
            source.blocks[1].instructions[0].fuel
        );
        for hoist in [false, true] {
            let mut changed = selected.clone();
            let initialization = changed.blocks[loop_block].instructions.remove(store);
            if hoist {
                changed.blocks[preheader].instructions.push(initialization);
            } else {
                changed
                    .memory_accesses
                    .retain(|access| access.instruction != initialization.id);
            }
            assert!(
                validate(&changed).is_err(),
                "initialization hoisted: {hoist}"
            );
        }
    }
}
