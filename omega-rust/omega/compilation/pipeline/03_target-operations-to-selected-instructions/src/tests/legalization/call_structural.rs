//! Crash-declaring structural-result calls carry their continuation roster.

use super::super::fixtures::call_structural::{
    fixture, mixed_result_fixture, reference_fixture, sum_reference_fixture,
};
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_legalized_operations, validate_selected_instructions,
};
use abstract_operations::AbstractStructuralCallResult;
use target_operations::TargetUnitOperation;

#[test]
fn owned_block_arguments_replay_destination_storage_across_call_abis() {
    use abstract_operations::{AbstractBlockEntry, AbstractOperation, AbstractStructuralBinding};
    use semantic_vocabulary::{BlockId, EdgeId, PlaceId};
    use target_operations::TargetStructuralArgumentSource;

    for length in [2, 3] {
        for native in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
            target::NativeTarget::windows_x64(),
        ] {
            let (mut source, _, original_unit) = fixture(native, Vec::new());
            let mut declarations = source.structural_types.to_vec();
            let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) =
                declarations[0].shape
            else {
                panic!("scalar field type")
            };
            declarations[1].identity = "test::Payload".into();
            declarations[1].shape = terminal_psi::StructuralTypeShape::Record {
                fields: (0..length)
                    .map(|ordinal| terminal_psi::StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(ordinal + 1).unwrap(),
                        identity: format!("word{ordinal}"),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(scalar_type),
                    })
                    .collect(),
            };
            source.structural_types = declarations.into();
            let mut parameter = source.functions[1].structural_parameters[0].clone();
            parameter.place = PlaceId::new(13).unwrap();
            let destination_block = BlockId::new(3).unwrap();
            let caller = &mut source.functions[0];
            let AbstractOperation::EstablishScalarArray { result, .. } = &caller.operations[1]
            else {
                panic!("array establishment")
            };
            // Two and three fields exercise register and indirect call ABIs
            // without changing the block-owned storage contract.
            caller.operations[1] = AbstractOperation::EstablishZeroedStructural {
                psi_operation: semantic_vocabulary::OperationId::new(11).unwrap(),
                result: result.clone(),
            };
            let AbstractOperation::CallStructural {
                structural_arguments,
                ..
            } = &mut caller.operations[2]
            else {
                panic!("call")
            };
            let incoming = structural_arguments[0].clone();
            structural_arguments[0].place = parameter.place;
            caller.operations.insert(
                2,
                AbstractOperation::Jump {
                    psi_edge: EdgeId::new(3).unwrap(),
                    target: destination_block,
                    bindings: Vec::new(),
                    structural_bindings: vec![AbstractStructuralBinding {
                        parameter: parameter.place,
                        argument: incoming,
                    }],
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            );
            caller.block_entries.push(AbstractBlockEntry {
                block: destination_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter.clone()],
                operation_offset: 3,
            });
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                original_unit.fuel_schedule,
            )
            .unwrap();
            for (function, original) in unit.functions.iter_mut().zip(&original_unit.functions) {
                function.verified_contract = original.verified_contract.clone();
            }
            unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
            optimization_unit_semantics::validate_psi_optimization_unit(&unit)
                .expect("owned arrival fixture has valid source custody");
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            let mut substituted = selected.plan().clone();
            let address = substituted.functions[0].blocks.iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| matches!(instruction.kind,
                    selected_instructions::SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Local(
                            selected_instructions::LocalStorageSlotId::StructuralBlockParameter { .. }
                        ), ..
                    }))
                .expect("call addresses its block-owned home");
            address.kind = selected_instructions::SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::Local(
                    selected_instructions::LocalStorageSlotId::Structural {
                        operation: semantic_vocabulary::OperationId::new(11).unwrap(),
                        place: PlaceId::new(11).unwrap(),
                    },
                ),
                byte_offset: 0,
            };
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    substituted,
                )
                .is_err(),
                "an incoming home cannot replace the destination home"
            );
            for mutation in 0..4 {
                let mut changed = target.clone();
                let argument = changed.functions[0]
                    .graph
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find_map(|operation| match operation {
                        TargetUnitOperation::Call { arguments, .. } => arguments.first_mut(),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    argument.source,
                    TargetStructuralArgumentSource::BlockParameter {
                        block: destination_block,
                        place: parameter.place,
                    }
                );
                match mutation {
                    0 => {
                        argument.source = TargetStructuralArgumentSource::BlockParameter {
                            block: BlockId::new(1).unwrap(),
                            place: parameter.place,
                        }
                    }
                    1 => {
                        argument.source = TargetStructuralArgumentSource::BlockParameter {
                            block: destination_block,
                            place: PlaceId::new(11).unwrap(),
                        }
                    }
                    2 => {
                        argument.source = TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: semantic_vocabulary::OperationId::new(11).unwrap(),
                        }
                    }
                    _ => argument.source_byte_offset = 8,
                }
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{native:?}, length {length}, source mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn owned_record_payload_arguments_replay_root_relative_offsets() {
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::{OperationId, PlaceId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, RecordFieldInitializer, RecordFieldValue, StructuralAccess,
        StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    for payload_length in [2, 3] {
        for native in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
            target::NativeTarget::windows_x64(),
        ] {
            let (mut source, _, original_unit) = fixture(native, Vec::new());
            let mut types = source.structural_types.to_vec();
            let StructuralTypeShape::FixedArray { length, .. } = &mut types[1].shape else {
                unreachable!()
            };
            *length = payload_length;
            let root_type = StructuralTypeId::new(3).unwrap();
            types.push(StructuralTypeDeclaration {
                id: root_type,
                identity: "test::Envelope".into(),
                shape: StructuralTypeShape::Record {
                    fields: ["prefix", "payload"]
                        .into_iter()
                        .enumerate()
                        .map(|(ordinal, name)| StructuralFieldDeclaration {
                            id: StructuralFieldId::new(ordinal as u64 + 1).unwrap(),
                            identity: name.into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(
                                StructuralTypeId::new(2).unwrap(),
                            ),
                        })
                        .collect(),
                },
            });
            source.structural_types = types.into();
            let operations = &mut source.functions[0].operations;
            let AbstractOperation::EstablishScalarArray {
                result, elements, ..
            } = &mut operations[1]
            else {
                unreachable!()
            };
            elements.resize(payload_length as usize, elements[0]);
            let mut root_result = result.clone();
            root_result.place = PlaceId::new(13).unwrap();
            root_result.structural_type = root_type;
            root_result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
            operations[2] = AbstractOperation::CallStructural {
                psi_operation: OperationId::new(12).unwrap(),
                callee: semantic_vocabulary::MachineId::new(2).unwrap(),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: PlaceId::new(14).unwrap(),
                    path: vec![StructuralPathSegment::Field("payload".into())],
                    access: StructuralAccess::Owned,
                }],
                dynamic_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                result: AbstractStructuralCallResult::Unit,
            };
            operations[3] = AbstractOperation::Jump {
                psi_edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
                target: semantic_vocabulary::BlockId::new(4).unwrap(),
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: vec![terminal_psi::StructuralAffineDiscard {
                    place: PlaceId::new(14).unwrap(),
                    path: vec![StructuralPathSegment::Field("prefix".into())],
                    structural_type: StructuralTypeId::new(2).unwrap(),
                }],
            };
            operations.insert(
                2,
                AbstractOperation::EstablishRecord {
                    psi_operation: OperationId::new(13).unwrap(),
                    result: root_result.clone(),
                    fields: [1, 2]
                        .into_iter()
                        .map(|field| RecordFieldInitializer {
                            field: StructuralFieldId::new(field).unwrap(),
                            value: RecordFieldValue::Structural(StructuralArgument {
                                place: PlaceId::new(11).unwrap(),
                                path: Vec::new(),
                                access: StructuralAccess::Owned,
                            }),
                        })
                        .collect(),
                },
            );
            root_result.place = PlaceId::new(14).unwrap();
            operations.insert(
                3,
                AbstractOperation::CallStructural {
                    psi_operation: OperationId::new(14).unwrap(),
                    result: AbstractStructuralCallResult::Structural {
                        result: root_result,
                        returned_claim_transfers: Vec::new(),
                        selected_evidence: Vec::new(),
                    },
                    callee: semantic_vocabulary::MachineId::new(3).unwrap(),
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: PlaceId::new(13).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }],
                    dynamic_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            );
            let operation_offset = operations.len();
            operations.push(AbstractOperation::ReturnUnit {
                psi_edge: semantic_vocabulary::EdgeId::new(4).unwrap(),
                cleanup_actions: Vec::new(),
            });
            source.functions[0]
                .block_entries
                .push(abstract_operations::AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: semantic_vocabulary::BlockId::new(4).unwrap(),
                    parameters: Vec::new(),
                    operation_offset,
                });
            let mut forward = source.functions[1].clone();
            forward.machine = semantic_vocabulary::MachineId::new(3).unwrap();
            forward.structural_parameters[0].structural_type = root_type;
            forward.structural_parameters[0].multiplicity =
                terminal_psi::StructuralMultiplicity::Affine;
            let abstract_operations::AbstractFunctionResult::Structural(result) =
                &mut forward.result
            else {
                unreachable!()
            };
            result.structural_type = root_type;
            result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
            source.functions.push(forward);
            source.functions[0].result = abstract_operations::AbstractFunctionResult::Unit;
            source.functions[1].result = abstract_operations::AbstractFunctionResult::Unit;
            source.functions[1].structural_parameters[0].multiplicity =
                terminal_psi::StructuralMultiplicity::Affine;
            source.functions[1].operations = vec![AbstractOperation::ReturnUnit {
                psi_edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
                cleanup_actions: vec![terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                    PlaceId::new(20).unwrap(),
                )],
            }];
            let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            for function in &mut unit.functions {
                function.verified_contract = original_unit.functions[0].verified_contract.clone();
            }
            unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let argument = target.functions[0].graph.blocks[0]
                .operations
                .iter()
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call {
                        arguments,
                        psi_operation,
                        ..
                    } if psi_operation.get() == 12 => arguments.first(),
                    _ => None,
                })
                .unwrap();
            assert_eq!(argument.root_structural_type, root_type);
            assert_eq!(u64::from(argument.source_byte_offset), payload_length * 8);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            let reads = selected.plan().functions[0]
                .memory_accesses
                .iter()
                .filter(|access| {
                    access.place == PlaceId::new(14).unwrap()
                        && access.role == selected_instructions::SelectedMemoryAccessRole::ReadPlace
                })
                .map(|access| (u64::from(access.byte_offset), access.byte_count))
                .collect::<Vec<_>>();
            assert_eq!(
                reads,
                (payload_length..2 * payload_length)
                    .map(|element| (element * 8, 8))
                    .collect::<Vec<_>>()
            );
            let mut wrong_read = selected.plan().clone();
            wrong_read.functions[0]
                .memory_accesses
                .iter_mut()
                .find(|access| {
                    access.place == PlaceId::new(14).unwrap()
                        && access.role == selected_instructions::SelectedMemoryAccessRole::ReadPlace
                })
                .unwrap()
                .byte_offset = 0;
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    wrong_read,
                )
                .is_err()
            );
            let mut missing_cleanup = target.clone();
            let target_operations::TargetControlTerminator::Jump { successor } =
                &mut missing_cleanup.functions[0].graph.blocks[0].terminator
            else {
                unreachable!()
            };
            successor.cleanup_actions.clear();
            assert!(legalize_target_operations(&missing_cleanup, &source, &unit).is_err());
            for mutation in ["offset", "root", "path"] {
                let mut hostile = target.clone();
                let argument = hostile.functions[0].graph.blocks[0]
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        TargetUnitOperation::Call {
                            arguments,
                            psi_operation,
                            ..
                        } if psi_operation.get() == 12 => arguments.first_mut(),
                        _ => None,
                    })
                    .unwrap();
                match mutation {
                    "offset" => argument.source_byte_offset = 0,
                    "root" => argument.root_structural_type = argument.structural_type,
                    "path" => argument.path = vec![StructuralPathSegment::Field("prefix".into())],
                    _ => unreachable!(),
                }
                assert!(
                    legalize_target_operations(&hostile, &source, &unit).is_err(),
                    "{mutation}"
                );
            }
        }
    }
}

#[test]
fn crash_declaring_structural_calls_select_and_replay_their_continuation_roster() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = fixture(
            native,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }],
        );
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        assert_eq!(
            call.call.crash_continuations,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }]
        );
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The roster is correspondence custody: a forged or dropped
        // continuation rejects at every replay level.
        let mut widened = target.clone();
        let TargetUnitOperation::Call {
            crash_continuations,
            ..
        } = widened.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        crash_continuations.clear();
        assert!(legalize_target_operations(&widened, &source, &unit).is_err());
        let mut changed = legal.plan().clone();
        let changed_call = changed
            .scalar_functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| block.instructions.iter_mut())
            .find_map(|instruction| {
                if let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &mut instruction.kind
                {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("call instruction");
        changed_call.crash_continuations.clear();
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        let mut forged = selected.plan().clone();
        forged.functions[0].calls[0]
            .call
            .crash_continuations
            .clear();
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                forged
            )
            .is_err()
        );
        // A caller-side roster mismatch against the callee's verified crash
        // contract fails unit-custody before any instruction projects.
        let mut contract_changed = unit.clone();
        contract_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .clear();
        contract_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&contract_changed);
        assert!(legalize_target_operations(&target, &source, &contract_changed).is_err());
    }
}

#[test]
fn crash_declaring_reference_calls_select_and_replay_their_continuation_roster() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = reference_fixture(
            native,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }],
        );
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        assert_eq!(
            call.call.crash_continuations,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }]
        );
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The roster is correspondence custody: a forged or dropped
        // continuation rejects at every replay level.
        let mut widened = target.clone();
        let TargetUnitOperation::Call {
            crash_continuations,
            ..
        } = widened.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        crash_continuations.clear();
        assert!(legalize_target_operations(&widened, &source, &unit).is_err());
        let mut changed = legal.plan().clone();
        let changed_call = changed
            .scalar_functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| block.instructions.iter_mut())
            .find_map(|instruction| {
                if let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &mut instruction.kind
                {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("call instruction");
        changed_call.crash_continuations.clear();
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        let mut forged = selected.plan().clone();
        forged.functions[0].calls[0]
            .call
            .crash_continuations
            .clear();
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                forged
            )
            .is_err()
        );
        // A caller-side roster mismatch against the callee's verified crash
        // contract fails unit-custody before any instruction projects.
        let mut contract_changed = unit.clone();
        contract_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .clear();
        contract_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&contract_changed);
        assert!(legalize_target_operations(&target, &source, &contract_changed).is_err());
        // A forged roster on the call itself fails the same contract pairing.
        let mut roster_changed = unit.clone();
        roster_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .push(terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Trap,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            });
        roster_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&roster_changed);
        assert!(legalize_target_operations(&target, &source, &roster_changed).is_err());
    }
}

#[test]
fn crash_declaring_sum_reference_calls_select_and_replay_their_continuation_roster() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = sum_reference_fixture(
            native,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }],
        );
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        assert_eq!(
            call.call.crash_continuations,
            vec![terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Abort,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            }]
        );
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The roster is correspondence custody: a forged or dropped
        // continuation rejects at every replay level.
        let mut widened = target.clone();
        let TargetUnitOperation::Call {
            crash_continuations,
            ..
        } = widened.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        crash_continuations.clear();
        assert!(legalize_target_operations(&widened, &source, &unit).is_err());
        let mut changed = legal.plan().clone();
        let changed_call = changed
            .scalar_functions
            .iter_mut()
            .flat_map(|function| function.blocks.iter_mut())
            .flat_map(|block| block.instructions.iter_mut())
            .find_map(|instruction| {
                if let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &mut instruction.kind
                {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("call instruction");
        changed_call.crash_continuations.clear();
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        let mut forged = selected.plan().clone();
        forged.functions[0].calls[0]
            .call
            .crash_continuations
            .clear();
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                forged
            )
            .is_err()
        );
        // A caller-side roster mismatch against the callee's verified crash
        // contract fails unit-custody before any instruction projects.
        let mut contract_changed = unit.clone();
        contract_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .clear();
        contract_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&contract_changed);
        assert!(legalize_target_operations(&target, &source, &contract_changed).is_err());
        let mut roster_changed = unit.clone();
        roster_changed.functions[1]
            .verified_contract
            .as_mut()
            .expect("contract")
            .crash_routes
            .push(terminal_psi::CrashRouteBucket {
                cause: terminal_psi::CrashCause::Trap,
                alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
            });
        roster_changed.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&roster_changed);
        assert!(legalize_target_operations(&target, &source, &roster_changed).is_err());
    }
}

/// A Mixed carrier returned through a structural call reuses the conventional
/// sum result home; each layer replays that exact transport.
#[test]
fn mixed_structural_results_select_and_replay_their_result_home() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = mixed_result_fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let call = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.calls)
            .next()
            .expect("structural call");
        let result = call.call.structural_result.as_ref().expect("mixed result");
        assert_eq!(
            result.structural_type,
            semantic_vocabulary::StructuralTypeId::new(1).unwrap()
        );
        assert_eq!(
            call.call
                .call_plan
                .result
                .as_ref()
                .expect("result")
                .shape
                .byte_size,
            12
        );
        let TargetUnitOperation::Call {
            result:
                target_operations::TargetCallResult::Structural {
                    result_home: Some(home),
                    ..
                },
            ..
        } = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| block.operations.iter())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        assert!(matches!(
            home.layout,
            target_operations::TargetStructuralHomeLayout::Sum(_)
        ));
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The incoming value has the same type and layout, but cannot replace
        // the completed call's storage identity on the return edge.
        let mut substituted_home = selected.plan().clone();
        let returned_home = substituted_home.functions[0]
            .memory_accesses
            .iter_mut()
            .find(|access| {
                matches!(
                    access.role,
                    selected_instructions::SelectedMemoryAccessRole::AddressLocal {
                        slot: selected_instructions::LocalStorageSlotId::Structural { .. }
                    }
                ) && matches!(
                    access.origin,
                    selected_instructions::SelectedMemoryAccessOrigin::Block(_)
                )
            })
            .expect("return addresses the completed mixed result");
        assert_eq!(returned_home.place, result.place);
        returned_home.place = semantic_vocabulary::PlaceId::new(10).unwrap();
        returned_home.role = selected_instructions::SelectedMemoryAccessRole::AddressLocal {
            slot: selected_instructions::LocalStorageSlotId::StructuralParameter {
                place: returned_home.place,
            },
        };
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                substituted_home,
            )
            .is_err()
        );
        // Dropping or substituting the agreed result home also rejects at
        // legalization, before selection consumes the completed value.
        let mut no_home = target.clone();
        let TargetUnitOperation::Call {
            result: target_operations::TargetCallResult::Structural { result_home, .. },
            ..
        } = no_home.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        *result_home = None;
        assert!(legalize_target_operations(&no_home, &source, &unit).is_err());
        let mut wrong_type = target.clone();
        let TargetUnitOperation::Call {
            result: target_operations::TargetCallResult::Structural { result, .. },
            ..
        } = wrong_type.functions[0]
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .expect("call")
        else {
            unreachable!("call");
        };
        result.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
        assert!(legalize_target_operations(&wrong_type, &source, &unit).is_err());
    }
}
