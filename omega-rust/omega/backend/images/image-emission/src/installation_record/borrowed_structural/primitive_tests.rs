use super::pointer_location;
use super::primitive_origin::local_primitive_source_is_exact;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use machine_code::{InternalUnitStructuralArgumentSourceRecord, StructuralSourceLocation};
use terminal_psi::{StructuralAccess, StructuralPathSegment};

#[test]
fn primitive_local_installation_keeps_exact_referent_extent_and_borrowed_access() {
    for size in [1, 2, 4, 8] {
        let shape = ValueShape::borrowed_reference(size, size);
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
            access: StructuralAccess::MutableBorrow,
            path: vec![],
            root_structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            shape,
            source_byte_offset: 0,
            source_location: StructuralSourceLocation::Stack { byte_offset: 16 },
            call_stack_bytes: 32,
            fixed_array_length: None,
            element_stride: None,
            source: InternalUnitStructuralArgumentSourceRecord::EstablishedPrimitiveLocal {
                psi_operation: semantic_vocabulary::OperationId::new(10).unwrap(),
            },
            destination: plan.parameters[0].clone(),
            code_offset: 40,
            byte_count: 5,
            bytes: vec![0; 5],
        };
        assert!(local_primitive_source_is_exact(
            &argument,
            16 + u32::from(size)
        ));
        assert!(!local_primitive_source_is_exact(
            &argument,
            15 + u32::from(size)
        ));
        for mutation in 0..7 {
            let mut changed = argument.clone();
            match mutation {
                0 => changed.access = StructuralAccess::Owned,
                1 => changed.shape = ValueShape::integer(size, size),
                2 => changed.source_byte_offset = 1,
                3 => changed.path.push(StructuralPathSegment::FixedIndex(0)),
                4 => {
                    changed.root_structural_type =
                        semantic_vocabulary::StructuralTypeId::new(2).unwrap()
                }
                5 => changed.shape = ValueShape::borrowed_reference(16, 8),
                _ => {
                    changed.source_location = StructuralSourceLocation::IncomingBorrowedPointer {
                        location: pointer_location(&plan.parameters[0]).unwrap(),
                    }
                }
            }
            assert!(
                !local_primitive_source_is_exact(&changed, 32),
                "size {size}, mutation {mutation}"
            );
        }
    }
}
