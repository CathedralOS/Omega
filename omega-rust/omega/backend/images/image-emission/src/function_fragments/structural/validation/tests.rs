use calling_conventions::{
    IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement, ValueShape,
};
use machine_code::StructuralSourceLocation;
use terminal_psi::StructuralAccess;

use super::validate_incoming_location;

fn borrowed(location: IndirectPointerLocation) -> StructuralSourceLocation {
    StructuralSourceLocation::IncomingBorrowedPointer { location }
}

#[test]
fn borrowed_register_and_stack_locations_preserve_exact_pointer_residence() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let register = MachineRegister::Aarch64X(0);
        let placement = ValuePlacement {
            shape: ValueShape::borrowed_reference(4, 2),
            locations: vec![ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            }],
        };
        assert!(
            validate_incoming_location(
                access,
                &placement,
                borrowed(IndirectPointerLocation::Register(register))
            )
            .is_ok()
        );
        assert!(
            validate_incoming_location(
                access,
                &placement,
                borrowed(IndirectPointerLocation::Register(
                    MachineRegister::Aarch64X(1)
                ))
            )
            .is_err()
        );
        assert!(
            validate_incoming_location(
                access,
                &placement,
                StructuralSourceLocation::IncomingIndirectPointer { register }
            )
            .is_err()
        );
        for stack_byte_offset in [0, 8, 32, 56] {
            let pointer = IndirectPointerLocation::Stack {
                stack_byte_offset,
                alignment: 8,
            };
            for location in [
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: None,
                    byte_size: 4,
                    alignment: 2,
                },
            ] {
                let placement = ValuePlacement {
                    shape: ValueShape::borrowed_reference(4, 2),
                    locations: vec![location],
                };
                assert!(validate_incoming_location(access, &placement, borrowed(pointer)).is_ok());
                assert!(
                    validate_incoming_location(
                        access,
                        &placement,
                        borrowed(IndirectPointerLocation::Stack {
                            stack_byte_offset: stack_byte_offset + 8,
                            alignment: 8
                        })
                    )
                    .is_err()
                );
                assert!(
                    validate_incoming_location(
                        access,
                        &placement,
                        StructuralSourceLocation::Stack {
                            byte_offset: stack_byte_offset
                        }
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn borrowed_locations_reject_copies_carrier_width_alignment_and_access_substitution() {
    let pointer = IndirectPointerLocation::Register(MachineRegister::Aarch64X(0));
    let mut placement = ValuePlacement {
        shape: ValueShape::borrowed_reference(16, 8),
        locations: vec![ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }],
    };
    let location = borrowed(pointer);
    assert!(
        validate_incoming_location(StructuralAccess::WriteOnlyBorrow, &placement, location).is_ok()
    );
    assert!(validate_incoming_location(StructuralAccess::Owned, &placement, location).is_err());
    for malformed in [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(0),
            byte_size: 16,
            alignment: 8,
        },
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: None,
            byte_size: 8,
            alignment: 8,
        },
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 4,
        },
        ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            value_byte_offset: 1,
            byte_size: 8,
        },
        ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            value_byte_offset: 0,
            byte_size: 4,
        },
    ] {
        placement.locations[0] = malformed;
        assert!(
            validate_incoming_location(StructuralAccess::WriteOnlyBorrow, &placement, location)
                .is_err()
        );
    }
    placement.locations = vec![ValueLocation::Register {
        register: MachineRegister::Aarch64X(0),
        value_byte_offset: 0,
        byte_size: 8,
    }];
    placement.shape = ValueShape::integer(8, 8);
    assert!(
        validate_incoming_location(StructuralAccess::WriteOnlyBorrow, &placement, location)
            .is_err()
    );
}

#[test]
fn owned_indirect_location_keeps_owned_copy_meaning() {
    let register = MachineRegister::Aarch64X(0);
    let placement = ValuePlacement {
        shape: ValueShape::integer(16, 8),
        locations: vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(register),
            copy_stack_byte_offset: Some(0),
            byte_size: 16,
            alignment: 8,
        }],
    };
    assert!(
        validate_incoming_location(
            StructuralAccess::Owned,
            &placement,
            StructuralSourceLocation::IncomingIndirectPointer { register }
        )
        .is_ok()
    );
    assert!(
        validate_incoming_location(
            StructuralAccess::Owned,
            &placement,
            borrowed(IndirectPointerLocation::Register(register))
        )
        .is_err()
    );
    assert!(
        validate_incoming_location(
            StructuralAccess::SharedBorrow,
            &placement,
            borrowed(IndirectPointerLocation::Register(register))
        )
        .is_err()
    );
}
