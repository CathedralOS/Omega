use super::{local_view_source_is_exact, outgoing_pointer_fits, pointer_location};
use calling_conventions::{
    CallSignature, CallingPolicy, IndirectPointerLocation, ValueClass, ValueLocation, ValueShape,
    evaluate_call_plan,
};
use machine_code::{InternalUnitStructuralArgumentSourceRecord, StructuralSourceLocation};
use terminal_psi::{StructuralAccess, StructuralPathSegment};

#[test]
fn established_descriptor_shape_uses_real_aligned_local_storage() {
    let shape = ValueShape::borrowed_reference(16, 8);
    let plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let argument = machine_code::InternalUnitCallArgumentRecord {
        place: semantic_vocabulary::PlaceId::new(2).unwrap(),
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
        root_structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
        structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
        shape,
        source_byte_offset: 0,
        source_location: StructuralSourceLocation::Stack { byte_offset: 16 },
        call_stack_bytes: 32,
        fixed_array_length: None,
        element_stride: None,
        source: InternalUnitStructuralArgumentSourceRecord::EstablishedByteView {
            psi_operation: semantic_vocabulary::OperationId::new(10).unwrap(),
        },
        destination: plan.parameters[0].clone(),
        code_offset: 40,
        byte_count: 5,
        bytes: vec![0; 5],
    };
    assert!(local_view_source_is_exact(&argument, 32));
    assert!(!local_view_source_is_exact(&argument, 31));
    for mutation in 0..9 {
        let mut changed = argument.clone();
        match mutation {
            0 => changed.access = StructuralAccess::Owned,
            1 => changed.shape = ValueShape::integer(16, 8),
            2 => changed.source_byte_offset = 8,
            3 => changed.path.push(StructuralPathSegment::FixedIndex(0)),
            4 => {
                changed.root_structural_type =
                    semantic_vocabulary::StructuralTypeId::new(2).unwrap()
            }
            5 => changed.source_location = StructuralSourceLocation::Stack { byte_offset: 17 },
            6 => changed.source_location = StructuralSourceLocation::Stack { byte_offset: 24 },
            7 => {
                changed.source_location = StructuralSourceLocation::Stack {
                    byte_offset: u32::MAX - 7,
                }
            }
            _ => {
                changed.source_location = StructuralSourceLocation::IncomingBorrowedPointer {
                    location: pointer_location(&plan.parameters[0]).unwrap(),
                }
            }
        }
        assert!(
            !local_view_source_is_exact(&changed, 32),
            "mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_pointer_shapes_keep_incoming_stack_distinct_from_local_copies() {
    for (target, scalar_count) in [
        (target::NativeTarget::windows_x64(), 4),
        (target::NativeTarget::linux_x64(), 6),
        (target::NativeTarget::linux_arm64(), 8),
        (target::NativeTarget::macos_arm64(), 8),
    ] {
        let shape = ValueShape {
            class: ValueClass::BorrowedReference,
            byte_size: 16,
            alignment: 8,
        };
        for prefix in [0, scalar_count] {
            let mut parameters = vec![ValueShape::integer(8, 8); prefix];
            parameters.push(shape);
            let plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters,
                    result: None,
                },
            )
            .expect("native reference plan");
            let pointer = plan.parameters.last().expect("borrowed parameter");
            let location = pointer_location(pointer).expect("pointer location");
            if prefix == 0 {
                assert!(matches!(location, IndirectPointerLocation::Register(_)));
            } else {
                let IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } = location
                else {
                    panic!("stack pointer");
                };
                assert!(!outgoing_pointer_fits(pointer, stack_byte_offset + 7));
                assert!(outgoing_pointer_fits(pointer, stack_byte_offset + 8));
            }
            let mut copied = pointer.clone();
            copied.locations = vec![ValueLocation::Indirect {
                pointer: location,
                copy_stack_byte_offset: Some(64),
                byte_size: 16,
                alignment: 8,
            }];
            assert_eq!(pointer_location(&copied), None);
            let mut owned = pointer.clone();
            owned.shape.class = ValueClass::Integer;
            assert_eq!(pointer_location(&owned), None);
        }
    }
}
