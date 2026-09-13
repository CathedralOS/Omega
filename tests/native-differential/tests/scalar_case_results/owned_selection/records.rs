//! Record joins lend their own storage, with independent source and address replay.
use super::super::*;

const SOURCE: &str =
    include_str!("../../../../omega/pass/expressions/owned_match_record_values/main.omg");

#[test]
fn joined_record_cannot_move_and_lend_its_child_to_the_same_call() {
    // Reuse the working projected getter, then give it an independently owned
    // argument. This control does not require source local-copy call lowering.
    let artifact = produce_source("choose", SOURCE);
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let incoming_owner = semantic_vocabulary::PlaceId::new(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.structural_places)
            .map(|place| place.id.get())
            .max()
            .unwrap()
            + 1,
    )
    .unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let (call_operation, callee, projected) = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallStructuralScalar {
                callee,
                structural_arguments,
                ..
            } => {
                assert_eq!(structural_arguments.len(), 1);
                Some((operation.id, *callee, structural_arguments[0].clone()))
            }
            _ => None,
        })
        .unwrap();
    let getter = module
        .machines
        .iter()
        .find(|machine| machine.id == callee)
        .unwrap();
    assert_eq!(getter.structural_parameters.len(), 1);
    let borrowed_type = getter.structural_parameters[0].structural_type;
    assert_eq!(
        projected.access,
        terminal_psi::StructuralAccess::SharedBorrow
    );
    assert_eq!(
        projected.path,
        vec![terminal_psi::StructuralPathSegment::Field("payload".into())]
    );
    let join = caller
        .blocks
        .iter()
        .find(|block| {
            block
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == projected.place)
        })
        .unwrap();
    let selected = join
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == projected.place)
        .unwrap()
        .clone();
    assert_eq!(selected.access, terminal_psi::StructuralAccess::Owned);
    assert_eq!(
        selected.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine
    );
    let mut markers = join
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.structural_type == borrowed_type);
    let marker = markers.next().unwrap().place;
    assert!(
        markers.next().is_none(),
        "the joined marker has one exact owner"
    );
    assert_ne!(marker, selected.place);

    let caller = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let arguments = caller
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } if operation.id == call_operation => Some(structural_arguments),
            _ => None,
        })
        .unwrap();
    arguments[0].place = marker;
    arguments[0].path.clear();
    arguments.push(terminal_psi::StructuralArgument {
        place: selected.place,
        path: Vec::new(),
        access: terminal_psi::StructuralAccess::Owned,
    });
    let mut removed_cleanup = 0;
    for block in &mut caller.blocks {
        if let terminal_psi::Terminator::Return {
            cleanup_actions, ..
        } = &mut block.terminator
        {
            cleanup_actions.retain(|action| {
                let transferred = *action
                    == terminal_psi::TerminalAffineCleanupAction::DiscardRoot(selected.place);
                removed_cleanup += usize::from(transferred);
                !transferred
            });
        }
    }
    assert_eq!(
        removed_cleanup, 1,
        "the caller transfers its selected cleanup obligation"
    );
    let getter = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == callee)
        .unwrap();
    getter
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: incoming_owner,
            position: 1,
            is_self: false,
            ..selected
        });
    getter
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: incoming_owner,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let mut added_cleanup = 0;
    for block in &mut getter.blocks {
        if let terminal_psi::Terminator::Return {
            cleanup_actions, ..
        } = &mut block.terminator
        {
            assert!(cleanup_actions.is_empty());
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                incoming_owner,
            ));
            added_cleanup += 1;
        }
    }
    assert_eq!(
        added_cleanup, 1,
        "the getter discharges its unused owned parameter"
    );

    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("Terminal control moves the selected record and borrows the distinct marker");
    let semantic_bytes = terminal_codec::encode_module(&module).unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic_bytes,
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
    )
    .expect("the valid Terminal control admits to current IR");
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(verified.unit())
        .expect("current-IR control retains distinct moved and borrowed owners");

    let mut changed_module = module.clone();
    let arguments = changed_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } if operation.id == call_operation => Some(structural_arguments),
            _ => None,
        })
        .unwrap();
    assert_eq!(arguments[1].place, projected.place);
    arguments[0] = projected.clone();
    let error =
        terminal_verifier::verify_module(&changed_module, &proof, &AdmissionProfile::default())
            .expect_err("a transferred join cannot supply an overlapping loan");
    assert!(
        matches!(error, terminal_verifier::VerificationError::Module(
            terminal_verifier::ModuleError::OverlappingExclusiveStructuralArguments { operation, .. }
        ) if operation == call_operation),
        "expected call overlap rejection, got {error:?}"
    );

    let mut changed = verified.unit().clone();
    let arguments = changed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            abstract_operations::AbstractOperation::CallStructuralScalar {
                psi_operation,
                structural_arguments,
                ..
            } if *psi_operation == call_operation => Some(structural_arguments),
            _ => None,
        })
        .unwrap();
    assert_eq!(arguments[0].place, marker);
    assert_eq!(arguments[1].place, projected.place);
    arguments[0] = projected;
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    assert!(
        optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err(),
        "current IR must reject a moved-and-borrowed joined record"
    );
}

#[test]
fn nested_record_selection_publishes_on_every_hosted_target() {
    let artifact = produce_source("choose", SOURCE);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
}

#[test]
fn fresh_first_record_arm_keeps_saved_fields_across_child_dispatch() {
    let source = SOURCE
        .replace("        0 -> left,\n        1 -> right,\n", "")
        .replace(
            "_ -> Envelope {",
            "0 -> Envelope {\n            leading: first,",
        )
        .replace(
            "            },\n            leading: first\n        }",
            "            }\n        },\n        1 -> right,\n        _ -> left",
        );
    // Observe the scalar saved before the child's dispatch as well as the child.
    let source = format!(
        "machine Envelope::get_leading(&self) -> u64 {{ self.leading }}\n{}",
        source.replace(
            "result.payload.get_right() ^ marker.left",
            "result.payload.get_right() ^ marker.left ^ result.get_leading()"
        )
    );
    let artifact = produce_source("choose", &source);
    membership::execute(
        &artifact,
        r#"
        #include <stdbool.h>
        #include <stdint.h>
        extern uint64_t omega_entry(uint64_t selected, bool inner, uint64_t first, uint64_t second);
        int main(void) {
            uint64_t first = UINT64_C(0x8123456789abcdef);
            uint64_t second = UINT64_C(0xfedcba9876543210);
            for (uint64_t selected = 0; selected < 4; ++selected) {
                for (unsigned inner = 0; inner < 2; ++inner) {
                    uint64_t expected = selected == 0 ? (inner ? first : second) ^ 255
                        : selected == 1 ? first : second;
                    uint64_t leading = selected == 1 ? second : first;
                    if (omega_entry(selected, inner, first, second) != (expected ^ 37 ^ leading)) return 1;
                }
            }
            return 0;
        }
    "#,
    );
}

#[test]
fn selected_record_loan_rejects_changed_origin_geometry_and_home() {
    let artifact = produce_source("choose", SOURCE);
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(verified.unit()).unwrap();
    for mutation in 0..3 {
        let mut changed = verified.unit().clone();
        let argument = changed
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.blocks)
            .flat_map(|block| &mut block.nodes)
            .find_map(|node| match &mut node.operation {
                abstract_operations::AbstractOperation::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => structural_arguments.first_mut(),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => argument.path.clear(),
            1 => argument.access = terminal_psi::StructuralAccess::MutableBorrow,
            _ => argument.path = vec![terminal_psi::StructuralPathSegment::Field("leading".into())],
        }
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err(),
            "current-IR record loan mutation {mutation}"
        );
    }
    let target = NativeTarget::macos_arm64();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let compiled = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized, target,
    )
    .unwrap();
    for mutation in 0..4 {
        let mut changed = compiled.target_operations().clone();
        let argument = changed
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.graph.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match operation {
                target_operations::TargetUnitOperation::StructuralScalarCall {
                    arguments, ..
                } => arguments.first_mut(),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => argument.source_byte_offset = 0,
            1 => argument.path.clear(),
            2 => {
                let target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block, ..
                } = &mut argument.source
                else {
                    panic!("expected joined record home")
                };
                *block = semantic_vocabulary::BlockId::new(9999).unwrap();
            }
            _ => argument.access = terminal_psi::StructuralAccess::MutableBorrow,
        }
        assert!(
            target_operations_to_selected_instructions::legalize_target_operations(
                &changed,
                compiled.optimized().plan(),
                compiled.optimized()
            )
            .is_err(),
            "target record loan mutation {mutation}"
        );
    }
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    let staged = target_operations_to_selected_instructions::stage_optimized_instruction_selection(
        compiled,
        environment,
    )
    .unwrap();
    let environment = staged.register_environment();
    let constraints = target_operations_to_selected_instructions::selection_constraints(
        staged.legalized(),
        environment,
    );
    for mutation in 0..3 {
        let mut changed = staged.selected().plan().clone();
        let function = changed.functions.iter_mut().find(|function| function.calls.iter().any(|call|
            call.call.arguments.iter().any(|argument| matches!(argument,
                legalized_operations::LegalizedScalarArgument::Structural { target, .. }
                if matches!(target.source, target_operations::TargetStructuralArgumentSource::BlockParameter { .. }))))).unwrap();
        match mutation {
            0 => {
                let instruction = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|instruction| {
                        matches!(
                            instruction.kind,
                            selected_instructions::SelectedInstructionKind::AddressOffset {
                                byte_offset: 8
                            }
                        )
                    })
                    .unwrap();
                instruction.kind = selected_instructions::SelectedInstructionKind::AddressOffset {
                    byte_offset: 0,
                };
            }
            1 => {
                let slot = function.local_storage_slots.iter_mut().find(|slot|
                    matches!(slot.id, selected_instructions::LocalStorageSlotId::StructuralBlockParameter { .. }) && slot.byte_size == 24).unwrap();
                slot.byte_size = 16;
            }
            _ => {
                let instruction = function.blocks.iter_mut().flat_map(|block| &mut block.instructions)
                    .find(|instruction| matches!(instruction.kind, selected_instructions::SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Local(selected_instructions::LocalStorageSlotId::StructuralBlockParameter { .. }), .. })).unwrap();
                let selected_instructions::SelectedInstructionKind::FrameAddress {
                    slot:
                        selected_instructions::FrameStorageSlotId::Local(
                            selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                                block,
                                ..
                            },
                        ),
                    ..
                } = &mut instruction.kind
                else {
                    unreachable!()
                };
                *block = semantic_vocabulary::BlockId::new(9999).unwrap();
            }
        }
        assert!(
            target_operations_to_selected_instructions::validate_selected_instructions(
                staged.legalized(),
                &constraints,
                environment.physical(),
                environment.constraints(),
                changed
            )
            .is_err(),
            "selected record storage mutation {mutation}"
        );
    }
}
