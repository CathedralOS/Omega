//! Array backing and descriptor contents are independently replayed.
use super::*;
use selected_instructions::SelectedInstructionKind;
use target_operations_to_selected_instructions::{
    legalize_target_operations, selection_constraints, stage_optimized_instruction_selection,
    validate_selected_instructions,
};

#[test]
fn fixed_array_target_rejects_changed_backing_extent_and_access() {
    let lowered = array_writer(3, true);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&lowered.semantic_module, &lowered.proof_bundle, target);
        let native = compiled.target_operations();
        let abstracted = compiled.optimized().plan();
        let unit = compiled.optimized();
        legalize_target_operations(native, abstracted, unit).unwrap();
        for mutation in 0..9 {
            let mut changed = native.clone();
            let caller = changed
                .functions
                .iter_mut()
                .find(|function| function.machine == lowered.semantic_module.entry)
                .unwrap();
            let body = &mut caller.graph;
            let argument = body
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| {
                    if let target_operations::TargetUnitOperation::Call { arguments, .. } =
                        operation
                    {
                        arguments
                            .iter_mut()
                            .find(|argument| argument.fixed_array_length.is_some())
                    } else {
                        None
                    }
                })
                .unwrap();
            match mutation {
                0 => argument.fixed_array_length = None,
                1 => argument.fixed_array_length = Some(0),
                2 => argument.fixed_array_length = Some(4),
                3 => argument.element_stride = Some(2),
                4 => argument.source_byte_offset += 1,
                5 => argument.path.clear(),
                6 => argument.access = terminal_psi::StructuralAccess::SharedBorrow,
                7 => argument.root_structural_type = argument.structural_type,
                8 => {
                    let terminal_psi::StructuralPathSegment::Field(selected) =
                        argument.path.last().unwrap()
                    else {
                        panic!("array field")
                    };
                    let fields = abstracted
                        .structural_types
                        .iter()
                        .find_map(|declaration| {
                            let terminal_psi::StructuralTypeShape::Record { fields } =
                                &declaration.shape
                            else {
                                return None;
                            };
                            fields
                                .iter()
                                .any(|field| field.identity == *selected)
                                .then_some(fields)
                        })
                        .unwrap();
                    let original = fields
                        .iter()
                        .find(|field| field.identity == *selected)
                        .unwrap();
                    let other = fields
                        .iter()
                        .find(|field| {
                            field.identity != *selected && field.field_type == original.field_type
                        })
                        .unwrap();
                    *argument.path.last_mut().unwrap() =
                        terminal_psi::StructuralPathSegment::Field(other.identity.clone());
                    argument.source_byte_offset += 3;
                }
                _ => unreachable!(),
            }
            assert!(
                legalize_target_operations(&changed, abstracted, unit).is_err(),
                "target mutation {mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn fixed_array_selection_rejects_changed_descriptor_contents_and_storage() {
    let lowered = array_writer(3, true);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&lowered.semantic_module, &lowered.proof_bundle, target);
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let staged = stage_optimized_instruction_selection(compiled, environment).unwrap();
        let environment = staged.register_environment();
        let constraints = selection_constraints(staged.legalized(), environment);
        let validate = |raw| {
            validate_selected_instructions(
                staged.legalized(),
                &constraints,
                environment.physical(),
                environment.constraints(),
                raw,
            )
        };
        validate(staged.selected().plan().clone()).unwrap();
        for mutation in 0..5 {
            let mut changed = staged.selected().plan().clone();
            let caller = changed
                .functions
                .iter_mut()
                .find(|function| function.machine == lowered.semantic_module.entry)
                .unwrap();
            if mutation == 0 {
                caller.local_storage_slots[0].byte_size = 8;
            } else {
                let instruction = caller
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|instruction| {
                        if mutation == 1 {
                            matches!(
                                instruction.kind,
                                SelectedInstructionKind::MaterializeI64 {
                                    value: semantic_vocabulary::IntegerValue::Unsigned(3)
                                }
                            )
                        } else {
                            matches!(
                                instruction.kind,
                                SelectedInstructionKind::Store64 { byte_offset: 0, .. }
                            )
                        }
                    })
                    .unwrap();
                match mutation {
                    1 => {
                        instruction.kind = SelectedInstructionKind::MaterializeI64 {
                            value: semantic_vocabulary::IntegerValue::Unsigned(4),
                        }
                    }
                    2 => instruction.operands.clear(),
                    3 => {
                        let SelectedInstructionKind::Store64 { byte_offset, .. } =
                            &mut instruction.kind
                        else {
                            unreachable!()
                        };
                        *byte_offset = 8;
                    }
                    4 => instruction.provenance.operations = vec![OperationId::new(9999).unwrap()],
                    _ => unreachable!(),
                }
            }
            assert!(
                validate(changed).is_err(),
                "selected mutation {mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn fixed_array_installation_rejects_changed_array_metadata() {
    let lowered = array_writer(3, true);
    let (image, _) = publish_lowered(NativeTarget::macos_arm64(), &lowered);
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    for mutation in 0..6 {
        let mut changed = record.clone();
        let call = changed
            .internal_unit_calls_mut_for_test()
            .iter_mut()
            .find(|call| call.machine == lowered.semantic_module.entry)
            .unwrap();
        let argument = &mut call.custody.arguments[0];
        match mutation {
            0 => argument.fixed_array_length = Some(2),
            1 => argument.fixed_array_length = Some(4),
            2 => argument.element_stride = Some(2),
            3 => argument.source_byte_offset += 1,
            4 => argument.path.clear(),
            5 => argument.access = terminal_psi::StructuralAccess::SharedBorrow,
            _ => unreachable!(),
        }
        assert!(
            image_emission::validate_installation_record(&changed, &image).is_err(),
            "installation mutation {mutation}"
        );
    }
}
