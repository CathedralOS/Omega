//! Graph parameters retain exact value or reference ABI and published pointer homes.
//! This is record shape only: the mandatory exact image join retains graph replay.
use machine_code::StructuralSourceLocation;

pub(crate) fn indirect_source_location(
    pointer: &calling_conventions::IndirectPointerLocation,
) -> StructuralSourceLocation {
    match pointer {
        calling_conventions::IndirectPointerLocation::Register(register) => {
            StructuralSourceLocation::IncomingIndirectPointer {
                register: *register,
            }
        }
        calling_conventions::IndirectPointerLocation::Stack {
            stack_byte_offset,
            alignment,
        } => StructuralSourceLocation::IncomingIndirectStackPointer {
            stack_byte_offset: *stack_byte_offset,
            alignment: *alignment,
        },
    }
}
