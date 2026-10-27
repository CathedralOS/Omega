use super::encode_machine_register;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    PackageReviewBoundaryValueClass, PackageReviewBoundaryValueLocation,
    PackageReviewBoundaryValuePlacement, PackageReviewIndirectPointerLocation,
    PackageReviewSystemVEightbyteClass,
};

pub(crate) fn encode_value_placement(
    encoder: &mut Encoder,
    placement: &PackageReviewBoundaryValuePlacement,
) -> Result<(), PackageReviewEncodingError> {
    let shape = placement.shape();
    encoder.field("class", |encoder| {
        match shape.class() {
            PackageReviewBoundaryValueClass::Integer => encoder.tag("integer", 0),
            PackageReviewBoundaryValueClass::Float => encoder.tag("float", 1),
            PackageReviewBoundaryValueClass::BorrowedReference => {
                encoder.tag("borrowed_reference", 4)
            }
            PackageReviewBoundaryValueClass::BorrowedScalarReference => {
                encoder.tag("borrowed_scalar_reference", 5)
            }
            PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members } => {
                encoder.tag("homogeneous_float_aggregate", 2);
                encoder.byte_field("members", members)?;
            }
            PackageReviewBoundaryValueClass::SystemVAggregate { first, second } => {
                encoder.tag("system_v_aggregate", 3);
                encoder.field("first", |encoder| {
                    encode_system_v_class(encoder, first);
                    Ok(())
                })?;
                encoder.field("second", |encoder| {
                    encode_system_v_class(encoder, second);
                    Ok(())
                })?;
            }
        };
        Ok(())
    })?;
    encoder.u32_field("byte_size", shape.byte_size())?;
    encoder.u16_field("alignment", shape.alignment())?;
    encoder.field("locations", |encoder| {
        encoder.sequence(placement.locations(), encode_value_location)
    })
}

fn encode_value_location(
    encoder: &mut Encoder,
    location: &PackageReviewBoundaryValueLocation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("location", |encoder| {
        match *location {
            PackageReviewBoundaryValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                encoder.tag("register", 0);
                encoder.field("register", |encoder| {
                    encode_machine_register(encoder, register);
                    Ok(())
                })?;
                encoder.u32_field("value_byte_offset", value_byte_offset)?;
                encoder.u32_field("byte_size", byte_size)?;
            }
            PackageReviewBoundaryValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                encoder.tag("stack", 1);
                encoder.u32_field("stack_byte_offset", stack_byte_offset)?;
                encoder.u32_field("value_byte_offset", value_byte_offset)?;
                encoder.u32_field("byte_size", byte_size)?;
                encoder.u16_field("alignment", alignment)?;
            }
            PackageReviewBoundaryValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                encoder.tag("indirect", 2);
                encoder.field("pointer", |encoder| {
                    match pointer {
                        PackageReviewIndirectPointerLocation::Register(register) => {
                            encoder.tag("register", 0);
                            encoder.field("register", |encoder| {
                                encode_machine_register(encoder, register);
                                Ok(())
                            })?;
                        }
                        PackageReviewIndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => {
                            encoder.tag("stack", 1);
                            encoder.u32_field("stack_byte_offset", stack_byte_offset)?;
                            encoder.u16_field("alignment", alignment)?;
                        }
                    };
                    Ok(())
                })?;
                encoder.field("copy_stack_byte_offset", |encoder| {
                    encoder.option(copy_stack_byte_offset.as_ref(), |encoder, offset| {
                        encoder.u32_field("offset", *offset)?;
                        Ok(())
                    })
                })?;
                encoder.u32_field("byte_size", byte_size)?;
                encoder.u16_field("alignment", alignment)?;
            }
        };
        Ok(())
    })?;
    Ok(())
}

fn encode_system_v_class(encoder: &mut Encoder, class: PackageReviewSystemVEightbyteClass) {
    match class {
        PackageReviewSystemVEightbyteClass::Integer => encoder.tag("integer", 0),
        PackageReviewSystemVEightbyteClass::Sse => encoder.tag("sse", 1),
    }
}
