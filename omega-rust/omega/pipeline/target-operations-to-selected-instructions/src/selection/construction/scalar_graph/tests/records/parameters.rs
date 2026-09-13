//! An owned incoming record is ordinary initialized child storage.
use super::*;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

#[test]
fn borrowed_owned_input_returns_current_home_instead_of_entry_fragments() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = record_fixture(native, 2);
        let LegalizedScalarInstructionKind::EstablishRecord { result, shape, .. } =
            source.blocks[0].instructions[2].kind.clone()
        else {
            panic!("record");
        };
        let place = result.place;
        let structural_type = result.structural_type;
        source.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: vec![shape],
                result: Some(shape),
            },
        )
        .unwrap();
        let input = source.call_plan.parameters[0].clone();
        let signature = source.structural.as_mut().unwrap();
        signature.structural_places[0].kind = StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        };
        signature.result.as_mut().unwrap().multiplicity = StructuralMultiplicity::Affine;
        signature.parameters = vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: terminal_psi::StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            target: target_operations::TargetStructuralParameter {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                projected_qualifications: Vec::new(),
                shape,
                placement: input.clone(),
            },
        }];
        let borrowed = ValueShape::borrowed_reference(shape.byte_size, shape.alignment);
        let call_plan = evaluate_call_plan(
            source.call_plan.policy,
            &CallSignature {
                parameters: vec![borrowed],
                result: None,
            },
        )
        .unwrap();
        let row = &mut source.blocks[0].instructions[2];
        row.ownership = vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())];
        row.kind = LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            source: legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit,
            callee: MachineId::new(10).unwrap(),
            arguments: vec![LegalizedScalarArgument::Structural {
                semantic: terminal_psi::StructuralArgument {
                    place,
                    access: StructuralAccess::MutableBorrow,
                    path: Vec::new(),
                },
                target: target_operations::TargetStructuralArgument {
                    place,
                    access: StructuralAccess::MutableBorrow,
                    path: Vec::new(),
                    root_structural_type: structural_type,
                    structural_type,
                    shape: borrowed,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    source: input.into(),
                    destination: call_plan.parameters[0].clone(),
                },
            }],
            call_plan,
            result_placement: None,
            structural_result: None,
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        });
        let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator else {
            panic!("return");
        };
        returned.value = LegalizedScalarReturnValue::StructuralParameter { place };
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            native,
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
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert_eq!(selected.local_storage_slots.len(), 1);
        assert_eq!(
            selected.local_storage_slots[0].id,
            selected_instructions::LocalStorageSlotId::StructuralParameter { place }
        );
        let rows = &selected.blocks[0].instructions;
        let call_position = rows
            .iter()
            .position(|row| matches!(row.kind, SelectedInstructionKind::CallUnit { .. }))
            .unwrap();
        let returned_load = rows
            .iter()
            .position(|row| matches!(row.kind, SelectedInstructionKind::Load16 { byte_offset: 0 }))
            .unwrap();
        assert!(
            call_position < returned_load,
            "return observes the completed borrowed call's current value"
        );
        let incoming = selected
            .virtual_registers
            .iter()
            .find(|register| {
                matches!(
                    register.origin,
                    selected_instructions::VirtualRegisterOrigin::StructuralParameter { .. }
                )
            })
            .unwrap()
            .id;
        let mut stale = selected.clone();
        let load = &mut stale.blocks[0].instructions[returned_load];
        load.kind = SelectedInstructionKind::ZeroExtendU16;
        load.constraint = constraints.keys.copy_i64;
        load.operands[0].virtual_register = incoming;
        assert!(
            validate(&stale).is_err(),
            "a stale captured fragment cannot replace the current home"
        );
    }
}

#[test]
fn owned_parameter_child_rejoins_exact_captured_fragments() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = nested_fixture(target, 2);
        let child = source.blocks[0].instructions.remove(2);
        let LegalizedScalarInstructionKind::EstablishRecord { result, shape, .. } = child.kind
        else {
            panic!("child");
        };
        source
            .provenance
            .operations
            .retain(|operation| *operation != child.operation);
        source.call_plan = evaluate_call_plan(
            source.call_plan.policy,
            &CallSignature {
                parameters: vec![shape],
                result: source
                    .call_plan
                    .result
                    .as_ref()
                    .map(|placement| placement.shape),
            },
        )
        .unwrap();
        let signature = source.structural.as_mut().unwrap();
        signature
            .structural_places
            .retain(|place| place.id != result.place);
        signature
            .parameters
            .push(legalized_operations::LegalizedCallUnitParameter {
                semantic: terminal_psi::StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    is_self: false,
                    structural_type: result.structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place: result.place,
                    structural_type: result.structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    projected_qualifications: Vec::new(),
                    shape,
                    placement: source.call_plan.parameters[0].clone(),
                },
            });
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
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
        .unwrap_or_else(|error| panic!("parameter {target:?}: {error:?}"));
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        for mutation in 0..4 {
            let mut changed = source.clone();
            let parameter = &mut changed.structural.as_mut().unwrap().parameters[0];
            match mutation {
                0 => parameter.semantic.access = StructuralAccess::SharedBorrow,
                1 => parameter.semantic.multiplicity = StructuralMultiplicity::Linear,
                2 => parameter.semantic.structural_type = StructuralTypeId::new(3).unwrap(),
                3 => parameter.target.placement.locations.clear(),
                _ => unreachable!(),
            }
            assert!(validate(&changed, &selected).is_err());
        }
        let mut changed = selected.clone();
        let register = changed
            .virtual_registers
            .iter_mut()
            .find(|register| {
                matches!(
                    register.origin,
                    selected_instructions::VirtualRegisterOrigin::StructuralParameter { .. }
                )
            })
            .unwrap();
        register.entry_fixed_view = None;
        assert!(validate(&source, &changed).is_err());
        let mut copy_source = source.clone();
        let parameter = &mut copy_source.structural.as_mut().unwrap().parameters[0];
        parameter.semantic.multiplicity = StructuralMultiplicity::Unrestricted;
        parameter.target.multiplicity = StructuralMultiplicity::Unrestricted;
        let copied = build(
            0,
            &copy_source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate(&copy_source, &copied).unwrap();
        let incoming = copied
            .virtual_registers
            .iter()
            .find(|register| {
                matches!(
                    register.origin,
                    selected_instructions::VirtualRegisterOrigin::StructuralParameter { .. }
                )
            })
            .unwrap()
            .id;
        let mut aliased = copied.clone();
        let store = aliased
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .rfind(|row| {
                matches!(
                    row.kind,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 2
                    }
                )
            })
            .expect("nested two-byte child is copied into parent storage");
        assert_ne!(store.operands[0].virtual_register, incoming);
        store.operands[0].virtual_register = incoming;
        assert!(
            validate(&copy_source, &aliased).is_err(),
            "owned copy cannot write through the incoming value"
        );
    }
}
