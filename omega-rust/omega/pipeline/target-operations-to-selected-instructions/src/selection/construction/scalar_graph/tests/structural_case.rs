//! Lawfully legalized cases select through ordinary control and edge bridges.
use super::*;
use selected_instructions::{
    SelectedBlockOrigin, SelectedCasePayloadTransport, SelectedSuccessorRole,
};

#[test]
fn joined_structural_return_rejects_owner_declaration_and_abi_substitution() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let (abstracted, targeted, unit) =
            crate::tests::legalization::structural_case::fixture(native);
        let legal = crate::legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let mut source = legal.plan().scalar_functions[0].clone();
        let LegalizedScalarTerminator::StructuralCase {
            source: subject,
            layout,
            ..
        } = &source.blocks[0].terminator
        else {
            panic!("case source");
        };
        let shape = layout.shape;
        let declaration = terminal_psi::StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(99).unwrap(),
            position: 0,
            is_self: false,
            structural_type: subject.structural_type(),
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let block = source.blocks[1].id;
        source.blocks[1]
            .structural_parameters
            .push(declaration.clone());
        source.structural.as_mut().unwrap().result =
            Some(terminal_psi::StructuralResultDeclaration {
                place: semantic_vocabulary::PlaceId::new(100).unwrap(),
                structural_type: declaration.structural_type,
                multiplicity: declaration.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        source.call_plan.result = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: Vec::new(),
                result: Some(shape),
            },
        )
        .unwrap()
        .result;
        let returned = LegalizedScalarReturnValue::Structural {
            source: legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration,
            },
        };
        let accepted =
            crate::selection::aggregate_result_input::returned(&source, &returned).unwrap();
        assert_eq!(
            accepted.0,
            selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                block,
                place: semantic_vocabulary::PlaceId::new(99).unwrap(),
            }
        );
        for mutation in [
            "owner",
            "place",
            "position",
            "access",
            "multiplicity",
            "type",
            "abi",
        ] {
            let mut changed_source = source.clone();
            let mut changed = returned.clone();
            let LegalizedScalarReturnValue::Structural {
                source:
                    legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                        block,
                        declaration,
                    },
            } = &mut changed
            else {
                panic!("block owner");
            };
            match mutation {
                "owner" => *block = changed_source.blocks[0].id,
                "place" => declaration.place = semantic_vocabulary::PlaceId::new(98).unwrap(),
                "position" => declaration.position += 1,
                "access" => declaration.access = terminal_psi::StructuralAccess::SharedBorrow,
                "multiplicity" => {
                    declaration.multiplicity = terminal_psi::StructuralMultiplicity::Linear
                }
                "type" => declaration.structural_type = StructuralTypeId::new(98).unwrap(),
                "abi" => {
                    changed_source
                        .call_plan
                        .result
                        .as_mut()
                        .unwrap()
                        .shape
                        .byte_size += 1
                }
                _ => unreachable!(),
            }
            assert!(
                crate::selection::aggregate_result_input::returned(&changed_source, &changed)
                    .is_none(),
                "accepted {mutation}"
            );
        }
    }
}

#[test]
fn structural_case_unused_payload_retains_direct_edge_metadata_without_load() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let (mut abstracted, _, _) = crate::tests::legalization::structural_case::fixture(native);
        let removed = abstracted.functions[0].operations.remove(4);
        assert!(matches!(
            removed,
            abstract_operations::AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Unit,
                ..
            }
        ));
        abstracted.boundary_machines.truncate(1);
        let targeted = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
            &abstracted, native, &[abstract_operations_to_target_operations::AdmittedBoundarySettlement {
                boundary: abstracted.boundary_machines[0].id,
                execution: abstract_operations_to_target_operations::AdmittedBoundaryExecution::CompilerBuiltin(target_operations::CompilerBuiltinExecution::HostedReadByte),
                realization: target_operations::HostedReadByteRealization.into(),
            }],
        ).unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstracted,
            semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let legal = crate::legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let function = &selected.plan().functions[0];
        assert!(
            function
                .blocks
                .iter()
                .all(|block| matches!(block.origin, SelectedBlockOrigin::Source(_)))
        );
        let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
            &function.blocks[0].terminator
        else {
            panic!("case")
        };
        let case = when_nonzero.structural_case.as_ref().unwrap();
        assert_eq!(case.payloads.len(), 1);
        assert_eq!(
            case.payloads[0].transport,
            SelectedCasePayloadTransport::Unused
        );
        assert_eq!(case.trivial_affine_discards.len(), 1);
        assert!(!when_nonzero.fuel.is_empty());
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|instruction| matches!(
                    instruction.kind,
                    SelectedInstructionKind::Load32 { .. }
                ))
                .count(),
            1,
            "only the tag is read"
        );
    }
}

#[test]
fn structural_case_selects_tag_and_edge_payload_without_fabricated_values() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let (abstracted, targeted, unit) =
            crate::tests::legalization::structural_case::fixture(native);
        let legal = crate::legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let function = &selected.plan().functions[0];
        let entry = function
            .blocks
            .iter()
            .find(|block| block.id == function.entry_block)
            .unwrap();
        assert!(matches!(
            entry.instructions.as_slice(),
            [
                SelectedInstruction {
                    kind: SelectedInstructionKind::HostedReadByte { .. },
                    ..
                },
                SelectedInstruction {
                    kind: SelectedInstructionKind::FrameAddress { .. },
                    ..
                },
                SelectedInstruction {
                    kind: SelectedInstructionKind::Load32 { byte_offset: 0 },
                    ..
                },
                SelectedInstruction {
                    kind: SelectedInstructionKind::CompareI64Zero,
                    ..
                }
            ]
        ));
        let SelectedTerminator::ConditionalBranch {
            when_zero,
            when_nonzero,
            ..
        } = &entry.terminator
        else {
            panic!("case branch")
        };
        assert_eq!(when_zero.structural_case.as_ref().unwrap().case_tag, 0);
        assert_eq!(when_nonzero.structural_case.as_ref().unwrap().case_tag, 1);
        assert_eq!(when_nonzero.role, SelectedSuccessorRole::Semantic);
        assert!(
            when_nonzero
                .structural_case
                .as_ref()
                .unwrap()
                .payloads
                .iter()
                .all(|payload| payload.transport == SelectedCasePayloadTransport::Unused)
        );
        let bridge = function
            .blocks
            .iter()
            .find(|block| block.id == when_nonzero.block)
            .unwrap();
        assert!(
            matches!(bridge.origin, SelectedBlockOrigin::EdgeTransfer { edge, .. } if edge == when_nonzero.psi_edge)
        );
        assert!(matches!(
            bridge.instructions.as_slice(),
            [
                SelectedInstruction {
                    kind: SelectedInstructionKind::FrameAddress { .. },
                    ..
                },
                SelectedInstruction {
                    kind: SelectedInstructionKind::Load32 { byte_offset: 4 },
                    ..
                }
            ]
        ));
        let SelectedTerminator::Jump { successor, .. } = &bridge.terminator else {
            panic!("edge continuation")
        };
        assert!(successor.fuel.is_empty());
        assert!(
            successor
                .structural_case
                .as_ref()
                .unwrap()
                .trivial_affine_discards
                .is_empty()
        );
        assert_eq!(
            successor.role,
            SelectedSuccessorRole::EdgeTransferContinuation
        );
        let payload = &successor.structural_case.as_ref().unwrap().payloads[0];
        let SelectedCasePayloadTransport::Registers {
            argument,
            parameter,
        } = payload.transport
        else {
            panic!("loaded payload")
        };
        let loaded = &function.virtual_registers[argument.0 as usize];
        assert!(matches!(
            loaded.origin,
            VirtualRegisterOrigin::StructuralObservation { byte_offset: 4, .. }
        ));
        assert_eq!(loaded.definition_site, None);
        assert_eq!(
            function.virtual_registers[parameter.0 as usize].definition_site,
            Some(payload.semantic.parameter.definition_site)
        );
        assert!(
            entry.instructions[1..]
                .iter()
                .chain(&bridge.instructions)
                .all(|instruction| instruction.provenance.operations.is_empty()
                    && instruction.provenance.values.is_empty()
                    && instruction.provenance.fuel.is_empty())
        );
        assert_eq!(function.local_storage_slots.len(), 2);
        assert_eq!(
            function
                .local_storage_slots
                .iter()
                .filter(|slot| matches!(
                    slot.id,
                    selected_instructions::LocalStorageSlotId::Structural { .. }
                ) && slot.byte_size == 8
                    && slot.alignment == 4)
                .count(),
            1
        );
        assert_eq!(
            function
                .local_storage_slots
                .iter()
                .filter(|slot| matches!(
                    slot.id,
                    selected_instructions::LocalStorageSlotId::Boundary { .. }
                ) && slot.byte_size == 1
                    && slot.alignment == 1)
                .count(),
            1
        );
    }
}
