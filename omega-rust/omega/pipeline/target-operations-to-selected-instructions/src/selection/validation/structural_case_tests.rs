//! Lawful case graphs and hostile substitutions through full selected admission.
use super::*;
use selected_instructions::{
    SelectedBlockOrigin, SelectedCasePayloadTransport as Transport, SelectedMemoryAccessOrigin,
};
use semantic_vocabulary::{PlaceId, StructuralFieldId};

#[test]
fn case_payload_selection_replays_only_the_selected_edge_and_rejects_substitution() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let (abstracted, target, unit) =
            crate::tests::legalization::structural_case::fixture(native);
        let legal = crate::legalize_target_operations(&target, &abstracted, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap()
        .plan()
        .clone();
        let validate = |plan| {
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                plan,
            )
        };
        validate(selected.clone()).expect("the complete prepared case graph replays");
        let function = &selected.functions[0];
        let bridge = function
            .blocks
            .iter()
            .position(|block| matches!(block.origin, SelectedBlockOrigin::EdgeTransfer { .. }))
            .unwrap();
        let dispatch = function.blocks.iter().position(|block| matches!(&block.terminator,
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } if when_nonzero.structural_case.is_some())).unwrap();
        assert_eq!(function.blocks[bridge].instructions.len(), 2);
        let load_register = function.blocks[bridge].instructions[1].operands[1].virtual_register;
        let source_count = function
            .blocks
            .iter()
            .filter(|block| matches!(block.origin, SelectedBlockOrigin::Source(_)))
            .count();
        assert_eq!(source_count, 4);
        for mutation in 0..21 {
            let mut changed = selected.clone();
            let function = &mut changed.functions[0];
            match mutation {
                0 => {
                    function.blocks[bridge].instructions[1].kind =
                        SelectedInstructionKind::Load32 { byte_offset: 0 }
                }
                1 => function.blocks[bridge].instructions.swap(0, 1),
                2 => {
                    function.virtual_registers[load_register.0 as usize].definition_site =
                        Some(ValueDefinitionSite::FunctionParameter(0))
                }
                3 => {
                    function.virtual_registers[load_register.0 as usize].origin =
                        VirtualRegisterOrigin::InstructionResult {
                            instruction: function.blocks[bridge].instructions[1].id,
                            source_value: ValueId::new(10).unwrap(),
                        }
                }
                4 => function.blocks[bridge].instructions[1]
                    .provenance
                    .edges
                    .clear(),
                5 => {
                    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
                        &mut function.blocks[dispatch].terminator
                    else {
                        unreachable!()
                    };
                    when_nonzero.fuel[0].units += 1;
                }
                6 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[bridge].terminator
                    else {
                        unreachable!()
                    };
                    successor.fuel.push(FuelSettlement {
                        site: PsiProvenance::Edge(successor.psi_edge),
                        units: 1,
                    });
                }
                7 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[bridge].terminator
                    else {
                        unreachable!()
                    };
                    successor.structural_case.as_mut().unwrap().payloads[0].transport =
                        Transport::Unused;
                }
                8 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[bridge].terminator
                    else {
                        unreachable!()
                    };
                    let payload = &mut successor.structural_case.as_mut().unwrap().payloads[0];
                    let Transport::Registers { parameter, .. } = payload.transport else {
                        unreachable!()
                    };
                    payload.transport = Transport::Unmaterialized { parameter };
                }
                9 => {
                    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
                        &mut function.blocks[dispatch].terminator
                    else {
                        unreachable!()
                    };
                    when_nonzero.structural_case.as_mut().unwrap().payloads[0].transport =
                        Transport::Registers {
                            argument: load_register,
                            parameter: load_register,
                        };
                }
                10 => {
                    let access = function
                        .memory_accesses
                        .iter_mut()
                        .find(|access| {
                            access.instruction == function.blocks[bridge].instructions[1].id
                        })
                        .unwrap();
                    access.byte_offset = 0;
                }
                11 => {
                    function.memory_accesses.retain(|access| {
                        !matches!(access.origin, SelectedMemoryAccessOrigin::Edge(_))
                    });
                }
                12 => {
                    function.blocks[bridge].instructions[1].kind =
                        SelectedInstructionKind::Load64 { byte_offset: 4 }
                }
                13 => {
                    let SelectedTerminator::ConditionalBranch {
                        when_nonzero,
                        when_zero,
                        ..
                    } = &mut function.blocks[dispatch].terminator
                    else {
                        unreachable!()
                    };
                    std::mem::swap(when_nonzero, when_zero);
                }
                14..=17 => {
                    // Coordinated metadata forgery must survive neither projection nor source replay.
                    for block in &mut function.blocks {
                        let edges = match &mut block.terminator {
                            SelectedTerminator::ConditionalBranch {
                                when_nonzero,
                                when_zero,
                                ..
                            } => vec![when_nonzero, when_zero],
                            SelectedTerminator::Jump { successor, .. } => vec![successor],
                            _ => Vec::new(),
                        };
                        for edge in edges {
                            let Some(case) = &mut edge.structural_case else {
                                continue;
                            };
                            if case.payloads.is_empty() {
                                continue;
                            }
                            match mutation {
                                14 => {
                                    case.payloads[0].semantic.field =
                                        StructuralFieldId::new(99).unwrap()
                                }
                                15 => {
                                    case.payloads[0].semantic.parameter.definition_site =
                                        ValueDefinitionSite::FunctionParameter(0)
                                }
                                16 => case.trivial_affine_discards.push(PlaceId::new(99).unwrap()),
                                17 => case.case_tag = 0,
                                _ => unreachable!(),
                            }
                        }
                    }
                }
                18 => {
                    let tag = function.blocks[dispatch]
                        .instructions
                        .iter()
                        .find(|instruction| {
                            matches!(instruction.kind, SelectedInstructionKind::Load32 { .. })
                        })
                        .unwrap()
                        .operands[1]
                        .virtual_register;
                    function.virtual_registers[tag.0 as usize].definition_site =
                        Some(ValueDefinitionSite::FunctionParameter(0));
                }
                19 => {
                    let SelectedTerminator::Jump { successor, .. } =
                        &mut function.blocks[bridge].terminator
                    else {
                        unreachable!()
                    };
                    successor
                        .structural_case
                        .as_mut()
                        .unwrap()
                        .trivial_affine_discards
                        .push(PlaceId::new(99).unwrap());
                }
                20 => {
                    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
                        &mut function.blocks[dispatch].terminator
                    else {
                        unreachable!()
                    };
                    when_nonzero
                        .structural_case
                        .as_mut()
                        .unwrap()
                        .trivial_affine_discards
                        .push(PlaceId::new(99).unwrap());
                }
                _ => unreachable!(),
            }
            assert!(
                validate(changed).is_err(),
                "mutation {mutation} on {native:?}"
            );
        }
    }
}
