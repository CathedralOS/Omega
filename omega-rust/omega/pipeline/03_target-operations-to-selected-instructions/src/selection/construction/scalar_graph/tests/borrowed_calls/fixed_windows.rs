//! Distinct call operands need distinct descriptors, not distinct backing owners.
use super::{
    CallSignature, IntegerSign, IntegerType, LegalizedScalarArgument,
    LegalizedScalarInstructionKind, SelectedFunction, SelectedSelectionConstraints,
    StructuralAccess, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueShape,
    borrowed_call, build, evaluate_call_plan,
};

#[test]
fn repeated_fixed_backing_windows_have_operand_owned_descriptors() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        for (access, windows) in [
            (StructuralAccess::MutableBorrow, [(0, 2), (2, 4)]),
            (StructuralAccess::SharedBorrow, [(1, 3), (1, 3)]),
            (StructuralAccess::SharedBorrow, [(0, 0), (4, 4)]),
        ] {
            let mut source = borrowed_call(native);
            let array = StructuralTypeId::new(2).unwrap();
            let byte = StructuralTypeId::new(3).unwrap();
            let shape = ValueShape::borrowed_reference(4, 1);
            source.call_plan = evaluate_call_plan(
                source.call_plan.policy,
                &CallSignature {
                    parameters: vec![shape],
                    result: Some(ValueShape::integer(8, 8)),
                },
            )
            .unwrap();
            let incoming = source.call_plan.parameters[0].clone();
            let signature = source.structural.as_mut().unwrap();
            let mut declarations = signature.structural_types.to_vec();
            declarations.extend([
                StructuralTypeDeclaration {
                    id: array,
                    identity: "fixed-backing".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: byte,
                        length: 4,
                    },
                },
                StructuralTypeDeclaration {
                    id: byte,
                    identity: "byte".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(
                        semantic_vocabulary::ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                        ),
                    ),
                },
            ]);
            signature.structural_types = declarations.into();
            let parameter = &mut signature.parameters[0];
            parameter.semantic.structural_type = array;
            parameter.semantic.access = access;
            parameter.target.structural_type = array;
            parameter.target.access = access;
            parameter.target.shape = shape;
            parameter.target.placement = incoming.clone();
            let row = &mut source.blocks[0].instructions[0];
            let operation = row.operation;
            let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
                panic!("borrowed call");
            };
            call.call_plan = evaluate_call_plan(
                source.call_plan.policy,
                &CallSignature {
                    parameters: vec![ValueShape::borrowed_reference(16, 8); 2],
                    result: Some(ValueShape::integer(8, 8)),
                },
            )
            .unwrap();
            call.result_placement = call.call_plan.result.clone();
            let template = call.arguments[0].clone();
            call.arguments = windows
                .into_iter()
                .enumerate()
                .map(|(argument_index, (start, end))| {
                    let mut argument = template.clone();
                    let LegalizedScalarArgument::Structural { semantic, target } = &mut argument
                    else {
                        panic!("structural operand");
                    };
                    semantic.access = access;
                    semantic.path =
                        vec![terminal_psi::StructuralPathSegment::FixedByteRange { start, end }];
                    target.access = access;
                    target.path = semantic.path.clone();
                    target.root_structural_type = array;
                    target.source_byte_offset = start as u32;
                    target.fixed_array_length = Some(end - start);
                    target.element_stride = Some(1);
                    target.source = incoming.clone().into();
                    target.destination = call.call_plan.parameters[argument_index].clone();
                    argument
                })
                .collect();
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
            assert_eq!(selected.local_storage_slots.len(), 2);
            for (argument_index, slot) in selected.local_storage_slots.iter().enumerate() {
                assert_eq!(
                    slot.id,
                    selected_instructions::LocalStorageSlotId::StructuralCallArgument {
                        operation,
                        argument_index: argument_index as u32,
                    }
                );
                assert_eq!((slot.byte_size, slot.alignment), (16, 8));
            }
            let mut changed = selected.clone();
            changed.local_storage_slots[1].id = changed.local_storage_slots[0].id;
            assert!(
                validate(&changed).is_err(),
                "two operands must not share descriptor storage"
            );
        }
    }
}
